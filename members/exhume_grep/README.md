# exhume_grep

A Rust library and CLI for scanning the logical content of regular files already
inventoried by `exhume_indexer`. Queries start without building a separate content
index. Evidence and the existing SQLite index are opened read-only; no files are
extracted. Results stream as they become available.

## Build and run

Requires Rust 1.92 or newer. After publication, install the CLI with
`cargo install exhume_grep --locked` or add `exhume_grep = "0.1"` to a Rust
project's dependencies.

From the Exhume workspace:

```sh
cargo build --release -p exhume_grep
./target/release/exhume_grep --database case.sqlite --evidence-id 1 -e 'password'
./target/release/exhume_grep --database case.sqlite --evidence-id 1 \
  -e 'password' --show-line
./target/release/exhume_grep --database case.sqlite --evidence-id 1 \
  --mode regex -e '(?i)https?://[^\s]+' --encoding all --output ndjson
./target/release/exhume_grep --database case.sqlite --evidence-id 1 \
  --pattern-file indicators.txt --partition-id 2 --directory /Users \
  --exclude '/Cache/' --max-size 1073741824 --files-with-matches
```

`--body` overrides a relocated evidence source. Without it, `evidence.path` is
used. An explicit evidence ID is mandatory; partitions must belong to it.
`--include`/`--exclude` are regular expressions over normalized indexed paths,
not shell globs. Directory and size filters are optional; no filename, extension,
or binary-content heuristics exclude regular files. Pattern-file lines are UTF-8,
with one nonempty pattern per line. CLI pattern IDs are one-based input positions.

## Matching contract

| Property | Behavior |
|---|---|
| Default | Case-sensitive literal, UTF-8, one worker, unlimited streamed hits |
| Multiple patterns | One physical content pass per file; each hit retains its pattern ID |
| Literals | Exact encoded bytes; overlaps included, including across read buffers and newlines |
| Regex | Rust regex syntax, non-overlapping occurrences independently for each pattern |
| Encodings | Explicit `utf8`, `utf16le`, `utf16be`, or `all`; no encoding guessing |
| UTF-16 | Literals search either alignment; regex scans both even and odd byte alignments |
| Offsets | Zero-based byte offsets and byte lengths in the original logical file, never decoded-text offsets |
| Regex lines | LF-delimited; CRLF terminators excluded; a final lone CR remains content |
| Case folding | Regex `(?i)` uses Unicode simple folding; no normalization/accent folding; literals remain exact |
| Anchors | `^`/`$` refer to lines; `\A`, `\z`, and disabling multiline anchors are rejected |
| Unsupported regex | Backreferences, lookaround, and expressions capable of empty matches |
| Binary data | UTF-8 byte regex supported; UTF-16 regex must match complete Unicode scalars |
| Malformed UTF-16 | Invalid units become nonmatching barriers, not replacement characters or removed bytes |
| BOM | Preserved as original content; an explicitly selected endianness is never overridden |

`--encoding all` performs all interpretations without rereading the file for each
encoding. A hit is identified by pattern, encoding, offset, and length; different
interpretations are not silently merged. Regexes cannot span LF, including with
`(?s)`. Escaped newline expressions have no cross-line matching behavior.

The default regex line ceiling is **8 MiB of original bytes per interpretation**.
`--max-line-bytes` changes it (up to 256 MiB). An oversized line is discarded as a
whole, an exact skipped range is emitted, and coverage is incomplete. Literal
matching has no line ceiling. Binary files without LF can exceed the regex limit.

Optional `--preview-bytes` includes up to 4096 original bytes encoded as hex, with
its own source offset. Previews can be shorter at a read-buffer or line boundary;
they are display context, not an extracted file or evidence hash.

## Matching lines and highlighting

`--show-line` prints each ordinary matching line once per selected path and
encoding/alignment, with all matching spans highlighted in red. Overlapping
occurrences share a highlight; occurrence counts and pattern IDs still come from
the individual match events. The prefix includes the quoted path, one-based line
number, zero-based **original byte offset of the excerpt**, encoding, alignment,
and indexed file ID. Different UTF-16 alignments have independent line numbers.

```sh
exhume_grep -d case.sqlite --evidence-id 1 -e password --show-line
exhume_grep -d case.sqlite --evidence-id 1 -e password --show-line --color always
exhume_grep -d case.sqlite --evidence-id 1 -e password --show-line \
  --line-bytes 8192 --output ndjson
```

`--color auto|always|never` defaults to `auto`: color is enabled for terminal
stdout when `TERM` is set to something other than `dumb` and `NO_COLOR` is unset.
Pipes and redirected files are uncolored unless `always` is specified. NDJSON
never contains ANSI, even with `--color always`.

Lines use LF boundaries in the selected encoding; ordinary CRLF delimiters are
omitted. Literal matches spanning lines show every touched line. Matched newline
characters appear as highlighted `\n` markers, including newline-only matches.
Tabs, controls, backslashes, malformed bytes/UTF-16 units, and directional format
controls are visibly escaped. Byte regexes matching part of a UTF-8 scalar show
byte escapes, so highlights still identify exact source bytes. This is safe
display text, not an extracted or normalized copy of the evidence.

`--line-bytes` bounds each excerpt to **4096 original bytes** by default
(configurable from 16 to 65536). Longer lines produce bounded excerpts containing
matches, with `[…]` markers where context is omitted; excerpts share the original
line number. Matches crossing excerpt boundaries continue into the next excerpt.
Valid Unicode scalars stay together. Escaping can expand text to at most eight
times the source-byte limit. Display truncation does **not** skip search coverage
or change a complete result to incomplete. The existing regex `--max-line-bytes`
search ceiling remains separate.

Context is collected in the same evidence pass. Literal display waits until
potential overlapping matches are resolved, then until a newline, excerpt
boundary, or EOF. Enabling it adds buffering, line tracking, decoding, and output
work; the default scan does not collect this context. At a hit cap, read failure,
or cancellation, available context is marked `[partial]` and no additional read
is made to finish a line. Cancellation flushes are best effort and do not wait
for a stalled consumer. Already-read bytes can be included without all their
matches being enumerated; partial highlights therefore are not exhaustive.
`--show-line` conflicts with `--files-with-matches`.

For library consumers, set `MatcherOptions.show_line = true` and optionally
`line_bytes`. `SearchEvent::MatchingLine` / NDJSON `matching_line` accompanies
the existing occurrence events, using **schema version 2** when enabled (version
1 otherwise). `MatchingLine` contains escaped `text`, encoding/alignment, line
number/start, excerpt source offset/length, truncation/partial flags, and merged
`spans`. A span's `start`/`end` index **UTF-8 bytes in the escaped text**, not
JavaScript UTF-16 string indices; `offset`/`length` always index original file
bytes. Consumers can render those spans themselves without terminal escapes.
Line events do not increment hit counts and share the occurrence events' file
identity, aliases, and provisional-until-file-finished semantics.

## Coverage and identity

Scope is the primary logical content of indexed regular files. Deleted files,
unallocated space, NTFS alternate streams, document text extraction, and archive
traversal are outside v1. Sparse logical regions are searched as zeroes.

Each result contains the database row ID, filesystem identifier, evidence and
partition IDs, indexed path, and size. These IDs are distinct. Filesystem
identifiers serialize as decimal strings so JavaScript preserves packed APFS
identifiers exactly; the Rust API uses `u64`. Selected hard-link
occurrences are validated, grouped by the reader's actual file identity, and read
once; the other selected paths appear in `aliases`. Hit counts describe content
occurrences; readable CLI output renders those hits for each selected alias path. Only paths present in the
inventory can be reported. In particular, current Folder indexing can deduplicate
hard links before grep sees them; NTFS inventories can preserve those occurrences.

Identity and available size/creation/modification timestamps are checked before
and after a scan. Folder handles also compare precise modification times across
reading. Changes, missing files, lookup failures, and short reads are reported.
Hits emitted before a file finishes are provisional until its `file_finished`
event. A changed image source produces `source_error`; all its hits need
revalidation. Checks do not prove unchanged content since indexing: the index
contains no content fingerprint, timestamps can be coarse, and multi-part image
backing files are not individually fingerprinted. Search immutable evidence.

The filesystem dependencies provide these reader guarantees:

- exFAT prefix/range reads avoid whole-file allocation and reuse sequential FAT
  traversal. Invalid/truncated chains and cycles are errors. Bytes beyond valid
  data length are zero-filled, as required by the
  [exFAT specification](https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification#765-validdatalength-field).
- APFS resource-fork compression reads its table and required compressed chunks
  through extent ranges. It decodes one 64 KiB chunk at a time and caches the last
  chunk. Tables are capped at 4 MiB; individual compressed chunks at 1 MiB.
- Inline APFS compression is decoded once and cached, with an 8 MiB input/output
  ceiling. Larger inline files are reported as unsupported by this bounded reader.
- APFS extent reads propagate failures and reject unsupported encrypted extents.
- Folder scanning retains one verified file handle per active file.

Existing NTFS-compressed/EFS streams and other reader limitations remain explicit
file errors. APFS resource-fork decompression reconstructs primary file content;
it does not introduce general alternate-stream searching.

## Streaming API

```rust,no_run
use exhume_grep::{search, MatcherOptions, Pattern, SearchRequest};
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::mpsc;

# async fn example() -> anyhow::Result<()> {
let options = MatcherOptions {
    patterns: vec![Pattern { id: "indicator-1".into(), expression: "password".into() }],
    ..Default::default()
};
let request = SearchRequest::new("case.sqlite", 1, options);
let cancel = Arc::new(AtomicBool::new(false));
let (tx, mut rx) = mpsc::channel(128);
let job = tokio::spawn(search(request, tx, cancel.clone()));
while let Some(event) = rx.recv().await {
    // Forward structured events to the consumer/UI; do not accumulate all hits.
    println!("{}", serde_json::to_string(&event)?);
}
let summary = job.await??;
# Ok(())
# }
```

Set `cancel` to true to cancel. A dropped consumer stops the scan; dropping the
search future requests cancellation of its workers. Workers open independent
sources rather than cloning shared seek handles. `--workers` is configurable
from 1 to 64; more workers can increase seeks and duplicate filesystem metadata
caches. The default remains one.

Events use a tagged `event` field: `started` (schema version 1, or 2 with line
context enabled), `file_started`,
`match` or `file_matched`, optional `matching_line`, `skipped_range`, `file_error`, `source_error`,
`file_finished`, `progress`, and `finished`. Ordering is discovery order, not a
global sort by path or offset. Multiple workers may interleave files. Progress is
throttled and expendable; hits/errors use bounded-channel backpressure. Consumers
must drain concurrently. The returned summary is authoritative: cancellation
waits at most 100 ms to queue the terminal event if a consumer has stopped draining.

A SQLite read transaction fixes candidate metadata for the request. In rollback
journal mode, this can delay database writers until the search ends; grep does
not change the case database's journal policy. A streaming
query sorts at most once, with temporary sorts on disk; it does not load the
entire inventory into a result vector. Search buffers, payload caches, queues,
and pattern input are bounded. Total process memory also depends on existing
filesystem metadata caches, active hard-link groups, FAT cycle-detection metadata,
compiled regexes and worker count. UTF-16 regex adds up to four line buffers plus
one transient decoded line. This is not a fixed-RAM guarantee for every backend.
Optional line display adds at most five excerpt buffers, a transient 64 KiB byte
mask, and merged hit intervals bounded by the active literal window or regex
line size, rather than retaining every occurrence until the file finishes.

Cancellation is checked between reads, lines, patterns, matches, and output
retries. It cannot interrupt a blocking filesystem call, SQLite operation, regex
compilation, or an individual synchronous regex evaluation. Adversarial regex
match enumeration can be expensive despite using a non-backtracking engine.

## Completion and limits

File errors allow other files to continue. A final summary distinguishes:

- `complete`: the requested scope/mode was searched, including a complete no-hit result.
- `incomplete`: file/source failures or skipped regex ranges left coverage gaps.
- `limited`: an explicit overall or per-file hit cap stopped searching.
- `cancelled`: cancellation or a closed consumer stopped the job.

`--max-hits` and `--max-hits-per-file` are opt-in. Reaching a cap conservatively
marks the search limited, even if that hit happened to be the last possible one.
`--files-with-matches` stops at the first hit in each content group; this is a
complete answer for that mode, not a count of every occurrence. A complete
zero-hit result requires a `finished` event or successful returned summary with
`status = complete`. EOF alone is not a completion marker. Invalid requests and
fatal database errors return `Err`; the CLI reports them on stderr.

CLI exit codes: **0** complete with hits, **1** complete without hits, **2** invalid,
failed, incomplete, or limited, **130** cancelled. NDJSON goes to stdout; readable
errors and summaries use stderr. No search history or hits are stored in the case
database. The API is ready for later Thanatology integration; this change adds no UI.

## Development and validation

```sh
cargo fmt -p exhume_grep --check
cargo test -p exhume_grep
cargo clippy -p exhume_grep --all-targets --no-deps -- -D warnings
cargo package --list -p exhume_grep
cargo publish --dry-run -p exhume_grep
```

Workspace patches select local Exhume dependencies for development. Package
verification must also succeed against the published registry dependencies;
a successful workspace build alone is not a release check. Keep the package's
integration tests: they exercise real indexing, read-only evidence access,
streaming, CLI output, and completion status. They use temporary fixtures that
are removed automatically.

The first release uses published `exhume_filesystem 0.5.6` (including
`exhume_exfat 0.1.13`) for the bounded readers described above. Integration tests
use published `exhume_indexer 0.2.4` and its `exhume_artefacts 0.3.4` dependency,
with the same filesystem version throughout.
