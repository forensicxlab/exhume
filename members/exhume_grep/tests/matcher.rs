use exhume_grep::{matcher::ScanEvent, *};
use std::{
    io::{self, Read},
    sync::atomic::{AtomicBool, Ordering},
};
fn options(
    mode: Mode,
    patterns: &[&str],
    encodings: &[Encoding],
    chunk_size: usize,
) -> MatcherOptions {
    MatcherOptions {
        mode,
        patterns: patterns
            .iter()
            .enumerate()
            .map(|(i, p)| Pattern {
                id: i.to_string(),
                expression: p.to_string(),
            })
            .collect(),
        encodings: encodings.to_vec(),
        chunk_size,
        ..Default::default()
    }
}
fn run(options: MatcherOptions, bytes: &[u8]) -> (Vec<Hit>, Vec<SkippedRange>) {
    let matcher = Matcher::new(options).unwrap();
    let mut hits = Vec::new();
    let mut skipped = Vec::new();
    let stats = matcher
        .scan(bytes, &AtomicBool::new(false), |e| {
            match e {
                ScanEvent::Hit(h) => hits.push(h),
                ScanEvent::Skipped(s) => skipped.push(s),
                _ => {}
            };
            true
        })
        .unwrap();
    assert_eq!(stats.bytes_read, bytes.len() as u64);
    (hits, skipped)
}
#[test]
fn overlapping_literals_are_independent_of_every_chunk_boundary() {
    let bytes = b"bananana\0banana\nana";
    for chunk in 1..24 {
        let (hits, _) = run(
            options(
                Mode::Literal,
                &["ana", "banana", "ana"],
                &[Encoding::Utf8],
                chunk,
            ),
            bytes,
        );
        for (id, pattern) in [b"ana".as_slice(), b"banana", b"ana"].iter().enumerate() {
            let expected: Vec<_> = bytes
                .windows(pattern.len())
                .enumerate()
                .filter(|(_, w)| w == pattern)
                .map(|(i, _)| i as u64)
                .collect();
            let actual: Vec<_> = hits
                .iter()
                .filter(|h| h.pattern_id == id.to_string())
                .map(|h| h.offset)
                .collect();
            assert_eq!(actual, expected, "chunk={chunk} pattern={id}");
        }
    }
}
#[test]
fn utf16_regex_offsets_surrogates_crlf_odd_alignment_and_case() {
    for encoding in [Encoding::Utf16le, Encoding::Utf16be] {
        for prefix in [Vec::new(), vec![0xff]] {
            let mut bytes = prefix.clone();
            for unit in "A€😀HELLO\r\nhello!".encode_utf16() {
                bytes.extend_from_slice(&if encoding == Encoding::Utf16le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            for chunk in 1..20 {
                let (hits, _) = run(
                    options(
                        Mode::Regex,
                        &["(?i)hello", "€😀", "^hello!$"],
                        &[encoding],
                        chunk,
                    ),
                    &bytes,
                );
                let spans: Vec<_> = hits
                    .iter()
                    .filter(|h| h.pattern_id == "0")
                    .map(|h| (h.offset, h.length))
                    .collect();
                assert_eq!(
                    spans,
                    vec![
                        (prefix.len() as u64 + 8, 10),
                        (prefix.len() as u64 + 22, 10)
                    ],
                    "chunk={chunk}"
                );
                assert!(hits.iter().any(|h| h.pattern_id == "1"
                    && h.offset == prefix.len() as u64 + 2
                    && h.length == 6));
                assert!(hits.iter().any(|h| h.pattern_id == "2"
                    && h.offset == prefix.len() as u64 + 22
                    && h.length == 12));
            }
        }
    }
}
#[test]
fn literal_all_encodings_and_newlines_have_original_offsets() {
    let bytes = [
        b"xfoo\nbar".as_slice(),
        &[0xff],
        &"foo\nbar"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    ]
    .concat();
    let (hits, _) = run(
        options(
            Mode::Literal,
            &["foo\nbar"],
            &[Encoding::Utf8, Encoding::Utf16le, Encoding::Utf16be],
            3,
        ),
        &bytes,
    );
    assert!(
        hits.iter()
            .any(|h| h.encoding == Encoding::Utf8 && h.offset == 1 && h.length == 7)
    );
    assert!(
        hits.iter()
            .any(|h| h.encoding == Encoding::Utf16le && h.offset == 9 && h.length == 14)
    );
}
#[test]
fn malformed_utf16_is_not_replaced_or_removed() {
    let bytes = [b'a', 0, 0, 0xd8, b'b', 0, b'c', 0, 0xff];
    let (hits, _) = run(
        options(Mode::Regex, &["ab", "�", "bc"], &[Encoding::Utf16le], 1),
        &bytes,
    );
    assert!(
        !hits
            .iter()
            .any(|h| h.pattern_id == "0" || h.pattern_id == "1")
    );
    assert!(
        hits.iter()
            .any(|h| h.pattern_id == "2" && h.offset == 4 && h.length == 4)
    );
}
#[test]
fn long_line_is_skipped_whole_then_next_line_is_searched() {
    let mut o = options(Mode::Regex, &["hit"], &[Encoding::Utf8], 2);
    o.max_line_bytes = 5;
    let (hits, skipped) = run(o, b"hitxxxxhit\nhit\nhit\r");
    assert_eq!(hits.iter().map(|h| h.offset).collect::<Vec<_>>(), [11, 15]);
    assert_eq!(skipped.len(), 1);
    assert_eq!((skipped[0].offset, skipped[0].length), (0, 10));
}
#[test]
fn regex_lines_anchors_nonoverlap_and_lone_cr() {
    let (hits, _) = run(
        options(
            Mode::Regex,
            &["ana", "^banana$", "x\\r$"],
            &[Encoding::Utf8],
            1,
        ),
        b"banana\r\nx\r",
    );
    assert_eq!(hits.iter().filter(|h| h.pattern_id == "0").count(), 1);
    assert!(hits.iter().any(|h| h.pattern_id == "1" && h.offset == 0));
    assert!(
        hits.iter()
            .any(|h| h.pattern_id == "2" && h.offset == 8 && h.length == 2)
    );
}
#[test]
fn invalid_queries_fail_before_reading() {
    for expression in [
        "", "a*", "^", "\\b", "x|", "(?=x)", "(a)\\1", "\\Aabc", "abc\\z",
    ] {
        assert!(
            Matcher::new(options(Mode::Regex, &[expression], &[Encoding::Utf8], 64)).is_err(),
            "{expression}"
        );
    }
    assert!(Matcher::new(options(Mode::Regex, &["(?-u:.)"], &[Encoding::Utf16le], 64)).is_err());
    assert!(Matcher::new(options(Mode::Regex, &["(?i)Ł"], &[Encoding::Utf16le], 64)).is_ok());
    let (hits, _) = run(
        options(Mode::Regex, &["(?i)Ł"], &[Encoding::Utf16le], 1),
        &[0x61, 0x01],
    );
    assert!(hits.is_empty());
}
#[test]
fn read_errors_and_cancellation_are_not_clean_eof() {
    struct Bad;
    impl Read for Bad {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("injected failure"))
        }
    }
    let matcher = Matcher::new(options(Mode::Literal, &["x"], &[Encoding::Utf8], 4)).unwrap();
    assert!(
        matcher
            .scan(Bad, &AtomicBool::new(false), |_| true)
            .is_err()
    );
    let cancel = AtomicBool::new(false);
    let stats = matcher
        .scan(b"xxxxxxxx".as_slice(), &cancel, |event| {
            if matches!(event, ScanEvent::Hit(_)) {
                cancel.store(true, Ordering::Relaxed);
            };
            true
        })
        .unwrap();
    assert!(stats.stopped);
    assert_eq!(stats.hits, 1);
}
#[test]
fn preview_contains_only_original_bytes_and_correct_position() {
    let mut o = options(Mode::Regex, &["€"], &[Encoding::Utf16le], 3);
    o.preview_bytes = 4;
    let (hits, _) = run(o, &[65, 0, 0xac, 0x20, 66, 0]);
    let h = hits.iter().find(|h| h.offset == 2).unwrap();
    let p = h.preview.as_ref().unwrap();
    assert_eq!(p.offset, 1);
    assert_eq!(p.bytes_hex, "00ac2042");
}

#[test]
fn single_literal_probe_and_boundary_matches_agree_with_naive_search() {
    let bytes = [
        vec![b'x'; 5000],
        b"nxxxxxneedle--needleneedle\nneedle".to_vec(),
    ]
    .concat();
    for chunk in [1, 3, 7, 127, 1024, 4096, 8192] {
        let (hits, _) = run(
            options(Mode::Literal, &["needle"], &[Encoding::Utf8], chunk),
            &bytes,
        );
        let expected: Vec<_> = bytes
            .windows(6)
            .enumerate()
            .filter(|(_, w)| *w == b"needle")
            .map(|(i, _)| i as u64)
            .collect();
        assert_eq!(hits.iter().map(|h| h.offset).collect::<Vec<_>>(), expected);
    }
}

#[test]
fn packed_filesystem_identifiers_survive_json_without_js_precision_loss() {
    let file = exhume_grep::IndexedFile {
        id: 1,
        evidence_id: 1,
        partition_id: 1,
        identifier: (1u64 << 56) + 123,
        generation: None,
        path: "/volume_1/file".into(),
        size: 0,
        modified: None,
        created: None,
    };
    let value = serde_json::to_value(&file).unwrap();
    assert_eq!(value["identifier"].as_str(), Some("72057594037928059"));
    let decoded: exhume_grep::IndexedFile = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.identifier, file.identifier);
}
