//! Error types and conversions for this subsystem.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use thiserror::Error;

/// Errors from cache operations (SQLite, filesystem, or configuration).
#[derive(Debug, Error)]
pub enum CacheError {
    /// Could not read file (e.g. permissions, missing file).
    #[error("Failed to read file: {path}")]
    Metadata {
        /// Path to the file.
        path: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// Platform cache directory could not be determined.
    #[error("Failed to determine cache directory")]
    CacheDirMissing,
    /// Failed to create or migrate the SQLite database schema.
    #[error("Failed to initialize cache database")]
    InitDatabase {
        /// Underlying SQLite error.
        source: sqlx::Error,
    },
    /// A database migration failed.
    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    /// A read-only open found no cache database to read.
    #[error("no validation cache exists at {path}")]
    NoCacheDatabase {
        /// The database path looked for.
        path: String,
    },
    /// A read-only open found a database whose schema is older than this
    /// build's; a writing run upgrades it.
    #[error("the validation cache at {path} has an older schema; a writing run upgrades it")]
    SchemaNotCurrent {
        /// The database path.
        path: String,
    },
    /// A database a newer build migrated past every migration this build
    /// knows: this build can neither read it nor migrate it.
    #[error(
        "the validation cache at {path} was written by a newer build, which this build cannot read"
    )]
    SchemaNewer {
        /// The database path.
        path: String,
    },
    /// Timed out waiting for the cross-process cache initialization lock.
    ///
    /// Another process held the advisory init lock (taken around first-time
    /// database create + migrate) past the bounded acquisition deadline.
    /// Initialization deliberately fails typed here rather than blocking
    /// indefinitely; callers of [`crate::CachePool::new`] can explicitly
    /// handle this error by continuing uncached.
    #[error("Timed out waiting for cache initialization lock: {path}")]
    InitLockTimeout {
        /// Path to the lockfile beside the cache database.
        path: String,
    },
    /// A SQLite query or update failed.
    #[error("Database operation failed")]
    Database {
        /// Underlying SQLite error.
        source: sqlx::Error,
    },
    /// General filesystem I/O error.
    #[error("IO error: {path}")]
    Io {
        /// Path involved in the I/O operation.
        path: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// A file's modification time lies outside the range a timestamp can
    /// hold (years -9999 to 9999), reachable only by a forged time.
    #[error("modification time of {} is out of range", path.display())]
    ModifiedOutOfRange {
        /// The file whose time could not be admitted.
        path: std::path::PathBuf,
        /// Why jiff refused it.
        source: jiff::Error,
    },
    /// A stored column held a value its type does not allow (a pass/fail
    /// that is neither 0 nor 1, a tested roundtrip with no verdict).
    #[error("corrupt cache row: column {column} holds {value:?}")]
    CorruptColumn {
        /// The column.
        column: &'static str,
        /// What it held.
        value: Option<i64>,
    },
    /// SQLite reported a count that is negative or does not fit in `usize`.
    #[error("cache count {value} is not a count on this platform")]
    CountOutOfRange {
        /// The count as SQLite reported it.
        value: i128,
    },
    /// Freeform error message for miscellaneous failures.
    #[error("{0}")]
    Message(String),
}
