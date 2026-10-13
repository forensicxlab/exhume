//! Matching over a single sequential byte source, independent of evidence I/O.
use crate::{
    MatchingLine,
    lines::{ContextLane, Contexts},
};
use aho_corasick::{AhoCorasick, MatchKind};
use anyhow::{Result, ensure};
use memchr::memmem;
use regex::bytes::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use regex_syntax::hir::{Hir, HirKind, Look};
use serde::{Deserialize, Serialize};
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    Utf8,
    Utf16le,
    Utf16be,
}
impl Encoding {
    fn encode(self, text: &str) -> Vec<u8> {
        match self {
            Self::Utf8 => text.as_bytes().to_vec(),
            _ => text
                .encode_utf16()
                .flat_map(|u| {
                    if self == Self::Utf16le {
                        u.to_le_bytes()
                    } else {
                        u.to_be_bytes()
                    }
                })
                .collect(),
        }
    }
    pub(crate) fn unit(self, bytes: [u8; 2]) -> u16 {
        if self == Self::Utf16le {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Literal,
    Regex,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    pub id: String,
    pub expression: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatcherOptions {
    pub mode: Mode,
    pub patterns: Vec<Pattern>,
    pub encodings: Vec<Encoding>,
    pub chunk_size: usize,
    /// Maximum original bytes in a regex line, excluding the LF terminator.
    pub max_line_bytes: usize,
    /// Maximum bytes of original-byte preview, zero disables previews.
    pub preview_bytes: usize,
    /// Collect grouped, safely escaped matching lines during the same read.
    #[serde(default)]
    pub show_line: bool,
    /// Maximum source bytes per display excerpt, independent of regex limits.
    #[serde(default = "default_line_bytes")]
    pub line_bytes: usize,
}
fn default_line_bytes() -> usize {
    4096
}
impl Default for MatcherOptions {
    fn default() -> Self {
        Self {
            mode: Mode::Literal,
            patterns: Vec::new(),
            encodings: vec![Encoding::Utf8],
            chunk_size: 1024 * 1024,
            max_line_bytes: 8 * 1024 * 1024,
            preview_bytes: 0,
            show_line: false,
            line_bytes: default_line_bytes(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preview {
    pub offset: u64,
    pub bytes_hex: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hit {
    pub pattern_id: String,
    pub encoding: Encoding,
    pub offset: u64,
    pub length: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<Preview>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkippedRange {
    pub offset: u64,
    pub length: u64,
    pub encoding: Encoding,
    pub alignment: u8,
    pub reason: String,
}
#[derive(Debug)]
pub enum ScanEvent {
    Hit(Hit),
    MatchingLine(MatchingLine),
    Skipped(SkippedRange),
    BytesRead(u64),
}
#[derive(Debug, Default)]
pub struct ScanStats {
    pub bytes_read: u64,
    pub hits: u64,
    pub skipped_ranges: u64,
    pub stopped: bool,
}

enum Engine {
    Single(Vec<u8>),
    Literals(AhoCorasick, Vec<(usize, Encoding)>, usize),
    Regex {
        patterns: Vec<Regex>,
        set: Option<RegexSet>,
    },
}
pub struct Matcher {
    options: MatcherOptions,
    engine: Engine,
}
impl Matcher {
    pub fn new(mut options: MatcherOptions) -> Result<Self> {
        ensure!(
            !options.patterns.is_empty() && options.patterns.len() <= 4096,
            "supply 1..=4096 patterns"
        );
        ensure!(
            options
                .patterns
                .iter()
                .map(|p| p.expression.len())
                .sum::<usize>()
                <= 1024 * 1024,
            "pattern text exceeds 1 MiB"
        );
        let mut ids = std::collections::HashSet::new();
        for pattern in &options.patterns {
            ensure!(
                !pattern.id.is_empty() && ids.insert(&pattern.id),
                "pattern IDs must be nonempty and unique"
            );
            ensure!(
                !pattern.expression.is_empty(),
                "empty patterns are unsupported"
            );
        }
        ensure!(
            options.chunk_size > 0 && options.chunk_size <= 16 * 1024 * 1024,
            "chunk size must be 1..=16 MiB"
        );
        ensure!(
            options.max_line_bytes > 0 && options.max_line_bytes <= 256 * 1024 * 1024,
            "line limit must be 1..=256 MiB"
        );
        ensure!(options.preview_bytes <= 4096, "preview limit is 4096 bytes");
        ensure!(
            (16..=65536).contains(&options.line_bytes),
            "display line bytes must be 16..=65536"
        );
        options.encodings.dedup();
        ensure!(
            !options.encodings.is_empty(),
            "at least one encoding is required"
        );
        let mut seen = Vec::new();
        options.encodings.retain(|e| {
            if seen.contains(e) {
                false
            } else {
                seen.push(*e);
                true
            }
        });
        let engine = match options.mode {
            Mode::Literal => {
                let mut needles = Vec::new();
                let mut mapping = Vec::new();
                for (id, pattern) in options.patterns.iter().enumerate() {
                    for &encoding in &options.encodings {
                        needles.push(encoding.encode(&pattern.expression));
                        mapping.push((id, encoding));
                    }
                }
                if needles.len() == 1 {
                    Engine::Single(needles.pop().unwrap())
                } else {
                    let max = needles.iter().map(Vec::len).max().unwrap();
                    let ac = AhoCorasick::builder()
                        .match_kind(MatchKind::Standard)
                        .build(&needles)?;
                    ensure!(
                        ac.memory_usage() <= 128 * 1024 * 1024,
                        "literal automaton exceeds 128 MiB"
                    );
                    Engine::Literals(ac, mapping, max)
                }
            }
            Mode::Regex => {
                let utf16 = options.encodings.iter().any(|e| *e != Encoding::Utf8);
                let budget = 64 * 1024 * 1024 / options.patterns.len();
                let mut patterns = Vec::new();
                for pattern in &options.patterns {
                    let hir = regex_syntax::ParserBuilder::new()
                        .utf8(false)
                        .multi_line(true)
                        .build()
                        .parse(&pattern.expression)?;
                    ensure!(
                        hir.properties().minimum_len().is_some_and(|len| len > 0),
                        "pattern {:?} may match zero bytes or cannot match",
                        pattern.id
                    );
                    ensure!(
                        !has_file_anchor(&hir),
                        "use ^/$ line anchors; file anchors \\A/\\z and disabling multiline anchors are unsupported"
                    );
                    ensure!(
                        !utf16 || hir.properties().is_utf8(),
                        "byte-mode regex that can split UTF-8 scalars cannot search transcoded UTF-16"
                    );
                    patterns.push(
                        RegexBuilder::new(&pattern.expression)
                            .multi_line(true)
                            .size_limit(budget)
                            .dfa_size_limit(budget.min(2 * 1024 * 1024))
                            .build()?,
                    );
                }
                let set = if patterns.len() > 1 {
                    Some(
                        RegexSetBuilder::new(options.patterns.iter().map(|p| &p.expression))
                            .multi_line(true)
                            .size_limit(32 * 1024 * 1024)
                            .dfa_size_limit(4 * 1024 * 1024)
                            .build()?,
                    )
                } else {
                    None
                };
                Engine::Regex { patterns, set }
            }
        };
        Ok(Self { options, engine })
    }
    pub fn options(&self) -> &MatcherOptions {
        &self.options
    }

    /// Return false from `emit` to stop (first match, limits, or a closed sink).
    /// With `show_line`, already captured partial line events may follow a stop;
    /// no additional reads are made. Return false on those to stop the flush.
    /// Regex calls are synchronous: cancellation is checked between reads,
    /// lines, patterns and matches, not inside one regex engine invocation.
    pub fn scan<R: Read>(
        &self,
        reader: R,
        cancel: &AtomicBool,
        emit: impl FnMut(ScanEvent) -> bool,
    ) -> io::Result<ScanStats> {
        self.scan_buffered(reader, self.options.chunk_size, cancel, emit)
    }

    // The index supplies the logical length. Small files should not allocate
    // and zero a full MiB per occurrence. Take also bounds a growing host file.
    pub(crate) fn scan_sized<R: Read>(
        &self,
        reader: R,
        size: u64,
        cancel: &AtomicBool,
        emit: impl FnMut(ScanEvent) -> bool,
    ) -> io::Result<ScanStats> {
        let chunk = size.min(self.options.chunk_size as u64).max(1) as usize;
        self.scan_buffered(reader.take(size), chunk, cancel, emit)
    }

    fn scan_buffered<R: Read>(
        &self,
        mut reader: R,
        chunk_size: usize,
        cancel: &AtomicBool,
        mut emit: impl FnMut(ScanEvent) -> bool,
    ) -> io::Result<ScanStats> {
        let mut stats = ScanStats::default();
        if let Engine::Regex { .. } = &self.engine {
            let mut buffer = vec![0; chunk_size];
            let mut lanes = Vec::new();
            for &encoding in &self.options.encodings {
                lanes.push(Line::new(encoding, 0));
                if encoding != Encoding::Utf8 {
                    lanes.push(Line::new(encoding, 1));
                }
            }
            loop {
                if cancel.load(Ordering::Relaxed) {
                    stats.stopped = true;
                    break;
                }
                let n = read_retry(&mut reader, &mut buffer)?;
                if n == 0 {
                    for lane in &mut lanes {
                        if !self.flush_line(lane, false, &mut stats, cancel, &mut emit) {
                            stats.stopped = true;
                            break;
                        }
                    }
                    break;
                }
                let base = stats.bytes_read;
                stats.bytes_read += n as u64;
                if !emit(ScanEvent::BytesRead(n as u64)) {
                    stats.stopped = true;
                    break;
                }
                for lane in &mut lanes {
                    if !self.feed_line(lane, &buffer[..n], base, &mut stats, cancel, &mut emit) {
                        stats.stopped = true;
                        break;
                    }
                }
                if stats.stopped {
                    break;
                }
            }
        } else {
            let max = match &self.engine {
                Engine::Single(p) => p.len(),
                Engine::Literals(_, _, max) => *max,
                _ => unreachable!(),
            };
            let mut window = vec![0; chunk_size + max.saturating_sub(1)];
            let mut tail = 0;
            let mut contexts = self
                .options
                .show_line
                .then(|| Contexts::new(&self.options.encodings, self.options.line_bytes));
            let mut sampled = false;
            let mut absent_byte = None;
            // The optimized finder is reused, rather than rebuilt for each chunk.
            let finder = if let Engine::Single(p) = &self.engine {
                Some(memmem::Finder::new(p))
            } else {
                None
            };
            loop {
                if cancel.load(Ordering::Relaxed) {
                    stats.stopped = true;
                    finish_contexts(
                        &mut contexts,
                        &window[..tail],
                        stats.bytes_read - tail as u64,
                        true,
                        &mut emit,
                    );
                    break;
                }
                let n = match read_retry(&mut reader, &mut window[tail..tail + chunk_size]) {
                    Ok(n) => n,
                    Err(error) => {
                        finish_contexts(
                            &mut contexts,
                            &window[..tail],
                            stats.bytes_read - tail as u64,
                            true,
                            &mut emit,
                        );
                        return Err(error);
                    }
                };
                if n == 0 {
                    if !finish_contexts(
                        &mut contexts,
                        &window[..tail],
                        stats.bytes_read - tail as u64,
                        false,
                        &mut emit,
                    ) {
                        stats.stopped = true;
                    }
                    break;
                }
                let filled = tail + n;
                let base = stats.bytes_read - tail as u64;
                stats.bytes_read += n as u64;
                if !emit(ScanEvent::BytesRead(n as u64)) {
                    stats.stopped = true;
                    finish_contexts(&mut contexts, &window[..filled], base, true, &mut emit);
                    break;
                }
                let mut found = |id, encoding, start, end| {
                    if end <= tail {
                        return true;
                    }
                    if cancel.load(Ordering::Relaxed) {
                        return false;
                    }
                    stats.hits += 1;
                    let hit = self.hit(
                        id,
                        encoding,
                        base + start as u64,
                        (end - start) as u64,
                        &window[..filled],
                        base,
                    );
                    if let Some(contexts) = &mut contexts {
                        contexts.record(&hit);
                    }
                    emit(ScanEvent::Hit(hit))
                };
                match &self.engine {
                    Engine::Single(p) => {
                        // A missing needle byte proves absence. Select a probe
                        // once from a small sample, then disable it permanently
                        // on its first false positive to bound extra scanning.
                        if !sampled {
                            let mut present = [false; 256];
                            for &byte in &window[..filled.min(4096)] {
                                present[byte as usize] = true;
                            }
                            absent_byte = p.iter().copied().find(|byte| !present[*byte as usize]);
                            sampled = true;
                        }
                        let reject = absent_byte
                            .is_some_and(|byte| memchr::memchr(byte, &window[..filled]).is_none());
                        if !reject {
                            absent_byte = None;
                            let mut at = 0;
                            while let Some(relative) =
                                finder.as_ref().unwrap().find(&window[at..filled])
                            {
                                let start = at + relative;
                                if !found(0, self.options.encodings[0], start, start + p.len()) {
                                    stats.stopped = true;
                                    break;
                                }
                                at = start + 1;
                            }
                        }
                    }
                    Engine::Literals(ac, mapping, _) => {
                        for m in ac.find_overlapping_iter(&window[..filled]) {
                            let (id, encoding) = mapping[m.pattern().as_usize()];
                            if !found(id, encoding, m.start(), m.end()) {
                                stats.stopped = true;
                                break;
                            }
                        }
                    }
                    _ => unreachable!(),
                }
                if stats.stopped {
                    finish_contexts(&mut contexts, &window[..filled], base, true, &mut emit);
                    break;
                }
                tail = (max - 1).min(filled);
                // A future literal cannot begin before this watermark. Delay
                // line display until every possible overlapping hit is known.
                if let Some(contexts) = &mut contexts
                    && !contexts.feed(&window[..filled - tail], base, false, &mut |line| {
                        emit(ScanEvent::MatchingLine(line))
                    })
                {
                    stats.stopped = true;
                    break;
                }
                window.copy_within(filled - tail..filled, 0);
            }
        }
        Ok(stats)
    }
    fn hit(
        &self,
        id: usize,
        encoding: Encoding,
        offset: u64,
        length: u64,
        source: &[u8],
        source_base: u64,
    ) -> Hit {
        let preview = if self.options.preview_bytes == 0 {
            None
        } else {
            let start = (offset - source_base) as usize;
            let from = start.saturating_sub(self.options.preview_bytes / 4);
            let to = from
                .saturating_add(self.options.preview_bytes)
                .min(source.len());
            Some(Preview {
                offset: source_base + from as u64,
                bytes_hex: hex::encode(&source[from..to]),
            })
        };
        Hit {
            pattern_id: self.options.patterns[id].id.clone(),
            encoding,
            offset,
            length,
            preview,
        }
    }
    fn feed_line(
        &self,
        lane: &mut Line,
        bytes: &[u8],
        base: u64,
        stats: &mut ScanStats,
        cancel: &AtomicBool,
        emit: &mut impl FnMut(ScanEvent) -> bool,
    ) -> bool {
        if lane.encoding == Encoding::Utf8 {
            let mut from = 0;
            for end in memchr::memchr_iter(b'\n', bytes) {
                lane.append(&bytes[from..end], self.options.max_line_bytes);
                if !self.flush_line(lane, true, stats, cancel, emit) {
                    return false;
                }
                lane.start = base + end as u64 + 1;
                from = end + 1;
            }
            lane.append(&bytes[from..], self.options.max_line_bytes);
        } else {
            for (index, &byte) in bytes.iter().enumerate() {
                if base + (index as u64) < lane.alignment as u64 {
                    continue;
                }
                if let Some(first) = lane.pending.take() {
                    let pair = [first, byte];
                    if lane.encoding.unit(pair) == 10 {
                        if !self.flush_line(lane, true, stats, cancel, emit) {
                            return false;
                        }
                        lane.start = base + index as u64 + 1;
                    } else {
                        lane.append(&pair, self.options.max_line_bytes);
                    }
                } else {
                    lane.pending = Some(byte);
                }
            }
        }
        !cancel.load(Ordering::Relaxed)
    }
    fn flush_line(
        &self,
        lane: &mut Line,
        terminated: bool,
        stats: &mut ScanStats,
        cancel: &AtomicBool,
        emit: &mut impl FnMut(ScanEvent) -> bool,
    ) -> bool {
        if let Some(byte) = lane.pending.take() {
            lane.append(&[byte], self.options.max_line_bytes);
        }
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        if lane.skipping {
            stats.skipped_ranges += 1;
            let ok = emit(ScanEvent::Skipped(SkippedRange {
                offset: lane.start,
                length: lane.length,
                encoding: lane.encoding,
                alignment: lane.alignment,
                reason: "regex_line_limit".into(),
            }));
            lane.clear();
            return ok;
        }
        let mut raw = lane.data.as_slice();
        // CRLF is one line terminator. A lone CR remains searchable content.
        if terminated && lane.encoding == Encoding::Utf8 && raw.last() == Some(&b'\r') {
            raw = &raw[..raw.len() - 1];
        } else if terminated
            && lane.encoding != Encoding::Utf8
            && raw.len() >= 2
            && lane.encoding.unit([raw[raw.len() - 2], raw[raw.len() - 1]]) == 13
        {
            raw = &raw[..raw.len() - 2];
        }
        let decoded;
        let haystack = if lane.encoding == Encoding::Utf8 {
            raw
        } else {
            decoded = decode_utf16(raw, lane.encoding);
            &decoded
        };
        let Engine::Regex { patterns, set } = &self.engine else {
            unreachable!()
        };
        if set.as_ref().is_some_and(|set| !set.is_match(haystack)) {
            lane.clear();
            return true;
        }
        let selected = set.as_ref().map(|set| set.matches(haystack));
        let mut context = self.options.show_line.then(|| {
            ContextLane::new(
                lane.encoding,
                lane.alignment,
                lane.number,
                lane.start,
                self.options.line_bytes,
            )
        });
        let complete = (|| {
            for (id, pattern) in patterns.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    return false;
                }
                if selected
                    .as_ref()
                    .is_some_and(|matches| !matches.matched(id))
                {
                    continue;
                }
                let mut previous = 0;
                let mut original = 0;
                for m in pattern.find_iter(haystack) {
                    if cancel.load(Ordering::Relaxed) {
                        return false;
                    }
                    let (start, length) = if lane.encoding == Encoding::Utf8 {
                        (m.start(), m.len())
                    } else {
                        original += utf16_source_len(&haystack[previous..m.start()]);
                        let start = original;
                        let length = utf16_source_len(&haystack[m.start()..m.end()]);
                        original += length;
                        previous = m.end();
                        (start, length)
                    };
                    stats.hits += 1;
                    let hit = self.hit(
                        id,
                        lane.encoding,
                        lane.start + start as u64,
                        length as u64,
                        raw,
                        lane.start,
                    );
                    if let Some(context) = &mut context {
                        context.record(&hit);
                    }
                    if !emit(ScanEvent::Hit(hit)) {
                        return false;
                    }
                }
            }
            true
        })();
        let displayed = context.as_mut().is_none_or(|context| {
            let mut line_emit = |line| emit(ScanEvent::MatchingLine(line));
            context.feed(raw, lane.start, !complete, &mut line_emit)
                && context.finish(!complete, &mut line_emit)
        });
        lane.clear();
        complete && displayed
    }
}
fn finish_contexts(
    contexts: &mut Option<Contexts>,
    bytes: &[u8],
    base: u64,
    partial: bool,
    emit: &mut impl FnMut(ScanEvent) -> bool,
) -> bool {
    contexts.as_mut().is_none_or(|contexts| {
        let mut line_emit = |line| emit(ScanEvent::MatchingLine(line));
        contexts.feed(bytes, base, partial, &mut line_emit)
            && contexts.finish(partial, &mut line_emit)
    })
}
fn read_retry(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        match reader.read(buffer) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            other => return other,
        }
    }
}
fn has_file_anchor(hir: &Hir) -> bool {
    match hir.kind() {
        HirKind::Look(Look::Start | Look::End) => true,
        _ => hir.kind().subs().iter().any(has_file_anchor),
    }
}
struct Line {
    encoding: Encoding,
    alignment: u8,
    start: u64,
    length: u64,
    data: Vec<u8>,
    pending: Option<u8>,
    skipping: bool,
    number: u64,
}
impl Line {
    fn new(encoding: Encoding, alignment: u8) -> Self {
        Self {
            encoding,
            alignment,
            start: alignment as u64,
            length: 0,
            data: Vec::new(),
            pending: None,
            skipping: false,
            number: 1,
        }
    }
    fn append(&mut self, data: &[u8], maximum: usize) {
        self.length += data.len() as u64;
        if self.length > maximum as u64 {
            self.skipping = true;
            self.data.clear();
        } else if !self.skipping {
            self.data.extend_from_slice(data);
        }
    }
    fn clear(&mut self) {
        self.number += 1;
        self.length = 0;
        self.data.clear();
        self.skipping = false;
    }
}
// Invalid units become invalid UTF-8 bytes, never replacement characters or
// deleted text. Unicode regex atoms cannot match across those barriers.
fn decode_utf16(raw: &[u8], encoding: Encoding) -> Vec<u8> {
    let units = raw.chunks_exact(2).map(|p| encoding.unit([p[0], p[1]]));
    let mut output = Vec::with_capacity(raw.len());
    for scalar in char::decode_utf16(units) {
        match scalar {
            Ok(c) => {
                let mut buf = [0; 4];
                output.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
            Err(_) => output.push(0xff),
        }
    }
    if !raw.len().is_multiple_of(2) {
        output.push(0xff);
    }
    output
}
fn utf16_source_len(bytes: &[u8]) -> usize {
    let mut length = 0;
    let mut at = 0;
    while at < bytes.len() {
        let first = bytes[at];
        let width = match first {
            0..=127 | 255 => 1,
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            _ => 4,
        };
        length += if width == 4 { 4 } else { 2 };
        at += width;
    }
    length
}
