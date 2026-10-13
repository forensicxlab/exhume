//! Optional, bounded presentation context. All coordinates refer to source bytes.
use crate::{Encoding, Hit};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A union of matching bytes, mapped into the escaped UTF-8 `text` string.
/// `start`/`end` are UTF-8 byte indices, not character or UTF-16 indices.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineSpan {
    pub start: usize,
    pub end: usize,
    pub offset: u64,
    pub length: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchingLine {
    pub encoding: Encoding,
    pub alignment: u8,
    pub line_number: u64,
    pub line_offset: u64,
    /// Start and length of this excerpt in the original logical file.
    pub offset: u64,
    pub length: u64,
    /// Safe, escaped display text, without ANSI. Not a lossless text extraction.
    pub text: String,
    pub spans: Vec<LineSpan>,
    pub truncated_start: bool,
    pub truncated_end: bool,
    /// Scanning stopped before all matches/context could be collected.
    pub partial: bool,
}

pub(crate) struct Contexts {
    lanes: Vec<ContextLane>,
}
impl Contexts {
    pub fn new(encodings: &[Encoding], limit: usize) -> Self {
        let mut lanes = Vec::new();
        for &encoding in encodings {
            lanes.push(ContextLane::new(encoding, 0, 1, 0, limit));
            if encoding != Encoding::Utf8 {
                lanes.push(ContextLane::new(encoding, 1, 1, 1, limit));
            }
        }
        Self { lanes }
    }
    pub fn record(&mut self, hit: &Hit) {
        for lane in &mut self.lanes {
            if hit.encoding == lane.encoding
                && (hit.encoding == Encoding::Utf8 || hit.offset % 2 == lane.alignment as u64)
            {
                lane.record(hit);
            }
        }
    }
    pub fn feed(
        &mut self,
        bytes: &[u8],
        base: u64,
        partial: bool,
        emit: &mut impl FnMut(MatchingLine) -> bool,
    ) -> bool {
        self.lanes
            .iter_mut()
            .all(|lane| lane.feed(bytes, base, partial, emit))
    }
    pub fn finish(&mut self, partial: bool, emit: &mut impl FnMut(MatchingLine) -> bool) -> bool {
        self.lanes.iter_mut().all(|lane| lane.finish(partial, emit))
    }
}

pub(crate) struct ContextLane {
    encoding: Encoding,
    alignment: u8,
    number: u64,
    line_start: u64,
    offset: u64,
    limit: usize,
    raw: Vec<u8>,
    marked: Vec<u8>,
    pending: Option<(u8, u8, u64)>,
    // Merge overlaps as hits arrive; never retain one allocation per occurrence.
    ranges: BTreeMap<u64, u64>,
}
impl ContextLane {
    pub fn new(encoding: Encoding, alignment: u8, number: u64, start: u64, limit: usize) -> Self {
        Self {
            encoding,
            alignment,
            number,
            line_start: start,
            offset: start,
            limit,
            raw: Vec::new(),
            marked: Vec::new(),
            pending: None,
            ranges: BTreeMap::new(),
        }
    }
    pub fn record(&mut self, hit: &Hit) {
        let mut start = hit.offset;
        let mut end = start + hit.length;
        if let Some((&left, &right)) = self.ranges.range(..=start).next_back()
            && right >= start
        {
            start = left;
            end = end.max(right);
            self.ranges.remove(&left);
        }
        while let Some((&left, &right)) = self.ranges.range(start..=end).next() {
            end = end.max(right);
            self.ranges.remove(&left);
        }
        self.ranges.insert(start, end);
    }
    pub fn feed(
        &mut self,
        bytes: &[u8],
        base: u64,
        partial: bool,
        emit: &mut impl FnMut(MatchingLine) -> bool,
    ) -> bool {
        // Regex lines can be much larger than a display excerpt. Bound the
        // transient byte mask independently of that search-line ceiling.
        bytes
            .chunks(65536)
            .enumerate()
            .all(|(i, block)| self.feed_block(block, base + (i * 65536) as u64, partial, emit))
    }
    fn feed_block(
        &mut self,
        bytes: &[u8],
        base: u64,
        partial: bool,
        emit: &mut impl FnMut(MatchingLine) -> bool,
    ) -> bool {
        let end = base + bytes.len() as u64;
        let mut marks = vec![0; bytes.len()];
        for (&left, &right) in self.ranges.range(..end) {
            if right > base {
                marks[(left.max(base) - base) as usize..(right.min(end) - base) as usize].fill(1);
            }
        }
        while self
            .ranges
            .first_key_value()
            .is_some_and(|(_, right)| *right <= end)
        {
            self.ranges.pop_first();
        }
        if self.encoding == Encoding::Utf8 {
            let mut from = 0;
            for end in memchr::memchr_iter(b'\n', bytes) {
                if !self.content(
                    &bytes[from..end],
                    &marks[from..end],
                    base + from as u64,
                    partial,
                    emit,
                ) || !self.unit(
                    &bytes[end..end + 1],
                    &marks[end..end + 1],
                    base + end as u64,
                    true,
                    partial,
                    emit,
                ) {
                    return false;
                }
                from = end + 1;
            }
            return self.content(
                &bytes[from..],
                &marks[from..],
                base + from as u64,
                partial,
                emit,
            );
        }
        for (i, (&byte, &marked)) in bytes.iter().zip(&marks).enumerate() {
            let at = base + i as u64;
            if at < self.alignment as u64 {
                continue;
            }
            if let Some((first, first_mark, offset)) = self.pending.take() {
                let pair = [first, byte];
                let lf = self.encoding.unit(pair) == 10;
                if !self.unit(&pair, &[first_mark, marked], offset, lf, partial, emit) {
                    return false;
                }
            } else {
                self.pending = Some((byte, marked, at));
            }
        }
        true
    }
    fn content(
        &mut self,
        mut bytes: &[u8],
        mut marks: &[u8],
        mut at: u64,
        partial: bool,
        emit: &mut impl FnMut(MatchingLine) -> bool,
    ) -> bool {
        while !bytes.is_empty() {
            let n = bytes.len().min(self.limit + 1 - self.raw.len());
            if !self.unit(&bytes[..n], &marks[..n], at, false, partial, emit) {
                return false;
            }
            bytes = &bytes[n..];
            marks = &marks[n..];
            at += n as u64;
        }
        true
    }
    fn unit(
        &mut self,
        bytes: &[u8],
        marks: &[u8],
        at: u64,
        lf: bool,
        partial: bool,
        emit: &mut impl FnMut(MatchingLine) -> bool,
    ) -> bool {
        // Delimiters are only displayed when matched. A matched LF is visibly
        // escaped, including on an otherwise empty line.
        if !lf || marks.contains(&1) {
            if self.raw.is_empty() {
                self.offset = at;
            }
            self.raw.extend_from_slice(bytes);
            self.marked.extend_from_slice(marks);
        } else {
            let width = if self.encoding == Encoding::Utf8 {
                1
            } else {
                2
            };
            if self.raw.len() >= width {
                let from = self.raw.len() - width;
                let cr = if width == 1 {
                    self.raw[from] == b'\r'
                } else {
                    self.encoding.unit([self.raw[from], self.raw[from + 1]]) == 13
                };
                if cr && !self.marked[from..].contains(&1) {
                    self.raw.truncate(from);
                    self.marked.truncate(from);
                }
            }
        }
        while self.raw.len() > self.limit {
            let cut = safe_cut(&self.raw, self.limit, self.encoding);
            if self.marked[..cut].contains(&1) {
                if !self.output(cut, true, partial, emit) {
                    return false;
                }
                self.discard(cut);
            } else {
                // Keep some preceding context when a hit arrives late in a
                // huge line, rather than retaining the entire unmatched prefix.
                let keep_from = safe_cut(&self.raw, cut - self.limit / 4, self.encoding);
                self.discard(keep_from);
            }
        }
        if lf {
            if !self.output(self.raw.len(), partial, partial, emit) {
                return false;
            }
            self.raw.clear();
            self.marked.clear();
            self.number += 1;
            self.line_start = at + bytes.len() as u64;
            self.offset = self.line_start;
        }
        true
    }
    fn discard(&mut self, length: usize) {
        self.raw.drain(..length);
        self.marked.drain(..length);
        self.offset += length as u64;
    }
    pub fn finish(&mut self, partial: bool, emit: &mut impl FnMut(MatchingLine) -> bool) -> bool {
        if let Some((byte, mark, at)) = self.pending.take()
            && !self.unit(&[byte], &[mark], at, false, partial, emit)
        {
            return false;
        }
        let ok = self.output(self.raw.len(), partial, partial, emit);
        self.raw.clear();
        self.marked.clear();
        ok
    }
    fn output(
        &self,
        length: usize,
        truncated_end: bool,
        partial: bool,
        emit: &mut impl FnMut(MatchingLine) -> bool,
    ) -> bool {
        if !self.marked[..length].contains(&1) {
            return true;
        }
        let (text, spans) = escaped(
            &self.raw[..length],
            &self.marked[..length],
            self.offset,
            self.encoding,
        );
        emit(MatchingLine {
            encoding: self.encoding,
            alignment: self.alignment,
            line_number: self.number,
            line_offset: self.line_start,
            offset: self.offset,
            length: length as u64,
            text,
            spans,
            truncated_start: self.offset > self.line_start,
            truncated_end,
            partial,
        })
    }
}

// Keep a valid scalar together across excerpt boundaries; malformed bytes are
// represented explicitly by escaped(), so they need no replacement character.
fn safe_cut(raw: &[u8], mut cut: usize, encoding: Encoding) -> usize {
    if encoding != Encoding::Utf8 {
        cut -= cut % 2;
        if cut >= 2 && (0xd800..=0xdbff).contains(&encoding.unit([raw[cut - 2], raw[cut - 1]])) {
            cut -= 2;
        }
    } else {
        for from in cut.saturating_sub(3)..cut {
            let width = utf8_width(raw[from]);
            if width > 1
                && from + width > cut
                && raw[from + 1..cut].iter().all(|b| (0x80..=0xbf).contains(b))
            {
                return from;
            }
        }
    }
    cut
}
fn utf8_width(byte: u8) -> usize {
    match byte {
        0..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 1,
    }
}
fn safe_char(c: char) -> String {
    match c {
        '\\' => "\\\\".into(),
        '\n' => "\\n".into(),
        '\r' => "\\r".into(),
        '\t' => "\\t".into(),
        c if c.is_control()
            || matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}') =>
        {
            format!("\\u{{{:x}}}", c as u32)
        }
        c => c.to_string(),
    }
}
fn escaped(raw: &[u8], marks: &[u8], base: u64, encoding: Encoding) -> (String, Vec<LineSpan>) {
    let mut text = String::new();
    let mut spans: Vec<LineSpan> = Vec::new();
    let mut at = 0;
    while at < raw.len() {
        let (mut width, mut token) = if encoding == Encoding::Utf8 {
            let width = utf8_width(raw[at]);
            match raw
                .get(at..at + width)
                .and_then(|b| std::str::from_utf8(b).ok())
                .and_then(|s| s.chars().next())
            {
                Some(c) => (width, safe_char(c)),
                None => (1, format!("\\x{:02x}", raw[at])),
            }
        } else if at + 1 < raw.len() {
            let unit = encoding.unit([raw[at], raw[at + 1]]);
            let second = raw.get(at + 2..at + 4).map(|b| encoding.unit([b[0], b[1]]));
            if (0xd800..=0xdbff).contains(&unit)
                && second.is_some_and(|u| (0xdc00..=0xdfff).contains(&u))
            {
                let scalar =
                    0x10000 + (((unit as u32 - 0xd800) << 10) | (second.unwrap() as u32 - 0xdc00));
                (4, safe_char(char::from_u32(scalar).unwrap()))
            } else {
                (
                    2,
                    char::from_u32(unit as u32)
                        .map(safe_char)
                        .unwrap_or_else(|| format!("\\u{{{unit:04x}}}")),
                )
            }
        } else {
            (1, format!("\\x{:02x}", raw[at]))
        };
        if marks[at..at + width].iter().any(|&m| m != marks[at]) {
            // Byte regexes can match only part of a UTF-8 scalar. Escape each
            // byte in that scalar so red always denotes precisely matched bytes.
            width = 1;
            token = format!("\\x{:02x}", raw[at]);
        }
        let start = text.len();
        text.push_str(&token);
        if marks[at] != 0 {
            if let Some(last) = spans.last_mut()
                && last.end == start
                && last.offset + last.length == base + at as u64
            {
                last.end = text.len();
                last.length += width as u64;
            } else {
                spans.push(LineSpan {
                    start,
                    end: text.len(),
                    offset: base + at as u64,
                    length: width as u64,
                });
            }
        }
        at += width;
    }
    (text, spans)
}
