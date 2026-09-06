//! Persistent response cache for the HTTP judgment provider.
//!
//! A JSON object maps request hashes to raw response bodies. Writes prepare a
//! complete snapshot, flush a same-directory temporary file, atomically replace
//! the live file, then publish memory. One handle owns the cache path across
//! processes; share that handle across threads instead of opening it again.
//! Everything that affects the answer (endpoint, model, rendered prompt) is
//! folded into the key by the caller, borrowing `talkbank-cache`'s
//! versioned-key discipline: stale entries MISS, they are never served.

mod publication;

use publication::{ExclusiveCache, PreparedSnapshot};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Filesystem location of the cache file. Newtyped so a cache path is never
/// confused with any other path at a call site.
#[derive(Debug, Clone)]
pub struct CachePath(pub PathBuf);

/// Why the cache could not be opened or written.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// The cache file exists but is not a JSON string-to-string object.
    /// Fail closed: a corrupt cache must be moved aside deliberately, never
    /// silently ignored (that would re-pay every LLM call without telling
    /// the operator why).
    #[error("corrupt cache file {path}: {reason}")]
    Corrupt {
        /// The cache file that failed to parse.
        path: PathBuf,
        /// The parse failure, in human-readable form.
        reason: String,
    },
    /// Another handle or process owns this cache path.
    #[error("cache is already open: {path}; share the existing handle or close it first")]
    InUse {
        /// Canonical path already owned by another handle.
        path: PathBuf,
    },
    /// Replacement is visible in memory and on disk, but directory sync failed.
    #[error(
        "cache replacement is visible at {path}, but durability could not be confirmed: {source}"
    )]
    PublishedNotDurable {
        /// Destination where the replacement is already visible.
        path: PathBuf,
        /// Failure while confirming directory durability.
        #[source]
        source: std::io::Error,
    },
    /// Reading or writing the cache file failed.
    #[error("cache io on {path}: {source}")]
    Io {
        /// The cache file the I/O operation targeted.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// Request-hash keyed, write-through, JSON-file response cache.
///
/// Holds entries in a `Mutex<BTreeMap>` and publishes one complete snapshot per
/// put. The mutex spans preparation through publication, preventing stale
/// snapshots from overwriting newer writes. An OS lock rejects a second opener
/// across threads and processes. Drop the handle before reopening the path.
///
/// File contents are flushed before replacement. Unix also syncs the parent
/// directory; Windows has no portable directory-sync guarantee here. The old
/// live file is never truncated. Lockfiles remain on disk after handle drop.
#[derive(Debug)]
pub struct ResponseCache {
    owner: ExclusiveCache,
    entries: Mutex<BTreeMap<String, String>>,
}

impl ResponseCache {
    /// Open the cache at `path`. A missing file is an empty cache; a file
    /// that exists but does not parse as a JSON string-to-string object is
    /// [`CacheError::Corrupt`] (fail closed, never silently bypassed).
    pub fn open(path: CachePath) -> Result<Self, CacheError> {
        let owner = ExclusiveCache::acquire(path)?;
        let entries = match std::fs::read_to_string(owner.path()) {
            Ok(text) => serde_json::from_str::<BTreeMap<String, String>>(&text).map_err(|e| {
                CacheError::Corrupt {
                    path: owner.path().to_owned(),
                    reason: e.to_string(),
                }
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => {
                return Err(CacheError::Io {
                    path: owner.path().to_owned(),
                    source: e,
                });
            }
        };
        Ok(Self {
            owner,
            entries: Mutex::new(entries),
        })
    }

    /// The cached raw response body for `key`, if present.
    pub fn get(&self, key: &str) -> Option<String> {
        match self.entries.lock() {
            Ok(map) => map.get(key).cloned(),
            // Staging never mutates this map; recovering a poisoned lock
            // still reads the last snapshot published to disk.
            Err(poisoned) => poisoned.into_inner().get(key).cloned(),
        }
    }

    /// Persist a replacement snapshot, then make it visible to lookups.
    /// Before rename, failures preserve disk and memory. A post-rename directory
    /// sync failure returns `PublishedNotDurable`: the new entry is visible,
    /// while crash durability remains unconfirmed.
    pub fn put(&self, key: &str, body: String) -> Result<(), CacheError> {
        PreparedSnapshot::new(self, key, body)
            .stage()?
            .publish()?
            .confirm_durability()
    }
}
