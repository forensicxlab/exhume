use exhume_grep::{
    matcher::{ScanEvent, ScanStats},
    *,
};
use std::{
    collections::BTreeSet,
    io::{self, Read},
    sync::atomic::{AtomicBool, Ordering},
};

fn options(mode: Mode, patterns: &[&str], encodings: &[Encoding], chunk: usize) -> MatcherOptions {
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
        chunk_size: chunk,
        show_line: true,
        ..Default::default()
    }
}
fn scan(options: MatcherOptions, bytes: &[u8]) -> (ScanStats, Vec<Hit>, Vec<MatchingLine>) {
    let mut hits = Vec::new();
    let mut lines = Vec::new();
    let stats = Matcher::new(options)
        .unwrap()
        .scan(bytes, &AtomicBool::new(false), |event| {
            match event {
                ScanEvent::Hit(hit) => hits.push(hit),
                ScanEvent::MatchingLine(line) => lines.push(line),
                _ => {}
            }
            true
        })
        .unwrap();
    (stats, hits, lines)
}
fn key(encoding: Encoding, offset: u64) -> (u8, u8) {
    (
        encoding as u8,
        if encoding == Encoding::Utf8 {
            0
        } else {
            (offset % 2) as u8
        },
    )
}
// Every highlighted source byte must be matched, and every matched byte must be
// represented once in its encoding/alignment view, including clipped excerpts.
fn check_coverage(hits: &[Hit], lines: &[MatchingLine], limit: usize) {
    let expected: BTreeSet<_> = hits
        .iter()
        .flat_map(|h| {
            (h.offset..h.offset + h.length).map(move |at| (key(h.encoding, h.offset), at))
        })
        .collect();
    let mut actual = BTreeSet::new();
    for line in lines {
        assert!(line.length <= limit as u64, "{line:?}");
        assert!(line.text.len() <= limit * 8);
        assert!(!line.partial);
        assert!(!line.text.chars().any(char::is_control), "{line:?}");
        let mut end = 0;
        for span in &line.spans {
            assert!(span.start >= end && span.start < span.end);
            assert!(line.text.get(span.start..span.end).is_some());
            assert!(
                span.offset >= line.offset
                    && span.offset + span.length <= line.offset + line.length
            );
            for at in span.offset..span.offset + span.length {
                assert!(
                    actual.insert(((line.encoding as u8, line.alignment), at)),
                    "duplicate source byte: {line:?}"
                );
            }
            end = span.end;
        }
    }
    assert_eq!(actual, expected);
}

#[test]
fn grouped_lines_union_overlaps_across_every_chunk_boundary() {
    let bytes = b"head\nbanana and ana\r\nlast ana";
    for mode in [Mode::Literal, Mode::Regex] {
        for chunk in 1..48 {
            let (stats, hits, lines) = scan(
                options(mode, &["ana", "ban", "na"], &[Encoding::Utf8], chunk),
                bytes,
            );
            assert_eq!(stats.bytes_read, bytes.len() as u64);
            assert_eq!(lines.len(), 2, "{lines:?}");
            assert_eq!(
                lines.iter().map(|l| l.line_number).collect::<Vec<_>>(),
                [2, 3]
            );
            assert_eq!(lines[0].text, "banana and ana");
            assert_eq!(lines[0].line_offset, 5);
            assert_eq!(lines[1].text, "last ana");
            assert!(lines.iter().all(|l| !l.truncated_start && !l.truncated_end));
            check_coverage(&hits, &lines, 4096);
        }
    }
}

#[test]
fn multiline_and_newline_only_literals_show_every_touched_line() {
    for chunk in 1..16 {
        let (_, hits, lines) = scan(
            options(
                Mode::Literal,
                &["a\nb", "\n", "\r\n"],
                &[Encoding::Utf8],
                chunk,
            ),
            b"x a\nb c\r\n\n",
        );
        assert_eq!(
            lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            ["x a\\n", "b c\\r\\n", "\\n"]
        );
        assert_eq!(
            lines.iter().map(|l| l.line_number).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        check_coverage(&hits, &lines, 4096);
    }
}

#[test]
fn utf16_lines_preserve_alignment_surrogates_and_original_offsets() {
    for encoding in [Encoding::Utf16le, Encoding::Utf16be] {
        for prefix in [vec![], vec![0xff]] {
            let mut bytes = prefix.clone();
            for u in "head\n😀 hello héllo\r\nlast hello".encode_utf16() {
                bytes.extend_from_slice(&if encoding == Encoding::Utf16le {
                    u.to_le_bytes()
                } else {
                    u.to_be_bytes()
                });
            }
            for mode in [Mode::Literal, Mode::Regex] {
                for chunk in 1..25 {
                    let (_, hits, lines) = scan(
                        options(
                            mode,
                            &["hello", "héllo", "😀"],
                            &[Encoding::Utf8, Encoding::Utf16le, Encoding::Utf16be],
                            chunk,
                        ),
                        &bytes,
                    );
                    let selected: Vec<_> = lines
                        .iter()
                        .filter(|l| l.encoding == encoding && l.alignment as usize == prefix.len())
                        .collect();
                    assert_eq!(selected.len(), 2, "{lines:?}");
                    assert_eq!(selected[0].text, "😀 hello héllo");
                    assert_eq!(selected[0].line_number, 2);
                    assert_eq!(selected[0].line_offset, prefix.len() as u64 + 10);
                    check_coverage(&hits, &lines, 4096);
                }
            }
        }
    }
}

#[test]
fn controls_invalid_bytes_and_partial_scalars_have_safe_exact_highlights() {
    let bytes = b"a\x1b[31m\xff\0\t\r\\z";
    let (_, hits, lines) = scan(
        options(
            Mode::Regex,
            &[r"(?-u:\x1b|\xff|\x00)", "a"],
            &[Encoding::Utf8],
            1,
        ),
        bytes,
    );
    assert_eq!(lines[0].text, "a\\u{1b}[31m\\xff\\u{0}\\t\\r\\\\z");
    check_coverage(&hits, &lines, 4096);
    let (_, hits, lines) = scan(
        options(Mode::Regex, &[r"(?-u:\xa9)"], &[Encoding::Utf8], 1),
        "é".as_bytes(),
    );
    assert_eq!(lines[0].text, "\\xc3\\xa9");
    let span = &lines[0].spans[0];
    assert_eq!(&lines[0].text[span.start..span.end], "\\xa9");
    assert_eq!((span.offset, span.length), (1, 1));
    check_coverage(&hits, &lines, 4096);
}

#[test]
fn long_lines_and_long_multiline_patterns_keep_complete_search_coverage() {
    let pattern = format!("{}\n{}", "a".repeat(73), "b".repeat(83));
    let bytes = format!(
        "{}{}{}\nneedle{}needle",
        "x".repeat(377),
        pattern,
        "y".repeat(71),
        "z".repeat(361)
    );
    for chunk in [1, 3, 17, 64, 1024] {
        let mut opts = options(
            Mode::Literal,
            &[&pattern, "needle", "aaa"],
            &[Encoding::Utf8],
            chunk,
        );
        opts.line_bytes = 16;
        let (stats, hits, lines) = scan(opts, bytes.as_bytes());
        assert!(!stats.stopped);
        assert_eq!(stats.skipped_ranges, 0);
        assert!(lines.iter().any(|l| l.truncated_start));
        assert!(lines.iter().any(|l| l.truncated_end));
        check_coverage(&hits, &lines, 16);
    }
}

#[test]
fn display_limits_do_not_split_valid_unicode_scalars() {
    for encoding in [Encoding::Utf8, Encoding::Utf16le, Encoding::Utf16be] {
        let text = "abc😀é€".repeat(17);
        let bytes = if encoding == Encoding::Utf8 {
            text.into_bytes()
        } else {
            text.encode_utf16()
                .flat_map(|u| {
                    if encoding == Encoding::Utf16le {
                        u.to_le_bytes()
                    } else {
                        u.to_be_bytes()
                    }
                })
                .collect()
        };
        for mode in [Mode::Literal, Mode::Regex] {
            for chunk in [1, 2, 17, 1000] {
                let mut opts = options(mode, &["😀", "é€", "abc"], &[encoding], chunk);
                opts.line_bytes = 17;
                let (_, hits, lines) = scan(opts, &bytes);
                assert!(lines.iter().all(|l| !l.text.contains('\\')), "{lines:?}");
                check_coverage(&hits, &lines, 17);
            }
        }
    }
}

#[test]
fn stopping_after_a_hit_flushes_partial_context_without_another_read() {
    for mode in [Mode::Literal, Mode::Regex] {
        for cancel_instead in [false, true] {
            let cancel = AtomicBool::new(false);
            let mut hits = 0;
            let mut lines = Vec::new();
            let matcher = Matcher::new(options(mode, &["needle"], &[Encoding::Utf8], 8)).unwrap();
            let stats = matcher
                .scan(
                    b"a needle and more needle\nUNREAD SOMETHING".as_slice(),
                    &cancel,
                    |event| {
                        match event {
                            ScanEvent::Hit(_) => {
                                hits += 1;
                                if cancel_instead {
                                    cancel.store(true, Ordering::Relaxed);
                                } else {
                                    return false;
                                }
                            }
                            ScanEvent::MatchingLine(line) => lines.push(line),
                            _ => {}
                        }
                        true
                    },
                )
                .unwrap();
            assert!(stats.stopped);
            assert_eq!(hits, 1);
            assert_eq!(stats.bytes_read, if mode == Mode::Literal { 8 } else { 32 });
            assert_eq!(lines.len(), 1);
            assert!(lines[0].partial && lines[0].truncated_end);
            let span = &lines[0].spans[0];
            assert_eq!(&lines[0].text[span.start..span.end], "needle");
        }
    }
}

#[test]
fn read_error_keeps_available_literal_context_marked_partial() {
    struct Failing(bool);
    impl Read for Failing {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.0 {
                return Err(io::Error::other("test read failure"));
            }
            self.0 = true;
            buf[..8].copy_from_slice(b"a needle");
            Ok(8)
        }
    }
    let mut lines = Vec::new();
    let result = Matcher::new(options(Mode::Literal, &["needle"], &[Encoding::Utf8], 8))
        .unwrap()
        .scan(Failing(false), &AtomicBool::new(false), |event| {
            if let ScanEvent::MatchingLine(line) = event {
                lines.push(line);
            }
            true
        });
    assert!(result.is_err());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].partial);
    assert_eq!(lines[0].text, "a needle");
}

#[test]
fn skipped_regex_lines_still_advance_line_numbers() {
    let mut opts = options(Mode::Regex, &["needle"], &[Encoding::Utf8], 3);
    opts.max_line_bytes = 8;
    let (stats, hits, lines) = scan(opts, b"very long needle\nneedle\n");
    assert_eq!(stats.skipped_ranges, 1);
    assert_eq!(lines[0].line_number, 2);
    check_coverage(&hits, &lines, 4096);
}

#[test]
fn legacy_matcher_options_default_to_no_line_capture() {
    let mut value = serde_json::to_value(MatcherOptions::default()).unwrap();
    value.as_object_mut().unwrap().remove("show_line");
    value.as_object_mut().unwrap().remove("line_bytes");
    let options: MatcherOptions = serde_json::from_value(value).unwrap();
    assert!(!options.show_line);
    assert_eq!(options.line_bytes, 4096);
}

#[test]
fn large_context_blocks_keep_matches_across_internal_mask_boundaries() {
    let mut bytes = vec![b'x'; 150_000];
    for at in [65533, 131069] {
        bytes[at..at + 6].copy_from_slice(b"needle");
    }
    for mode in [Mode::Literal, Mode::Regex] {
        for chunk in [65536, 200000] {
            let mut opts = options(mode, &["needle"], &[Encoding::Utf8], chunk);
            opts.line_bytes = 16;
            let (stats, hits, lines) = scan(opts, &bytes);
            assert_eq!(stats.hits, 2);
            assert_eq!(stats.bytes_read, bytes.len() as u64);
            check_coverage(&hits, &lines, 16);
        }
    }
}

#[test]
fn arbitrary_bytes_keep_exact_highlight_coverage_in_all_interpretations() {
    let mut seed = 17_u64;
    for chunk in [1, 3, 17, 4096] {
        let mut bytes = Vec::new();
        for _ in 0..2048 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            bytes.push((seed >> 32) as u8);
        }
        bytes.extend_from_slice(b"needle\n\x00n\x00e\x00e\x00d\x00l\x00e\x00\n\x00");
        let mut opts = options(
            Mode::Literal,
            &["needle", "\n", "\0", "a"],
            &[Encoding::Utf8, Encoding::Utf16le, Encoding::Utf16be],
            chunk,
        );
        opts.line_bytes = 17;
        let (_, hits, lines) = scan(opts, &bytes);
        check_coverage(&hits, &lines, 17);
    }
}
