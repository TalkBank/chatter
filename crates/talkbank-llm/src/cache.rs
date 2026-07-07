//! Persistent response cache for the HTTP judgment provider.
//!
//! Same design as the sibling Python `talkbank_llm.ResponseCache`: a JSON
//! object file mapping request-hash keys to raw response bodies, rewritten
//! on every put so a crashed batch loses at most the in-flight entry.
//! Everything that affects the answer (endpoint, model, rendered prompt) is
//! folded into the key by the caller, borrowing `talkbank-cache`'s
//! versioned-key discipline: stale entries MISS, they are never served.

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
/// Holds every entry in memory (a `Mutex<BTreeMap>`) and rewrites the whole
/// file on every [`ResponseCache::put`], so a crash mid-batch loses at most
/// the in-flight entry, never previously cached ones. `BTreeMap` (rather
/// than `HashMap`) keeps the serialized file byte-stable across runs for
/// the same entry set, useful for diffing a committed or shared cache.
#[derive(Debug)]
pub struct ResponseCache {
    path: CachePath,
    entries: Mutex<BTreeMap<String, String>>,
}

impl ResponseCache {
    /// Open the cache at `path`. A missing file is an empty cache; a file
    /// that exists but does not parse as a JSON string-to-string object is
    /// [`CacheError::Corrupt`] (fail closed, never silently bypassed).
    pub fn open(path: CachePath) -> Result<Self, CacheError> {
        let entries = match std::fs::read_to_string(&path.0) {
            Ok(text) => serde_json::from_str::<BTreeMap<String, String>>(&text).map_err(|e| {
                CacheError::Corrupt {
                    path: path.0.clone(),
                    reason: e.to_string(),
                }
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => {
                return Err(CacheError::Io {
                    path: path.0.clone(),
                    source: e,
                });
            }
        };
        Ok(Self {
            path,
            entries: Mutex::new(entries),
        })
    }

    /// The cached raw response body for `key`, if present.
    pub fn get(&self, key: &str) -> Option<String> {
        match self.entries.lock() {
            Ok(map) => map.get(key).cloned(),
            // A poisoned lock means another thread panicked mid-update;
            // treat as a miss so the caller re-fetches rather than
            // propagating the panic.
            Err(poisoned) => poisoned.into_inner().get(key).cloned(),
        }
    }

    /// Insert `key` -> `body` and persist the whole cache immediately
    /// (write-through, crash-safe like the Python sibling cache).
    pub fn put(&self, key: &str, body: String) -> Result<(), CacheError> {
        let serialized = {
            let mut map = match self.entries.lock() {
                Ok(map) => map,
                Err(poisoned) => poisoned.into_inner(),
            };
            map.insert(key.to_string(), body);
            serde_json::to_string(&*map).map_err(|e| CacheError::Corrupt {
                path: self.path.0.clone(),
                reason: e.to_string(),
            })?
        };
        std::fs::write(&self.path.0, serialized).map_err(|e| CacheError::Io {
            path: self.path.0.clone(),
            source: e,
        })
    }
}
