use exhume_grep::*;
use exhume_indexer::{
    PartitionKind, ensure_evidence_row, ensure_tables, index_folder, insert_partition,
};
use sqlx::sqlite::SqlitePoolOptions;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    db: PathBuf,
    partition: i64,
}
impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("evidence");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("one.bin"), b"banana\0").unwrap();
        std::fs::hard_link(root.join("one.bin"), root.join("alias.bin")).unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/two.txt"), b"another banana\n").unwrap();
        std::fs::write(root.join("empty"), b"").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("one.bin", root.join("link")).unwrap();
        let db = temp.path().join("index.sqlite");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&db)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        ensure_tables(&pool).await.unwrap();
        ensure_evidence_row(&pool, 1, root.to_str().unwrap(), true)
            .await
            .unwrap();
        let partition = insert_partition(&pool, 1, PartitionKind::Folder, 0, 0, 512, 0, None)
            .await
            .unwrap();
        index_folder(
            1,
            partition,
            root.to_string_lossy().into_owned(),
            &pool,
            None,
            None,
        )
        .await;
        // Folder walking currently deduplicates hard links. Add the second
        // occurrence that an NTFS-style inventory can contain, without changing
        // the indexer or relying on directory enumeration order.
        sqlx::query("INSERT INTO system_files (evidence_id, partition_id, identifier, absolute_path, name, ftype, size, created, modified, accessed, path_key, parent_path_key, depth, is_dir, metadata) SELECT evidence_id, partition_id, identifier, CASE WHEN absolute_path = '/one.bin' THEN '/alias.bin' ELSE '/one.bin' END, CASE WHEN absolute_path = '/one.bin' THEN 'alias.bin' ELSE 'one.bin' END, ftype, size, created, modified, accessed, CASE WHEN absolute_path = '/one.bin' THEN '/alias.bin' ELSE '/one.bin' END, '/', 1, 0, metadata FROM system_files WHERE name IN ('one.bin', 'alias.bin') LIMIT 1")
            .execute(&pool).await.unwrap();
        pool.close().await;
        Self {
            _temp: temp,
            root,
            db,
            partition,
        }
    }
    fn request(&self) -> SearchRequest {
        SearchRequest::new(
            &self.db,
            1,
            MatcherOptions {
                patterns: vec![Pattern {
                    id: "needle".into(),
                    expression: "ana".into(),
                }],
                chunk_size: 2,
                ..Default::default()
            },
        )
    }
}
async fn collect(request: SearchRequest) -> (SearchSummary, Vec<SearchEvent>) {
    let (tx, mut rx) = mpsc::channel(2);
    let cancel = Arc::new(AtomicBool::new(false));
    let task = tokio::spawn(search(request, tx, cancel));
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }
    let summary = task.await.unwrap().unwrap();
    (summary, events)
}
#[tokio::test]
async fn actual_indexer_hardlinks_workers_offsets_and_read_only_index() {
    let f = Fixture::new().await;
    let before = std::fs::read(&f.db).unwrap();
    for workers in [1, 3] {
        let mut r = f.request();
        r.workers = workers;
        let (summary, events) = collect(r).await;
        assert_eq!(summary.status, Status::Complete, "{events:?}");
        assert_eq!(summary.hits, 4);
        assert_eq!(summary.scanned_files, 3);
        assert_eq!(summary.matched_files, 2);
        assert_eq!(summary.selected_paths, 4);
        let mut offsets: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let SearchEvent::Match { hit, .. } = e {
                    Some(hit.offset)
                } else {
                    None
                }
            })
            .collect();
        offsets.sort();
        assert_eq!(offsets, [1, 3, 9, 11]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SearchEvent::Match { files, .. } if files.aliases.len() == 1))
        );
        assert!(matches!(events.last(), Some(SearchEvent::Finished { .. })));
    }
    assert_eq!(std::fs::read(&f.db).unwrap(), before);
}
#[tokio::test]
async fn selected_scope_and_filters_apply_to_aliases() {
    let f = Fixture::new().await;
    let mut r = f.request();
    r.partition_ids = vec![f.partition];
    r.directory = Some("/sub".into());
    let (summary, _) = collect(r).await;
    assert_eq!(summary.hits, 2);
    assert_eq!(summary.selected_paths, 1);
    let mut r = f.request();
    r.include_paths = vec!["alias\\.bin$".into()];
    let (summary, events) = collect(r).await;
    assert_eq!(summary.hits, 2);
    assert!(events.iter().any(|e| matches!(e, SearchEvent::Match { files, .. } if files.file.path.ends_with("alias.bin") && files.aliases.is_empty())));
    let mut r = f.request();
    r.max_size = Some(0);
    let (summary, _) = collect(r).await;
    assert_eq!(summary.scanned_files, 1);
    assert_eq!(summary.hits, 0);
    assert_eq!(summary.status, Status::Complete);
}
#[tokio::test]
async fn changed_missing_and_unreadable_are_incomplete() {
    let f = Fixture::new().await;
    std::fs::write(f.root.join("one.bin"), b"changed and now longer").unwrap();
    std::fs::remove_file(f.root.join("sub/two.txt")).unwrap();
    let (summary, events) = collect(f.request()).await;
    assert_eq!(summary.status, Status::Incomplete);
    assert_eq!(summary.failed_files, 3);
    assert_eq!(summary.hits, 0);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, SearchEvent::FileError { .. }))
            .count(),
        3
    );
}
#[tokio::test]
async fn limits_and_files_only_have_distinct_terminal_states() {
    let f = Fixture::new().await;
    let mut r = f.request();
    r.max_hits = Some(1);
    r.workers = 3;
    let (summary, events) = collect(r).await;
    assert_eq!(summary.status, Status::Limited);
    assert_eq!(summary.hits, 1);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, SearchEvent::Match { .. }))
            .count(),
        1
    );
    let mut r = f.request();
    r.max_hits_per_file = Some(1);
    let (summary, _) = collect(r).await;
    assert_eq!(summary.status, Status::Limited);
    assert_eq!(summary.hits, 2);
    let mut r = f.request();
    r.files_only = true;
    let (summary, events) = collect(r).await;
    assert_eq!(summary.status, Status::Complete);
    assert_eq!(summary.hits, 2);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, SearchEvent::FileMatched { .. }))
            .count(),
        2
    );
}
#[tokio::test]
async fn long_lines_produce_incomplete_coverage_and_exact_ranges() {
    let f = Fixture::new().await;
    let mut r = f.request();
    r.matcher.mode = Mode::Regex;
    r.matcher.max_line_bytes = 5;
    let (summary, events) = collect(r).await;
    assert_eq!(summary.status, Status::Incomplete);
    assert_eq!(summary.hits, 0);
    assert_eq!(summary.skipped_ranges, 2);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SearchEvent::SkippedRange { range, .. } if range.length == 14))
    );
}
#[tokio::test]
async fn cancellation_unblocks_backpressure_without_a_draining_consumer() {
    let f = Fixture::new().await;
    let (tx, _rx) = mpsc::channel(1);
    let cancel = Arc::new(AtomicBool::new(false));
    let task = tokio::spawn(search(f.request(), tx, cancel.clone()));
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel.store(true, Ordering::Relaxed);
    let summary = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(summary.status, Status::Cancelled);
}
#[tokio::test]
async fn invalid_request_never_creates_a_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.sqlite");
    let mut r = SearchRequest::new(&path, 1, MatcherOptions::default());
    r.matcher.patterns = vec![Pattern {
        id: "x".into(),
        expression: "x".into(),
    }];
    let (tx, _rx) = mpsc::channel(10);
    assert!(
        search(r, tx, Arc::new(AtomicBool::new(false)))
            .await
            .is_err()
    );
    assert!(!path.exists());
}

#[tokio::test]
async fn raw_exfat_indexer_to_search_fragmentation_and_uninitialized_tail() {
    let temp = tempfile::tempdir().unwrap();
    let image = temp.path().join("exfat.raw");
    let mut bytes = vec![0u8; 512 * 512];
    bytes[3..11].copy_from_slice(b"EXFAT   ");
    bytes[510..512].copy_from_slice(&0xaa55u16.to_le_bytes());
    bytes[0x48..0x50].copy_from_slice(&512u64.to_le_bytes());
    for (at, value) in [(0x50, 1u32), (0x54, 1), (0x58, 8), (0x5c, 32), (0x60, 2)] {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[0x68..0x6a].copy_from_slice(&0x100u16.to_le_bytes());
    bytes[0x6c] = 9;
    bytes[0x6d] = 3;
    bytes[0x6e] = 1;
    for (cluster, next) in [(2usize, u32::MAX), (3, 7), (7, u32::MAX)] {
        bytes[512 + cluster * 4..516 + cluster * 4].copy_from_slice(&next.to_le_bytes());
    }
    let directory = 8 * 512;
    bytes[directory] = 0x85;
    bytes[directory + 1] = 2;
    bytes[directory + 4] = 0x20;
    for at in [8, 12, 16] {
        bytes[directory + at..directory + at + 4].copy_from_slice(&0x58210000u32.to_le_bytes());
    }
    let stream = directory + 32;
    bytes[stream] = 0xc0;
    bytes[stream + 1] = 1;
    bytes[stream + 3] = 8;
    bytes[stream + 8..stream + 16].copy_from_slice(&4500u64.to_le_bytes());
    bytes[stream + 20..stream + 24].copy_from_slice(&3u32.to_le_bytes());
    bytes[stream + 24..stream + 32].copy_from_slice(&5000u64.to_le_bytes());
    bytes[directory + 64] = 0xc1;
    for (i, unit) in "test.bin".encode_utf16().enumerate() {
        bytes[directory + 66 + i * 2..directory + 68 + i * 2].copy_from_slice(&unit.to_le_bytes());
    }
    let cluster3 = 8 * 512 + 4096;
    let cluster7 = 8 * 512 + 5 * 4096;
    bytes[cluster3..cluster3 + 4096].fill(b'x');
    bytes[cluster7..cluster7 + 4096].fill(b'x');
    bytes[cluster3 + 4094..cluster3 + 4096].copy_from_slice(b"ne");
    bytes[cluster7..cluster7 + 4].copy_from_slice(b"edle");
    // This physical signature is beyond ValidDataLength: logical search must
    // see zeroes here, not bytes from uninitialized allocation.
    bytes[cluster7 + 500..cluster7 + 506].copy_from_slice(b"needle");
    std::fs::write(&image, bytes).unwrap();
    let db = temp.path().join("image.sqlite");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    ensure_tables(&pool).await.unwrap();
    ensure_evidence_row(&pool, 1, image.to_str().unwrap(), false)
        .await
        .unwrap();
    let part = insert_partition(
        &pool,
        1,
        PartitionKind::Logical,
        0,
        512,
        512,
        512 * 512,
        None,
    )
    .await
    .unwrap();
    exhume_indexer::index_partition(
        1,
        part,
        512,
        0,
        image.to_string_lossy().into_owned(),
        &pool,
        None,
        None,
    )
    .await
    .unwrap();
    pool.close().await;
    let request = SearchRequest::new(
        db,
        1,
        MatcherOptions {
            patterns: vec![Pattern {
                id: "cross_extent".into(),
                expression: "needle".into(),
            }],
            chunk_size: 7,
            ..Default::default()
        },
    );
    let (summary, events) = collect(request).await;
    assert_eq!(summary.status, Status::Complete, "{events:?}");
    assert_eq!(summary.hits, 1);
    assert_eq!(summary.bytes_read, 5000);
    assert!(events.iter().any(
        |e| matches!(e, SearchEvent::Match { hit, .. } if hit.offset == 4094 && hit.length == 6)
    ));
}

#[tokio::test]
async fn cli_ndjson_exit_codes_and_pattern_validation() {
    let f = Fixture::new().await;
    let run = |extra: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_exhume_grep"))
            .arg("--database")
            .arg(&f.db)
            .args(["--evidence-id", "1", "--output", "ndjson"])
            .args(extra)
            .output()
            .unwrap()
    };
    let found = run(&["-e", "ana"]);
    assert_eq!(found.status.code(), Some(0));
    let events: Vec<SearchEvent> = String::from_utf8(found.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        matches!(events.last(), Some(SearchEvent::Finished { summary }) if summary.status == Status::Complete && summary.hits == 4)
    );
    assert_eq!(run(&["-e", "NEVER_EXISTS"]).status.code(), Some(1));
    assert_eq!(
        run(&["-e", "ana", "--max-hits", "1"]).status.code(),
        Some(2)
    );
    let invalid = run(&["--mode", "regex", "-e", "(?=x)"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
}

#[tokio::test]
async fn text_output_contains_only_matches_and_preserves_alias_paths() {
    let f = Fixture::new().await;
    for (pattern, lines) in [("ana", 6), ("NEVER_EXISTS", 0)] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_exhume_grep"))
            .arg("--database")
            .arg(&f.db)
            .args(["--evidence-id", "1", "-e", pattern])
            .output()
            .unwrap();
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(text.lines().count(), lines, "{text}");
        if lines > 0 {
            assert!(text.contains("/alias.bin") && text.contains("/one.bin"));
        }
    }
}

#[tokio::test]
async fn cli_show_line_groups_aliases_colors_and_keeps_ndjson_ansi_free() {
    let f = Fixture::new().await;
    let run = |extra: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_exhume_grep"))
            .arg("--database")
            .arg(&f.db)
            .args(["--evidence-id", "1", "-e", "ana", "--show-line"])
            .args(extra)
            .output()
            .unwrap()
    };
    for extra in [vec![], vec!["--color", "never"], vec!["--color", "auto"]] {
        let output = run(&extra);
        assert_eq!(output.status.code(), Some(0));
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(text.lines().count(), 3, "{text}");
        assert!(!text.contains('\x1b'));
        assert!(text.contains("/alias.bin") && text.contains("/one.bin"));
        assert!(text.contains("banana\\u{0}") && text.contains("another banana"));
    }
    let output = run(&["--color", "always"]);
    let text = String::from_utf8(output.stdout).unwrap();
    // Overlapping literal occurrences at 1..4 and 3..6 form one red span.
    assert_eq!(text.matches("\x1b[31manana\x1b[0m").count(), 3, "{text}");
    assert_eq!(text.lines().count(), 3);
    let output = run(&["--output", "ndjson", "--color", "always"]);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains('\x1b') && !text.contains("\\u001b"));
    let events: Vec<SearchEvent> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(matches!(
        events.first(),
        Some(SearchEvent::Started {
            schema_version: 2,
            ..
        })
    ));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, SearchEvent::Match { .. }))
            .count(),
        4
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, SearchEvent::MatchingLine { .. }))
            .count(),
        2
    );
    assert!(
        matches!(events.last(), Some(SearchEvent::Finished { summary }) if summary.hits == 4 && summary.status == Status::Complete)
    );
    for extra in [
        vec!["--files-with-matches"],
        vec!["--line-bytes", "0"],
        vec!["--line-bytes", "65537"],
    ] {
        let output = run(&extra);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[tokio::test]
async fn line_context_respects_occurrence_caps_and_early_stopping() {
    let f = Fixture::new().await;
    for overall in [false, true] {
        let mut request = f.request();
        request.matcher.show_line = true;
        if overall {
            request.max_hits = Some(1);
        } else {
            request.max_hits_per_file = Some(1);
        }
        let (summary, events) = collect(request).await;
        assert_eq!(summary.status, Status::Limited);
        assert_eq!(summary.hits, if overall { 1 } else { 2 });
        let contexts: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let SearchEvent::MatchingLine { line, .. } = e {
                    Some(line)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(contexts.len(), summary.hits as usize, "{events:?}");
        assert!(contexts.iter().all(|l| l.partial && l.truncated_end));
        assert!(
            contexts
                .iter()
                .all(|l| l.spans.iter().map(|s| s.length).sum::<u64>() == 3)
        );
    }
    let mut request = f.request();
    request.files_only = true;
    request.matcher.show_line = true;
    let (tx, _rx) = mpsc::channel(1);
    assert!(
        search(request, tx, Arc::new(AtomicBool::new(false)))
            .await
            .is_err()
    );
}
