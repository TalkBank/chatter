//! Cross-process advisory lock serializing one-time cache initialization.
//!
//! sqlx's SQLite migrator has NO cross-connection serialization
//! (`Migrate::lock` is a no-op for SQLite, unlike Postgres advisory locks),
//! so two openers racing a FRESH cache database both apply migration
//! version 1 and the loser dies with `UNIQUE constraint failed:
//! _sqlx_migrations.version`; concurrent first-connection WAL setup can
//! collide the same way. WAL mode plus `busy_timeout` serialize steady-state
//! reads and writes, but not this one-time create + migrate window.
//!
//! [`InitLock`] closes that window at the OS level: every opener takes an
//! exclusive advisory lock on a small lockfile beside the database for the
//! duration of pool connect + migrate, so exactly one process performs the
//! first-time initialization and every other process connects to a database
//! that is already migrated (the migrator then no-ops). The lock is advisory
//! and held only across initialization, never across cache operation, so
//! steady-state concurrency is unchanged.
//!
//! Acquisition is a bounded try-lock loop rather than a blocking OS wait: a
//! cache is an accelerator, and its initialization must never be able to
//! hang a caller indefinitely, whatever state another process left the
//! lockfile or database in. If the deadline expires, acquisition fails with
//! a typed error and the caller degrades (for the CLI,
//! `CachePool::open_or_else` warns and runs uncached) instead of blocking.
//!
//! Implemented with `std::fs::File` locking (`try_lock` / `unlock`,
//! stabilized in Rust 1.89): `flock(2)` semantics on Unix, `LockFileEx` on
//! Windows. Both are cross-process, per-handle, and released by the OS when
//! the handle closes even if the holder crashes, so a dead initializer can
//! never strand the lock. No new dependency is needed.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::error::CacheError;

/// Name of the advisory lockfile created beside the cache database.
///
/// The file itself carries no data; only its OS-level lock state matters.
/// It is created once and left in place (removing it while another process
/// holds or is about to take the lock would defeat the serialization).
const INIT_LOCK_FILE_NAME: &str = "talkbank-cache.init.lock";

/// Upper bound on waiting for another process to finish initializing.
///
/// First-time create + migrate takes milliseconds; this bound is generous
/// so a heavily loaded machine still succeeds, while guaranteeing a wedged
/// or pathologically slow holder produces a typed failure instead of an
/// indefinite hang.
const ACQUIRE_DEADLINE: Duration = Duration::from_secs(10);

/// Poll interval between try-lock attempts while waiting for the holder.
const ACQUIRE_POLL: Duration = Duration::from_millis(25);

/// Exclusive advisory lock over cache initialization, held from successful
/// [`InitLock::acquire`] until drop.
///
/// See the module docs for the protocol and the incident history.
pub(crate) struct InitLock {
    /// Open handle to the lockfile; holding it holds the OS lock.
    file: std::fs::File,
    /// Lockfile path, kept for diagnostics on release failure.
    path: PathBuf,
}

impl InitLock {
    /// Acquire the exclusive init lock for `cache_dir`, waiting up to
    /// [`ACQUIRE_DEADLINE`] for a concurrent initializer to finish.
    ///
    /// Errors are typed and specific: lockfile I/O failures surface as
    /// [`CacheError::Io`]; deadline expiry surfaces as
    /// [`CacheError::InitLockTimeout`]. Nothing is swallowed.
    pub(crate) fn acquire(cache_dir: &Path) -> Result<Self, CacheError> {
        let path = cache_dir.join(INIT_LOCK_FILE_NAME);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|source| CacheError::Io {
                path: path.display().to_string(),
                source,
            })?;

        let deadline = Instant::now() + ACQUIRE_DEADLINE;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { file, path }),
                Err(std::fs::TryLockError::WouldBlock) => {
                    // Another process is initializing right now. Wait
                    // boundedly for it to finish, then take our turn.
                    if Instant::now() >= deadline {
                        return Err(CacheError::InitLockTimeout {
                            path: path.display().to_string(),
                        });
                    }
                    std::thread::sleep(ACQUIRE_POLL);
                }
                Err(std::fs::TryLockError::Error(source)) => {
                    return Err(CacheError::Io {
                        path: path.display().to_string(),
                        source,
                    });
                }
            }
        }
    }
}

impl Drop for InitLock {
    fn drop(&mut self) {
        // Explicit unlock for deterministic release; closing the handle
        // (which drop does next) also releases the lock on every supported
        // platform, so a failure here can only delay release, never leak
        // the lock. It is still surfaced rather than swallowed.
        if let Err(error) = self.file.unlock() {
            tracing::warn!(
                lockfile = %self.path.display(),
                %error,
                "failed to explicitly release cache init lock; \
                 the OS releases it when the handle closes"
            );
        }
    }
}
