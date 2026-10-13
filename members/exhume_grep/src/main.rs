use anyhow::{Context, Result, ensure};
use clap::{Parser, ValueEnum};
use exhume_grep::*;
use std::{
    io::{self, IsTerminal, Read, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::mpsc;

#[derive(Clone, Copy, ValueEnum)]
enum Output {
    Text,
    Ndjson,
}
#[derive(Clone, Copy, ValueEnum)]
enum Color {
    Auto,
    Always,
    Never,
}
#[derive(Clone, Copy, ValueEnum)]
enum Charset {
    Utf8,
    Utf16le,
    Utf16be,
    All,
}
#[derive(Parser)]
#[command(
    version,
    about = "Search logical file content selected from an Exhume index (read-only)."
)]
struct Args {
    #[arg(short, long)]
    database: PathBuf,
    #[arg(long)]
    evidence_id: i64,
    /// Relocated evidence path (otherwise use the index's evidence.path).
    #[arg(short, long)]
    body: Option<PathBuf>,
    #[arg(long)]
    partition_id: Vec<i64>,
    #[arg(short = 'e', long = "pattern")]
    patterns: Vec<String>,
    /// UTF-8 file, one pattern per line; empty lines are rejected.
    #[arg(long)]
    pattern_file: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "literal")]
    mode: Mode,
    #[arg(long, value_enum, default_value = "utf8")]
    encoding: Charset,
    #[arg(long)]
    directory: Option<String>,
    /// Include normalized indexed paths matching this regex (repeatable).
    #[arg(long = "include")]
    include: Vec<String>,
    /// Exclude normalized indexed paths matching this regex (repeatable).
    #[arg(long = "exclude")]
    exclude: Vec<String>,
    #[arg(long)]
    min_size: Option<u64>,
    #[arg(long)]
    max_size: Option<u64>,
    #[arg(short = 'j', long, default_value_t = 1)]
    workers: usize,
    #[arg(short = 'l', long)]
    files_with_matches: bool,
    #[arg(long)]
    max_hits: Option<u64>,
    #[arg(long)]
    max_hits_per_file: Option<u64>,
    #[arg(long, default_value_t = 8 * 1024 * 1024)]
    max_line_bytes: usize,
    #[arg(long, default_value_t = 0)]
    preview_bytes: usize,
    /// Print grouped matching lines (or add matching_line events to NDJSON).
    #[arg(long, conflicts_with = "files_with_matches")]
    show_line: bool,
    /// Source bytes per display excerpt, 16..=65536 (default: 4096).
    #[arg(long, requires = "show_line")]
    line_bytes: Option<usize>,
    /// Color matching spans in text output; NDJSON never contains ANSI.
    #[arg(long, value_enum, default_value = "auto")]
    color: Color,
    #[arg(long, value_enum, default_value = "text")]
    output: Output,
}
#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("exhume_grep: {error:#}");
            std::process::ExitCode::from(2)
        }
    }
}
async fn run() -> Result<u8> {
    let mut args = Args::parse();
    if let Some(path) = &args.pattern_file {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .context("open pattern file")?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 1024 * 1024, "pattern file exceeds 1 MiB");
        args.patterns
            .extend(std::str::from_utf8(&bytes)?.lines().map(str::to_owned));
    }
    let encodings = match args.encoding {
        Charset::Utf8 => vec![Encoding::Utf8],
        Charset::Utf16le => vec![Encoding::Utf16le],
        Charset::Utf16be => vec![Encoding::Utf16be],
        Charset::All => vec![Encoding::Utf8, Encoding::Utf16le, Encoding::Utf16be],
    };
    let matcher = MatcherOptions {
        mode: args.mode,
        encodings,
        max_line_bytes: args.max_line_bytes,
        preview_bytes: args.preview_bytes,
        show_line: args.show_line,
        line_bytes: args.line_bytes.unwrap_or(4096),
        patterns: args
            .patterns
            .into_iter()
            .enumerate()
            .map(|(id, expression)| Pattern {
                id: (id + 1).to_string(),
                expression,
            })
            .collect(),
        ..Default::default()
    };
    let mut request = SearchRequest::new(args.database, args.evidence_id, matcher);
    request.source = args.body;
    request.partition_ids = args.partition_id;
    request.directory = args.directory;
    request.include_paths = args.include;
    request.exclude_paths = args.exclude;
    request.min_size = args.min_size;
    request.max_size = args.max_size;
    request.workers = args.workers;
    request.files_only = args.files_with_matches;
    request.max_hits = args.max_hits;
    request.max_hits_per_file = args.max_hits_per_file;
    let cancel = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, cancel.clone())?;
    #[cfg(unix)]
    signal_hook::flag::register(signal_hook::consts::SIGTERM, cancel.clone())?;
    let (tx, mut rx) = mpsc::channel(128);
    let token = cancel.clone();
    let task = tokio::spawn(async move { search(request, tx, token).await });
    let color = match args.color {
        Color::Always => true,
        Color::Never => false,
        Color::Auto => {
            io::stdout().is_terminal()
                && std::env::var_os("NO_COLOR").is_none()
                && std::env::var_os("TERM").is_some_and(|term| term != "dumb")
        }
    };
    let mut out = io::BufWriter::new(io::stdout());
    let mut write_error = None;
    while let Some(event) = rx.recv().await {
        let result = match args.output {
            Output::Ndjson => serde_json::to_writer(&mut out, &event)
                .map_err(io::Error::other)
                .and_then(|_| writeln!(out)),
            Output::Text => render(&mut out, &event, args.show_line, color),
        }
        .and_then(|_| out.flush());
        if let Err(error) = result {
            write_error = Some(error);
            cancel.store(true, Ordering::Relaxed);
            rx.close();
            break;
        }
    }
    let summary = task.await??;
    if let Some(error) = write_error
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    Ok(match summary.status {
        Status::Cancelled => 130,
        Status::Incomplete | Status::Limited => 2,
        Status::Complete if summary.hits == 0 => 1,
        Status::Complete => 0,
    })
}
fn render(
    out: &mut impl Write,
    event: &SearchEvent,
    show_line: bool,
    color: bool,
) -> io::Result<()> {
    let quoted = |path: &str| serde_json::to_string(path).unwrap();
    match event {
        SearchEvent::Match { files, hit } => {
            if show_line {
                return Ok(());
            }
            for file in std::iter::once(&files.file).chain(&files.aliases) {
                writeln!(
                    out,
                    "{}:{}:{} pattern={} encoding={:?} file_id={}{}",
                    quoted(&file.path),
                    hit.offset,
                    hit.length,
                    quoted(&hit.pattern_id),
                    hit.encoding,
                    file.id,
                    hit.preview
                        .as_ref()
                        .map(|p| format!(" preview@{}={}", p.offset, p.bytes_hex))
                        .unwrap_or_default()
                )?;
            }
            Ok(())
        }
        SearchEvent::MatchingLine { files, line } => {
            for file in std::iter::once(&files.file).chain(&files.aliases) {
                write!(
                    out,
                    "{}:{}:{} encoding={:?} alignment={} file_id={}: {}",
                    quoted(&file.path),
                    line.line_number,
                    line.offset,
                    line.encoding,
                    line.alignment,
                    file.id,
                    if line.truncated_start { "[…]" } else { "" }
                )?;
                let mut from = 0;
                for span in &line.spans {
                    write!(out, "{}", &line.text[from..span.start])?;
                    if color {
                        write!(out, "\x1b[31m")?;
                    }
                    write!(out, "{}", &line.text[span.start..span.end])?;
                    if color {
                        write!(out, "\x1b[0m")?;
                    }
                    from = span.end;
                }
                writeln!(
                    out,
                    "{}{}{}",
                    &line.text[from..],
                    if line.truncated_end { "[…]" } else { "" },
                    if line.partial { " [partial]" } else { "" }
                )?;
            }
            Ok(())
        }
        SearchEvent::FileMatched { files } => {
            writeln!(
                out,
                "{} file_id={}",
                quoted(&files.file.path),
                files.file.id
            )?;
            for alias in &files.aliases {
                writeln!(
                    out,
                    "{} file_id={} alias_of={}",
                    quoted(&alias.path),
                    alias.id,
                    files.file.id
                )?;
            }
            Ok(())
        }
        SearchEvent::FileError { file, message } => {
            eprintln!("file {}: {}", file.id, quoted(message));
            Ok(())
        }
        SearchEvent::SourceError { message, .. } => {
            eprintln!("source: {}", quoted(message));
            Ok(())
        }
        SearchEvent::SkippedRange { files, range } => {
            eprintln!(
                "file {}: skipped bytes {}..{} ({:?}, alignment {}): {}",
                files.file.id,
                range.offset,
                range.offset + range.length,
                range.encoding,
                range.alignment,
                range.reason
            );
            Ok(())
        }
        SearchEvent::Finished { summary } => {
            eprintln!(
                "{:?}: {} hits, {} files scanned, {} failures, {} skipped ranges, {} bytes, {} ms",
                summary.status,
                summary.hits,
                summary.scanned_files,
                summary.failed_files,
                summary.skipped_ranges,
                summary.bytes_read,
                summary.elapsed_ms
            );
            Ok(())
        }
        _ => Ok(()),
    }
}
