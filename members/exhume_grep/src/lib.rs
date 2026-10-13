//! Search indexed regular files without extracting or modifying evidence.
//!
//! `search` streams serializable events through a bounded Tokio channel. Run a
//! consumer concurrently; dropping it stops the scan. Each worker independently
//! opens its source, never sharing seek cursors with another worker or viewer.
mod lines;
pub mod matcher;
mod search;
pub use lines::{LineSpan, MatchingLine};
pub use matcher::{Encoding, Hit, Matcher, MatcherOptions, Mode, Pattern, Preview, SkippedRange};
pub use search::search;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub database: PathBuf,
    pub evidence_id: i64,
    /// Override a relocated evidence source; identity/metadata checks still apply.
    pub source: Option<PathBuf>,
    pub partition_ids: Vec<i64>,
    pub directory: Option<String>,
    /// Regular expressions matched against normalized indexed paths.
    pub include_paths: Vec<String>,
    pub exclude_paths: Vec<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub matcher: MatcherOptions,
    pub workers: usize,
    pub files_only: bool,
    pub max_hits: Option<u64>,
    pub max_hits_per_file: Option<u64>,
}
impl SearchRequest {
    pub fn new(database: impl Into<PathBuf>, evidence_id: i64, matcher: MatcherOptions) -> Self {
        Self {
            database: database.into(),
            evidence_id,
            source: None,
            partition_ids: Vec::new(),
            directory: None,
            include_paths: Vec::new(),
            exclude_paths: Vec::new(),
            min_size: None,
            max_size: None,
            matcher,
            workers: 1,
            files_only: false,
            max_hits: None,
            max_hits_per_file: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexedFile {
    pub id: i64,
    pub evidence_id: i64,
    pub partition_id: i64,
    #[serde(with = "identifier_string")]
    pub identifier: u64,
    /// Indexed generation where the inventory supplies it (currently NTFS).
    pub generation: Option<u64>,
    pub path: String,
    pub size: u64,
    pub modified: Option<u64>,
    pub created: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileGroup {
    pub file: IndexedFile,
    pub aliases: Vec<IndexedFile>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Complete,
    Incomplete,
    Limited,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchSummary {
    pub status: Status,
    pub selected_paths: u64,
    pub scanned_files: u64,
    pub matched_files: u64,
    pub failed_files: u64,
    pub hits: u64,
    pub bytes_read: u64,
    pub skipped_ranges: u64,
    pub limited_files: u64,
    pub elapsed_ms: u64,
    pub first_hit_ms: Option<u64>,
}
impl Default for SearchSummary {
    fn default() -> Self {
        Self {
            status: Status::Complete,
            selected_paths: 0,
            scanned_files: 0,
            matched_files: 0,
            failed_files: 0,
            hits: 0,
            bytes_read: 0,
            skipped_ranges: 0,
            limited_files: 0,
            elapsed_ms: 0,
            first_hit_ms: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SearchEvent {
    Started {
        schema_version: u32,
        evidence_id: i64,
        source: PathBuf,
        matcher: MatcherOptions,
        workers: usize,
    },
    FileStarted {
        files: Arc<FileGroup>,
        source_view: String,
    },
    Match {
        files: Arc<FileGroup>,
        hit: Hit,
    },
    /// Optional grouped display context; occurrence counts come from `Match`.
    MatchingLine {
        files: Arc<FileGroup>,
        line: MatchingLine,
    },
    FileMatched {
        files: Arc<FileGroup>,
    },
    SkippedRange {
        files: Arc<FileGroup>,
        range: SkippedRange,
    },
    FileError {
        file: IndexedFile,
        message: String,
    },
    SourceError {
        evidence_id: i64,
        message: String,
    },
    FileFinished {
        files: Arc<FileGroup>,
        status: Status,
        bytes_read: u64,
        hits: u64,
    },
    Progress {
        bytes_read: u64,
        scanned_files: u64,
        hits: u64,
    },
    Finished {
        summary: SearchSummary,
    },
}

// APFS packed identifiers exceed JavaScript's exact-integer range even on
// ordinary evidence. Preserve them when events cross a JSON/Tauri boundary.
mod identifier_string {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
