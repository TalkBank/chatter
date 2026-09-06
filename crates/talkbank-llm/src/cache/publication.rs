//! Exclusive ownership and ordered publication of response-cache snapshots.
//!
//! The process lock lives beside the destination, not on the inode replaced by
//! rename. Keep that lockfile in place: deleting it would let another opener
//! lock a different inode and defeat exclusivity.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::Path;
use std::sync::MutexGuard;

use super::{CacheError, CachePath, ResponseCache};

/// Canonical cache location with exclusive process ownership until drop.
#[derive(Debug)]
pub(super) struct ExclusiveCache {
    path: CachePath,
    _lease: File,
}

impl ExclusiveCache {
    pub(super) fn acquire(requested: CachePath) -> Result<Self, CacheError> {
        let path = match requested.0.canonicalize() {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // A dangling symlink is not a new cache; preserve it and fail.
                if std::fs::symlink_metadata(&requested.0).is_ok() {
                    return Err(CacheError::Io {
                        path: requested.0,
                        source: error,
                    });
                }
                let name = requested.0.file_name().ok_or_else(|| CacheError::Io {
                    path: requested.0.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "cache path needs a filename",
                    ),
                })?;
                let parent =
                    parent_of(&requested.0)
                        .canonicalize()
                        .map_err(|source| CacheError::Io {
                            path: requested.0.clone(),
                            source,
                        })?;
                parent.join(name)
            }
            Err(source) => {
                return Err(CacheError::Io {
                    path: requested.0,
                    source,
                });
            }
        };
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|source| CacheError::Io {
                path: path.clone(),
                source,
            })?;
        match file.try_lock() {
            Ok(()) => Ok(Self {
                path: CachePath(path),
                _lease: file,
            }),
            Err(TryLockError::WouldBlock) => Err(CacheError::InUse { path }),
            Err(TryLockError::Error(source)) => Err(CacheError::Io { path, source }),
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path.0
    }

    fn io_error(&self, source: std::io::Error) -> CacheError {
        CacheError::Io {
            path: self.path.0.clone(),
            source,
        }
    }
}

fn parent_of(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

/// A complete replacement map while holding the current map's write lock.
/// No mutation of published memory has occurred yet.
pub(super) struct PreparedSnapshot<'cache> {
    owner: &'cache ExclusiveCache,
    current: MutexGuard<'cache, BTreeMap<String, String>>,
    next: BTreeMap<String, String>,
}

impl<'cache> PreparedSnapshot<'cache> {
    pub(super) fn new(cache: &'cache ResponseCache, key: &str, body: String) -> Self {
        let current = match cache.entries.lock() {
            Ok(map) => map,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut next = current.clone();
        next.insert(key.to_owned(), body);
        Self {
            owner: &cache.owner,
            current,
            next,
        }
    }

    /// Write and flush a same-directory temporary file before permitting rename.
    pub(super) fn stage(self) -> Result<DurableSnapshot<'cache>, CacheError> {
        let bytes = serde_json::to_vec(&self.next).map_err(|error| CacheError::Corrupt {
            path: self.owner.path.0.clone(),
            reason: error.to_string(),
        })?;
        let mut file = tempfile::NamedTempFile::new_in(parent_of(self.owner.path()))
            .map_err(|error| self.owner.io_error(error))?;
        file.write_all(&bytes)
            .map_err(|error| self.owner.io_error(error))?;
        file.as_file()
            .sync_all()
            .map_err(|error| self.owner.io_error(error))?;
        Ok(DurableSnapshot {
            prepared: self,
            file,
        })
    }
}

/// A fully written, flushed temporary file, still holding the memory lock.
pub(super) struct DurableSnapshot<'cache> {
    prepared: PreparedSnapshot<'cache>,
    file: tempfile::NamedTempFile,
}

impl<'cache> DurableSnapshot<'cache> {
    /// Publish to disk first, then update memory to the same visible snapshot.
    pub(super) fn publish(self) -> Result<PublishedSnapshot<'cache>, CacheError> {
        let Self { mut prepared, file } = self;
        file.persist(prepared.owner.path())
            .map_err(|error| prepared.owner.io_error(error.error))?;
        *prepared.current = prepared.next;
        Ok(PublishedSnapshot {
            owner: prepared.owner,
            _current: prepared.current,
        })
    }
}

/// Disk and memory agree; directory durability may still need confirmation.
pub(super) struct PublishedSnapshot<'cache> {
    owner: &'cache ExclusiveCache,
    _current: MutexGuard<'cache, BTreeMap<String, String>>,
}

impl PublishedSnapshot<'_> {
    /// On Unix, persist the directory entry after the atomic replacement.
    /// Windows has no portable directory-fsync API in `std`; the file itself
    /// was flushed before replacement, but no directory-durability claim is made.
    pub(super) fn confirm_durability(self) -> Result<(), CacheError> {
        #[cfg(unix)]
        File::open(parent_of(self.owner.path()))
            .and_then(|parent| parent.sync_all())
            .map_err(|source| CacheError::PublishedNotDurable {
                path: self.owner.path.0.clone(),
                source,
            })?;
        #[cfg(not(unix))]
        let _ = self.owner;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::ResponseCache;

    /// Fail after the temporary file is flushed but before its rename.
    #[test]
    fn failed_publication_preserves_the_prior_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let active = dir.path().join("active");
        let retained = dir.path().join("retained");
        std::fs::create_dir(&active).unwrap();
        let cache = ResponseCache::open(CachePath(active.join("cache.json"))).unwrap();
        cache.put("old", "old response".into()).unwrap();
        let before = std::fs::read(active.join("cache.json")).unwrap();
        let staged = PreparedSnapshot::new(&cache, "new", "new response".into())
            .stage()
            .unwrap();
        std::fs::rename(&active, &retained).unwrap();
        assert!(staged.publish().is_err());
        assert_eq!(cache.get("new"), None);
        assert_eq!(cache.get("old").as_deref(), Some("old response"));
        assert_eq!(std::fs::read(retained.join("cache.json")).unwrap(), before);
    }
}
