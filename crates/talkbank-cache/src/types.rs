//! Type definitions for unified cache
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::{Path, PathBuf};

use crate::CacheError;

/// Cache statistics: the entry count and what the storage says about itself.
///
/// Built only by `CachePool::stats`, which reads the database file's facts
/// itself, so callers render this rather than re-deriving the file from the
/// directory. `#[non_exhaustive]`, like the data-carrying variants below, so
/// no other crate can assemble one from raw parts: a report it reads is a
/// report the cache made.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheStats {
    /// Total number of file entries in the cache database.
    pub total_entries: usize,
    /// Where the database lives, and its file's facts when it has one.
    pub storage: StorageStats,
}

/// Where a cache's database lives, as reported by `CachePool::stats`.
///
/// "In memory" is a variant, not a missing directory: an in-memory cache has
/// neither a directory nor a file, and a directory cache always has a
/// directory and a [`DatabaseFile`] answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageStats {
    /// An in-memory database: no directory and no file.
    InMemory,
    /// A database in the directory the cache opened.
    #[non_exhaustive]
    Directory {
        /// The directory the cache opened.
        cache_dir: PathBuf,
        /// The database file in that directory.
        database: DatabaseFile,
    },
}

/// What the database file of a directory cache says about itself.
///
/// "No file" and "an empty file" are different facts and so different
/// variants; a size of zero is only ever the length of a file that exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseFile {
    /// No file. Opening a directory cache creates its file, so this means it
    /// was removed after the cache opened.
    Missing,
    /// The file exists and its metadata was read.
    #[non_exhaustive]
    Present {
        /// The file's length in bytes.
        size_bytes: u64,
        /// When the file last changed. Not an `Option`: a platform that keeps
        /// no modification time cannot occur on the platforms this crate is
        /// released for (macOS, Linux, Windows), so that case is a
        /// [`CacheError::Io`] rather than a variant or a guessed time. A
        /// `jiff::Timestamp`, admitted when the file is read: a time outside
        /// its range is [`CacheError::ModifiedOutOfRange`] there, so a
        /// renderer converts it without a failure case of its own.
        modified: jiff::Timestamp,
    },
}

impl DatabaseFile {
    /// Read the facts of the database file at `path`. A missing file is
    /// [`DatabaseFile::Missing`]; any other failure, including a platform
    /// with no modification time, is a [`CacheError::Io`], and a time out of
    /// range is a [`CacheError::ModifiedOutOfRange`], never a made-up size or
    /// time.
    pub(crate) fn read(path: &Path) -> Result<Self, CacheError> {
        let io_error = |source| CacheError::Io {
            path: path.display().to_string(),
            source,
        };
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Self::Missing),
            Err(err) => return Err(io_error(err)),
        };
        Ok(Self::Present {
            size_bytes: metadata.len(),
            modified: jiff::Timestamp::try_from(metadata.modified().map_err(io_error)?).map_err(
                |source| CacheError::ModifiedOutOfRange {
                    path: path.to_path_buf(),
                    source,
                },
            )?,
        })
    }
}

/// Where an open pool's database lives, fixed when it opened. The private
/// source of [`StorageStats`]; reading the file's facts is the one transition
/// between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CacheStorage {
    /// `sqlite::memory:`.
    InMemory,
    /// The directory the pool opened; its database file is named by
    /// [`crate::cache_db_path`].
    Directory(PathBuf),
}

impl CacheStorage {
    /// Read what the storage says about itself now.
    pub(crate) fn stats(&self) -> Result<StorageStats, CacheError> {
        match self {
            Self::InMemory => Ok(StorageStats::InMemory),
            Self::Directory(cache_dir) => Ok(StorageStats::Directory {
                database: DatabaseFile::read(&crate::cache_db_path(cache_dir))?,
                cache_dir: cache_dir.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The read boundary: a missing file is `Missing`, an existing empty
    /// file is `Present` with size zero.
    #[test]
    fn read_tells_a_missing_file_from_an_empty_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("cache.db");
        assert_eq!(
            DatabaseFile::read(&path).expect("missing file is not an error"),
            DatabaseFile::Missing
        );

        std::fs::write(&path, b"").expect("create empty file");
        assert!(
            matches!(
                DatabaseFile::read(&path).expect("read empty file"),
                DatabaseFile::Present { size_bytes: 0, .. }
            ),
            "an empty file is present with size zero"
        );
    }

    /// A directory cache whose database file is gone reports its directory
    /// with the file `Missing`, never an empty file. Read off the storage
    /// alone, with no pool open: a pool does not promise to reuse its open
    /// connection, and a new one recreates the file empty.
    #[test]
    fn a_directory_whose_database_is_gone_reports_it_missing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let storage = CacheStorage::Directory(dir.path().to_path_buf());
        assert_eq!(
            storage.stats().expect("storage facts"),
            StorageStats::Directory {
                cache_dir: dir.path().to_path_buf(),
                database: DatabaseFile::Missing,
            }
        );
    }
}

/// Validation identity: build/rule generation and parser row namespace.
/// Parser identity is deliberately outside the retained build generations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheIdentity {
    rules_version: crate::RulesVersion,
    parser: talkbank_model::ParserKind,
}

impl CacheIdentity {
    /// Bind a rule generation to the parser that will produce its verdicts.
    pub fn new(rules_version: crate::RulesVersion, parser: talkbank_model::ParserKind) -> Self {
        Self {
            rules_version,
            parser,
        }
    }

    /// Build/rule generation used for compatibility and retention.
    pub fn rules_version(&self) -> &crate::RulesVersion {
        &self.rules_version
    }

    /// Parser namespace carried by every validation and roundtrip lookup.
    pub fn parser(&self) -> talkbank_model::ParserKind {
        self.parser
    }

    /// The key namespace of this identity's validation rows.
    pub(crate) fn validation_namespace(&self) -> crate::cache_utils::KeyNamespace {
        crate::cache_utils::KeyNamespace::Validation(self.parser)
    }

    /// The key namespace of this identity's roundtrip rows.
    pub(crate) fn roundtrip_namespace(&self) -> crate::cache_utils::KeyNamespace {
        crate::cache_utils::KeyNamespace::Roundtrip(self.parser)
    }
}
