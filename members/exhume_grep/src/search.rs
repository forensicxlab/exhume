use crate::{matcher::ScanEvent, *};
use anyhow::{Context, Result, bail, ensure};
use exhume_body::Body;
use exhume_filesystem::detected_fs::{
    DetectedFile, DetectedFs, ImageStream, KeyMaterial, detect_filesystem,
    detect_filesystem_from_path,
};
use exhume_filesystem::filesystem::FileCommon;
use exhume_filesystem::{FileIdentity, FileKind, Filesystem};
use futures_util::TryStreamExt;
use regex::RegexSet;
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{
    collections::HashMap,
    io::Read,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

#[derive(Clone)]
struct Partition {
    id: i64,
    offset: u64,
    size: u64,
    key: Option<String>,
    folder: bool,
}
struct Job {
    partition: Partition,
    files: Vec<IndexedFile>,
}
struct State {
    cancel: Arc<AtomicBool>,
    stop: AtomicBool,
    bytes: AtomicU64,
    hits: AtomicU64,
    summary: Mutex<SearchSummary>,
    started: Instant,
    progress: Mutex<Instant>,
}
impl State {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed) || self.cancel.load(Ordering::Relaxed)
    }
    fn send(&self, tx: &mpsc::Sender<SearchEvent>, mut event: SearchEvent) -> bool {
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return false;
            }
            match tx.try_send(event) {
                Ok(()) => return true,
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    self.stop.store(true, Ordering::Relaxed);
                    return false;
                }
                Err(mpsc::error::TrySendError::Full(returned)) => {
                    event = returned;
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
    fn error(&self, tx: &mpsc::Sender<SearchEvent>, file: IndexedFile, message: String) {
        self.summary.lock().unwrap().failed_files += 1;
        self.send(tx, SearchEvent::FileError { file, message });
    }
}

/// The database is opened read-only and held in a read transaction for candidate
/// consistency. Partial file errors are events; invalid requests/database errors
/// return `Err`. All workers are joined before this future returns.
pub async fn search(
    request: SearchRequest,
    tx: mpsc::Sender<SearchEvent>,
    cancel: Arc<AtomicBool>,
) -> Result<SearchSummary> {
    let started = Instant::now();
    ensure!(
        (1..=64).contains(&request.workers),
        "workers must be 1..=64"
    );
    ensure!(
        request.max_hits != Some(0) && request.max_hits_per_file != Some(0),
        "hit limits must be positive"
    );
    ensure!(
        !request.files_only || !request.matcher.show_line,
        "show_line cannot be combined with files_only"
    );
    ensure!(
        request.min_size.unwrap_or(0) <= request.max_size.unwrap_or(u64::MAX),
        "minimum size exceeds maximum"
    );
    let matcher = Arc::new(Matcher::new(request.matcher.clone())?);
    let include = RegexSet::new(&request.include_paths)?;
    let exclude = RegexSet::new(&request.exclude_paths)?;
    let directory = request.directory.as_deref().map(normalize_path);
    let options = SqliteConnectOptions::new()
        .filename(&request.database)
        .read_only(true)
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(5));
    let mut db = SqliteConnection::connect_with(&options)
        .await
        .context("open existing index read-only")?;
    sqlx::query("PRAGMA temp_store=FILE")
        .execute(&mut db)
        .await?;
    sqlx::query("BEGIN").execute(&mut db).await?;
    let evidence = sqlx::query("SELECT path, type FROM evidence WHERE id = ?")
        .bind(request.evidence_id)
        .fetch_optional(&mut db)
        .await?
        .context("evidence ID does not exist")?;
    let source = request
        .source
        .clone()
        .unwrap_or_else(|| PathBuf::from(evidence.get::<String, _>("path")));
    let folder = evidence
        .get::<String, _>("type")
        .eq_ignore_ascii_case("folder");
    ensure!(
        source.exists(),
        "evidence source does not exist: {}",
        source.display()
    );
    ensure!(
        folder == source.is_dir(),
        "evidence source kind differs from indexed evidence"
    );
    let source_before = std::fs::metadata(&source)?;
    let mut partitions = HashMap::new();
    for row in sqlx::query(
        "SELECT id, kind, first_byte_addr, size_bytes, fvek FROM partitions WHERE evidence_id = ?",
    )
    .bind(request.evidence_id)
    .fetch_all(&mut db)
    .await?
    {
        let id = row.get::<i64, _>("id");
        if !request.partition_ids.is_empty() && !request.partition_ids.contains(&id) {
            continue;
        }
        let offset = row.get::<i64, _>("first_byte_addr");
        let size = row.get::<i64, _>("size_bytes");
        ensure!(
            offset >= 0 && size >= 0,
            "negative geometry for partition {id}"
        );
        let part_folder = folder || row.get::<String, _>("kind") == "folder";
        ensure!(
            part_folder || size > 0,
            "zero size for image partition {id}"
        );
        ensure!(
            part_folder == folder,
            "partition/source kind mismatch for {id}"
        );
        partitions.insert(
            id,
            Partition {
                id,
                offset: offset as u64,
                size: size as u64,
                key: row.get("fvek"),
                folder,
            },
        );
    }
    for id in &request.partition_ids {
        ensure!(
            partitions.contains_key(id),
            "partition {id} is outside the selected evidence"
        );
    }
    ensure!(!partitions.is_empty(), "no indexed partitions in scope");
    if tx
        .send(SearchEvent::Started {
            schema_version: if request.matcher.show_line { 2 } else { 1 },
            evidence_id: request.evidence_id,
            source: source.clone(),
            matcher: request.matcher.clone(),
            workers: request.workers,
        })
        .await
        .is_err()
    {
        bail!("result consumer closed before search started");
    }
    let state = Arc::new(State {
        cancel,
        stop: AtomicBool::new(false),
        bytes: AtomicU64::new(0),
        hits: AtomicU64::new(0),
        summary: Mutex::new(SearchSummary::default()),
        started,
        progress: Mutex::new(Instant::now()),
    });
    let mut guard = CancelOnDrop {
        token: state.cancel.clone(),
        armed: true,
    };
    let request = Arc::new(request);
    let mut inputs = Vec::new();
    let mut workers = Vec::new();
    for _ in 0..request.workers {
        let (input, mut receiver) = mpsc::channel::<Job>(2);
        inputs.push(input);
        let (request, matcher, state, tx, source) = (
            request.clone(),
            matcher.clone(),
            state.clone(),
            tx.clone(),
            source.clone(),
        );
        workers.push(tokio::task::spawn_blocking(move || {
            let mut opened: Option<(i64, DetectedFs<ImageStream>)> = None;
            let mut failed_partition: Option<(i64, String)> = None;
            while let Some(job) = receiver.blocking_recv() {
                if state.stopped() {
                    break;
                }
                if opened
                    .as_ref()
                    .is_none_or(|(id, _)| *id != job.partition.id)
                {
                    if failed_partition
                        .as_ref()
                        .is_none_or(|(id, _)| *id != job.partition.id)
                    {
                        match open_source(&source, &job.partition) {
                            Ok(fs) => {
                                opened = Some((job.partition.id, fs));
                                failed_partition = None;
                            }
                            Err(e) => {
                                failed_partition = Some((job.partition.id, e.to_string()));
                                opened = None;
                            }
                        }
                    }
                    if let Some((_, error)) = &failed_partition {
                        for file in job.files {
                            state.error(&tx, file, error.clone());
                        }
                        continue;
                    }
                }
                process_job(
                    &mut opened.as_mut().unwrap().1,
                    job.files,
                    &matcher,
                    &request,
                    &state,
                    &tx,
                );
            }
        }));
    }
    let enumeration: Result<()> = async {
        // One streaming query sorts at most once; keyset pagination without a
        // matching index would repeat a full inventory sort for every page.
        let mut group = Vec::new(); let mut group_key: Option<(i64, i64)> = None; let mut worker = 0;
        let mut rows = sqlx::query("SELECT id, evidence_id, partition_id, identifier, absolute_path, path_key, size, modified, created, json_extract(metadata, '$.header.sequence_number') AS generation FROM system_files WHERE evidence_id = ? AND lower(ftype) IN ('file', 'regular') ORDER BY partition_id, identifier, id")
            .bind(request.evidence_id).fetch(&mut db);
        while let Some(row) = rows.try_next().await? {
                if state.stopped() || tx.is_closed() { break; }
                let part = row.get::<i64, _>("partition_id"); let identifier = row.get::<i64, _>("identifier"); let id = row.get::<i64, _>("id");
                if !partitions.contains_key(&part) {
                    ensure!(!request.partition_ids.is_empty(), "file {id} refers to missing partition {part}");
                    continue;
                }
                let size = row.get::<i64, _>("size"); ensure!(size >= 0, "negative indexed size for file {id}");
                let path_key = row.get::<String, _>("path_key");
                if directory.as_ref().is_some_and(|dir| dir != "/" && path_key != *dir && !path_key.starts_with(&format!("{dir}/"))) { continue; }
                if (!request.include_paths.is_empty() && !include.is_match(&path_key)) || exclude.is_match(&path_key)
                    || request.min_size.is_some_and(|min| (size as u64) < min) || request.max_size.is_some_and(|max| size as u64 > max) { continue; }
                let key = (part, identifier);
                if group_key.is_some_and(|old| old != key) {
                    let previous = group_key.unwrap().0;
                    if !enqueue(&inputs[worker % inputs.len()], Job { partition: partitions[&previous].clone(), files: std::mem::take(&mut group) }, &state).await { return Ok(()); }
                    worker += 1;
                }
                group_key = Some(key);
                group.push(IndexedFile { id, evidence_id: request.evidence_id, partition_id: part, identifier: identifier as u64, generation: timestamp(row.get("generation")),
                    path: row.get("absolute_path"), size: size as u64,
                    modified: timestamp(row.get("modified")), created: timestamp(row.get("created")) });
                state.summary.lock().unwrap().selected_paths += 1;
                if state.stopped() { return Ok(()); }
        }
        if !group.is_empty() && !state.stopped() {
            let partition = partitions[&group[0].partition_id].clone();
            enqueue(&inputs[worker % inputs.len()], Job { partition, files: group }, &state).await;
        }
        Ok(())
    }.await;
    if enumeration.is_err() {
        state.stop.store(true, Ordering::Relaxed);
        state.cancel.store(true, Ordering::Relaxed);
    }
    drop(inputs);
    let mut worker_error = None;
    for worker in workers {
        if let Err(error) = worker.await {
            worker_error = Some(error);
        }
    }
    sqlx::query("ROLLBACK").execute(&mut db).await?;
    enumeration?;
    if let Some(error) = worker_error {
        return Err(error.into());
    }
    if !folder
        && std::fs::metadata(&source).ok().is_none_or(|after| {
            after.len() != source_before.len()
                || after.modified().ok() != source_before.modified().ok()
        })
    {
        state.summary.lock().unwrap().failed_files += 1;
        let event = SearchEvent::SourceError {
            evidence_id: request.evidence_id,
            message: "evidence source changed during search; all emitted hits require revalidation"
                .into(),
        };
        if state.cancel.load(Ordering::Relaxed) {
            let _ = tx.try_send(event);
        } else {
            let _ = tx.send(event).await;
        }
    }
    let mut summary = state.summary.lock().unwrap().clone();
    summary.bytes_read = state.bytes.load(Ordering::Relaxed);
    summary.hits = state.hits.load(Ordering::Relaxed);
    summary.elapsed_ms = state.started.elapsed().as_millis() as u64;
    summary.status = if state.cancel.load(Ordering::Relaxed) || tx.is_closed() {
        Status::Cancelled
    } else if summary.limited_files > 0 {
        Status::Limited
    } else if summary.failed_files > 0 || summary.skipped_ranges > 0 {
        Status::Incomplete
    } else {
        Status::Complete
    };
    // Even cancellation has a terminal summary. Consumers must keep draining
    // until Finished (or await the returned summary after closing their sink).
    if state.cancel.load(Ordering::Relaxed) {
        let _ = tokio::time::timeout(
            Duration::from_millis(100),
            tx.send(SearchEvent::Finished {
                summary: summary.clone(),
            }),
        )
        .await;
    } else {
        let _ = tx
            .send(SearchEvent::Finished {
                summary: summary.clone(),
            })
            .await;
    }
    guard.armed = false;
    Ok(summary)
}
fn timestamp(value: Option<i64>) -> Option<u64> {
    value.and_then(|n| u64::try_from(n).ok())
}
fn normalize_path(path: &str) -> String {
    let parts: Vec<_> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    format!("/{}", parts.join("/"))
}
async fn enqueue(tx: &mpsc::Sender<Job>, job: Job, state: &State) -> bool {
    let send = tx.send(job);
    tokio::pin!(send);
    loop {
        tokio::select! {
            result = &mut send => return result.is_ok(),
            _ = tokio::time::sleep(Duration::from_millis(20)) => if state.stopped() { return false; },
        }
    }
}
fn open_source(source: &std::path::Path, partition: &Partition) -> Result<DetectedFs<ImageStream>> {
    let path = source.to_str().context("evidence path is not UTF-8")?;
    if partition.folder {
        return detect_filesystem_from_path(path).map_err(|e| anyhow::anyhow!(e.to_string()));
    }
    let key = partition
        .key
        .as_ref()
        .map(|value| {
            hex::decode(value).map(|fvek| KeyMaterial {
                bitlocker_fvek: Some(fvek),
            })
        })
        .transpose()
        .context("invalid stored BitLocker key")?;
    let body =
        Body::try_new(path.to_string(), "auto").map_err(|e| anyhow::anyhow!(e.to_string()))?;
    detect_filesystem(&body, partition.offset, partition.size, key)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}
fn resolve(fs: &mut DetectedFs<ImageStream>, file: &IndexedFile) -> Result<DetectedFile> {
    let found = if matches!(fs, DetectedFs::Folder(_)) {
        fs.get_file_by_path(&file.path, file.identifier)
    } else {
        fs.get_file(file.identifier)
            .or_else(|_| fs.get_file_by_path(&file.path, file.identifier))
    }
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    ensure!(
        found.entry_kind() == FileKind::Regular,
        "indexed entry is no longer a regular file"
    );
    ensure!(
        fs.file_identifier(&found) == file.identifier,
        "file identifier changed since indexing"
    );
    ensure!(
        file.generation
            .is_none_or(|generation| generation == fs.file_identity(&found).generation),
        "file generation changed since indexing; reindex required"
    );
    let now = fs.record_to_file(&found, file.identifier, &file.path);
    ensure!(
        now.size == file.size
            && (file.modified.is_none() || now.modified == file.modified)
            && (file.created.is_none() || now.created == file.created),
        "file metadata changed since indexing; reindex required"
    );
    Ok(found)
}
fn process_job(
    fs: &mut DetectedFs<ImageStream>,
    files: Vec<IndexedFile>,
    matcher: &Matcher,
    request: &SearchRequest,
    state: &State,
    tx: &mpsc::Sender<SearchEvent>,
) {
    let mut groups: HashMap<FileIdentity, (DetectedFile, Vec<IndexedFile>)> = HashMap::new();
    for file in files {
        if state.stopped() {
            return;
        }
        match resolve(fs, &file) {
            Ok(record) => {
                let identity = fs.file_identity(&record);
                groups
                    .entry(identity)
                    .or_insert_with(|| (record, Vec::new()))
                    .1
                    .push(file);
            }
            Err(error) => state.error(tx, file, error.to_string()),
        }
    }
    for (identity, (record, mut paths)) in groups {
        if state.stopped() {
            return;
        }
        let primary = paths.remove(0);
        let group = Arc::new(FileGroup {
            file: primary,
            aliases: paths,
        });
        if !state.send(
            tx,
            SearchEvent::FileStarted {
                files: group.clone(),
                source_view: fs.source_view().as_str().into(),
            },
        ) {
            return;
        }
        let mut local_hits = 0;
        let mut local_bytes = 0;
        let mut limited = false;
        let mut skipped = false;
        let mut first_match = false;
        let mut discard_context = false;
        let mut emit = |event| {
            if let ScanEvent::MatchingLine(line) = event {
                if discard_context {
                    return false;
                }
                let event = SearchEvent::MatchingLine {
                    files: group.clone(),
                    line,
                };
                // Flush already captured context after a hit cap; cancellation
                // gets one nonblocking attempt and never waits for a consumer.
                return if state.cancel.load(Ordering::Relaxed) {
                    tx.try_send(event).is_ok()
                } else {
                    state.send(tx, event)
                };
            }
            if state.stopped() {
                // A rejected occurrence must not appear in display highlights.
                // A stopped read can still flush previously accepted hits.
                discard_context |= matches!(event, ScanEvent::Hit(_));
                return false;
            }
            match event {
                ScanEvent::BytesRead(bytes) => {
                    local_bytes += bytes;
                    state.bytes.fetch_add(bytes, Ordering::Relaxed);
                    let mut last = state.progress.lock().unwrap();
                    if last.elapsed() >= Duration::from_millis(100) {
                        *last = Instant::now();
                        let event = SearchEvent::Progress {
                            bytes_read: state.bytes.load(Ordering::Relaxed),
                            scanned_files: state.summary.lock().unwrap().scanned_files,
                            hits: state.hits.load(Ordering::Relaxed),
                        };
                        // Progress is expendable; match/error events use backpressure.
                        let _ = tx.try_send(event);
                    }
                    true
                }
                ScanEvent::Skipped(range) => {
                    skipped = true;
                    state.summary.lock().unwrap().skipped_ranges += 1;
                    state.send(
                        tx,
                        SearchEvent::SkippedRange {
                            files: group.clone(),
                            range,
                        },
                    )
                }
                ScanEvent::Hit(hit) => {
                    let reserved =
                        state
                            .hits
                            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                                if request.max_hits.is_some_and(|max| n >= max) {
                                    None
                                } else {
                                    n.checked_add(1)
                                }
                            });
                    let Ok(previous) = reserved else {
                        discard_context = true;
                        limited = true;
                        state.stop.store(true, Ordering::Relaxed);
                        return false;
                    };
                    local_hits += 1;
                    if local_hits == 1 {
                        let mut summary = state.summary.lock().unwrap();
                        summary.matched_files += 1;
                        summary
                            .first_hit_ms
                            .get_or_insert(state.started.elapsed().as_millis() as u64);
                    }
                    let event = if request.files_only {
                        SearchEvent::FileMatched {
                            files: group.clone(),
                        }
                    } else {
                        SearchEvent::Match {
                            files: group.clone(),
                            hit,
                        }
                    };
                    if !state.send(tx, event) {
                        return false;
                    }
                    if request.max_hits.is_some_and(|max| previous + 1 >= max) {
                        limited = true;
                        state.stop.store(true, Ordering::Relaxed);
                        return false;
                    }
                    if request.files_only {
                        first_match = true;
                        return false;
                    }
                    if request
                        .max_hits_per_file
                        .is_some_and(|max| local_hits >= max)
                    {
                        limited = true;
                        return false;
                    }
                    true
                }
                ScanEvent::MatchingLine(_) => unreachable!(),
            }
        };
        // Folder sources keep a verified handle open. Image sources retain the
        // resolved file and use bounded logical slices from their own reader.
        let result = match (&mut *fs, &record) {
            (DetectedFs::Folder(folder), DetectedFile::Folder(file)) => folder
                .open_verified_regular(file)
                .map_err(|e| std::io::Error::other(e.to_string()))
                .and_then(|mut reader| {
                    let before = reader.metadata()?;
                    let result =
                        matcher.scan_sized(&mut reader, group.file.size, &state.cancel, &mut emit);
                    let after = reader.metadata()?;
                    if before.len() != group.file.size
                        || before.len() != after.len()
                        || before.modified()? != after.modified()?
                    {
                        return Err(std::io::Error::other(
                            "folder file changed during search; reindex required",
                        ));
                    }
                    result
                }),
            (fs, _) => matcher.scan_sized(
                SliceReader {
                    fs,
                    file: &record,
                    size: group.file.size,
                    offset: 0,
                },
                group.file.size,
                &state.cancel,
                &mut emit,
            ),
        };
        // fs was reborrowed above; check all selected path occurrences again.
        let mut error = None;
        for file in std::iter::once(&group.file).chain(&group.aliases) {
            match resolve(fs, file) {
                Ok(found) if fs.file_identity(&found) == identity => {}
                Ok(_) => {
                    error =
                        Some("file identity changed during search; reindex required".to_string());
                    break;
                }
                Err(e) => {
                    error = Some(format!("file changed during search: {e}"));
                    break;
                }
            }
        }
        let (read, stopped) = match result {
            Ok(stats) => (stats.bytes_read, stats.stopped),
            Err(e) => {
                error = Some(e.to_string());
                (local_bytes, true)
            }
        };
        if error.is_none() && !stopped && read != group.file.size {
            error = Some(format!(
                "premature EOF: read {read} of {} indexed bytes",
                group.file.size
            ));
        }
        let failed = error.is_some();
        if let Some(message) = error {
            state.error(tx, group.file.clone(), message);
        }
        {
            let mut summary = state.summary.lock().unwrap();
            summary.scanned_files += 1;
            if limited {
                summary.limited_files += 1;
            }
        }
        let status = if state.cancel.load(Ordering::Relaxed) {
            Status::Cancelled
        } else if limited {
            Status::Limited
        } else if failed || skipped || (stopped && !first_match) {
            Status::Incomplete
        } else {
            Status::Complete
        };
        state.send(
            tx,
            SearchEvent::FileFinished {
                files: group,
                status,
                bytes_read: read,
                hits: local_hits,
            },
        );
    }
}

struct SliceReader<'a> {
    fs: &'a mut DetectedFs<ImageStream>,
    file: &'a DetectedFile,
    size: u64,
    offset: u64,
}
impl Read for SliceReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let count = self
            .size
            .saturating_sub(self.offset)
            .min(output.len() as u64) as usize;
        if count == 0 {
            return Ok(0);
        }
        let data = self
            .fs
            .read_file_slice(self.file, self.offset, count)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if data.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "premature logical file EOF",
            ));
        }
        if data.len() > count {
            return Err(std::io::Error::other(
                "filesystem returned more bytes than requested",
            ));
        }
        output[..data.len()].copy_from_slice(&data);
        self.offset += data.len() as u64;
        Ok(data.len())
    }
}

struct CancelOnDrop {
    token: Arc<AtomicBool>,
    armed: bool,
}
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.token.store(true, Ordering::Relaxed);
        }
    }
}
