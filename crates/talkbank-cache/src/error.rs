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
    /// Timed out waiting for the cross-process cache initialization lock.
    ///
    /// Another process held the advisory init lock (taken around first-time
    /// database create + migrate) past the bounded acquisition deadline.
    /// Initialization deliberately fails typed here rather than blocking
    /// indefinitely; callers such as [`crate::CachePool::open_or_else`]
    /// degrade to running uncached.
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
    /// Freeform error message for miscellaneous failures.
    #[error("{0}")]
    Message(String),
}
