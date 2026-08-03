//! SQLite pool-based cache implementation.
//!
//! `CachePool` wraps a `SqlitePool` with an embedded tokio `Runtime` so that
//! callers (crossbeam worker threads in `validate_parallel.rs`) remain
//! synchronous while database operations run async internally.
//!
//! Every one of those bridges goes through [`blocking::block_on`], never
//! `Runtime::block_on` directly, so that a caller which is itself driving a
//! runtime cannot nest one runtime inside another. See that module for the
//! four-week outage that rule was written from.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use super::blocking;
use super::cache_location;
use super::cache_utils;
use super::error::CacheError;
use super::init_lock::InitLock;
use super::rules_version::RulesVersion;
use super::types::CacheStats;
use super::version_prune::{self, VersionPruneOutcome};
use super::{maintenance_ops, roundtrip_ops, validation_ops};
use crate::{CacheOutcome, ValidationCache};

/// Connection pool backed by sqlx `SqlitePool` with an embedded tokio runtime.
///
/// The `ValidationCache` trait is sync (required by crossbeam worker threads),
/// so `CachePool` holds a dedicated single-threaded tokio runtime and bridges
/// sync ↔ async through [`blocking::block_on`], which is safe to call from any
/// thread including one already driving a runtime.
///
/// Every read and write binds `rules_version` into the `version` column, so a
/// pool only ever sees rows produced under the same validation rule set it was
/// opened with. Rows under any OTHER version are therefore unreachable to it,
/// and opening prunes all but one generation of them; see
/// [`Self::version_prune`].
pub struct CachePool {
    pool: SqlitePool,
    rt: tokio::runtime::Runtime,
    /// Cache-compatibility version bound into every row this pool reads/writes.
    rules_version: RulesVersion,
    /// What the reachability prune did when this pool was opened, so the caller
    /// can tell an operator rather than reclaiming 190 MB in silence.
    version_prune: VersionPruneOutcome,
}

// -- CachePool constructors --------------------------------------------------

impl CachePool {
    /// Create pool at default location (~/.cache/talkbank-chat).
    ///
    /// Keyed to the validation rule set compiled into this binary
    /// ([`RulesVersion::current`]).
    pub fn new() -> Result<Self, CacheError> {
        let cache_dir = cache_location::default_cache_dir()?;
        Self::with_directory(cache_dir)
    }

    /// Open the default cache, `Arc`-wrapped for sharing across worker
    /// threads/validation runs, degrading to `None` on failure instead of
    /// propagating the error.
    ///
    /// `on_error` is invoked with the failure so the caller can present it
    /// however fits their context (a CLI wants an unconditional `eprintln!`
    /// warning; other contexts may prefer `tracing::warn!` or silence). This
    /// exists so every "open the cache or degrade gracefully" call site
    /// shares the same construction and `Option`-collapsing logic instead of
    /// each hand-rolling the same `match`.
    pub fn open_or_else(on_error: impl FnOnce(&CacheError)) -> Option<Arc<Self>> {
        Self::open_or_else_with_rules_version(RulesVersion::current(), on_error)
    }

    /// Open the default cache keyed to an explicit [`RulesVersion`],
    /// `Arc`-wrapped for sharing across worker threads/validation runs,
    /// degrading to `None` on failure. Same error-reporting contract as
    /// [`Self::open_or_else`] (which is now expressed in terms of this
    /// method, passing [`RulesVersion::current`]).
    ///
    /// Exists so a caller whose active `RuleSelection` turns on opt-in checks
    /// (which changes which files count as Valid) can key the pool to a version
    /// that reflects that, via
    /// [`RulesVersion::current_with_rule_selection`], instead of always
    /// deriving the version from the compiled-in rule set alone.
    pub fn open_or_else_with_rules_version(
        rules_version: RulesVersion,
        on_error: impl FnOnce(&CacheError),
    ) -> Option<Arc<Self>> {
        match cache_location::default_cache_dir()
            .and_then(|cache_dir| Self::with_directory_and_rules_version(cache_dir, rules_version))
        {
            Ok(cache) => Some(Arc::new(cache)),
            Err(error) => {
                on_error(&error);
                None
            }
        }
    }

    /// Create pool at specified directory, keyed to the current rule set.
    pub fn with_directory(cache_dir: PathBuf) -> Result<Self, CacheError> {
        Self::with_directory_and_rules_version(cache_dir, RulesVersion::current())
    }

    /// Create pool at a directory keyed to an explicit [`RulesVersion`].
    ///
    /// Production callers use [`Self::with_directory`] (which derives the
    /// version from the active rule set). This variant exists so tests can
    /// stand up two caches over the same directory under different rule
    /// versions to exercise rule-change invalidation.
    pub fn with_directory_and_rules_version(
        cache_dir: PathBuf,
        rules_version: RulesVersion,
    ) -> Result<Self, CacheError> {
        std::fs::create_dir_all(&cache_dir).map_err(|source| CacheError::Io {
            path: cache_dir.display().to_string(),
            source,
        })?;

        let db_path = cache_location::cache_db_path(&cache_dir);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| CacheError::Message(format!("failed to create tokio runtime: {e}")))?;

        // Serialize the one-time create + WAL setup + migrate against every
        // concurrent opener (other threads AND other processes) with an
        // exclusive advisory file lock beside the database. Exactly one
        // opener initializes; the rest wait boundedly, then connect to a
        // ready database where the migrator no-ops. See `init_lock` module
        // docs for the race this closes and the incident history.
        let init_lock = InitLock::acquire(&cache_dir)?;
        let pool = blocking::block_on(&rt, Self::open_file_pool(&db_path))?;
        // Release before maintenance: the lock guards initialization only.
        // `clean_expired` is an ordinary write, serialized like any other
        // by WAL + busy_timeout, and may be slow on a large cache.
        drop(init_lock);

        // Run expired entry cleanup eagerly so DB is ready before worker threads start.
        blocking::block_on(&rt, Self::clean_expired(&pool))?;

        // Then drop what no reader can bind. Both passes run here, before any
        // worker thread starts, and they answer different questions: the one
        // above deletes what is STALE, this one deletes what is UNREACHABLE.
        let version_prune = blocking::block_on(
            &rt,
            version_prune::prune_unreachable_versions(
                &pool,
                Some(db_path.as_path()),
                &rules_version,
            ),
        )?;

        Ok(Self {
            pool,
            rt,
            rules_version,
            version_prune,
        })
    }

    /// Create in-memory pool for testing or disabled mode.
    ///
    /// Uses `max_connections(1)` because sqlx in-memory SQLite creates a
    /// separate database per connection, pool of 1 ensures a shared database.
    /// Keyed to the current rule set.
    pub fn in_memory() -> Result<Self, CacheError> {
        Self::in_memory_with_rules_version(RulesVersion::current())
    }

    /// Create an in-memory pool keyed to an explicit [`RulesVersion`].
    ///
    /// Test-support counterpart of [`Self::in_memory`]; see
    /// [`Self::with_directory_and_rules_version`] for why an injected version
    /// matters.
    pub fn in_memory_with_rules_version(rules_version: RulesVersion) -> Result<Self, CacheError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| CacheError::Message(format!("failed to create tokio runtime: {e}")))?;

        let pool = blocking::block_on(&rt, async {
            let options = SqliteConnectOptions::from_str("sqlite::memory:")
                .map_err(|source| CacheError::InitDatabase { source })?;

            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .map_err(|source| CacheError::InitDatabase { source })?;

            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .map_err(CacheError::Migration)?;

            Ok::<_, CacheError>(pool)
        })?;

        Ok(Self {
            pool,
            rt,
            rules_version,
            // An in-memory database is created empty on every open, so there is
            // never anything unreachable in it to prune.
            version_prune: VersionPruneOutcome::NothingUnreachable,
        })
    }

    /// Open a file-backed pool with WAL mode + PRAGMAs, applying migrations.
    ///
    /// Openers racing a FRESH database collide two ways: first-connection WAL
    /// setup (surfacing as `InitDatabase`), and the migration (both apply
    /// version 1 -> `UNIQUE constraint failed: _sqlx_migrations.version`,
    /// because sqlx's SQLite migrator has no cross-connection lock). WAL +
    /// `busy_timeout` serialize steady-state reads/writes but NOT this
    /// one-time init. The PRIMARY defence is the exclusive advisory
    /// [`InitLock`] the caller holds around this whole function, which
    /// serializes initialization across threads and processes. The bounded
    /// RETRY below is retained as a backstop for openers that do not honor
    /// the lock protocol (an older build sharing the same cache directory):
    /// once any winner has created + migrated the db, a re-attempt connects
    /// to a ready db and the migration no-ops. A genuine failure surfaces
    /// after the attempts. The in-memory pool is per-connection and never
    /// shared, so it is not affected.
    async fn open_file_pool(db_path: &Path) -> Result<SqlitePool, CacheError> {
        // Bounded so a persistent (non-race) failure still terminates; the total
        // backoff budget comfortably covers a winner creating + migrating the db.
        const MAX_ATTEMPTS: u32 = 16;
        const BACKOFF: std::time::Duration = std::time::Duration::from_millis(15);

        let mut attempt: u32 = 0;
        loop {
            match Self::try_open_file_pool(db_path).await {
                Ok(pool) => return Ok(pool),
                Err(error) => {
                    attempt += 1;
                    if attempt >= MAX_ATTEMPTS || !Self::is_concurrent_init_race(&error) {
                        return Err(error);
                    }
                    // Let the winning opener finish init before re-attempting.
                    tokio::time::sleep(BACKOFF).await;
                }
            }
        }
    }

    /// One attempt to connect a file-backed pool and apply migrations.
    async fn try_open_file_pool(db_path: &Path) -> Result<SqlitePool, CacheError> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_millis(5000))
            .pragma("cache_size", "-8000")
            .pragma("mmap_size", "268435456");

        let pool = SqlitePoolOptions::new()
            .max_connections(16)
            .connect_with(options)
            .await
            .map_err(|source| CacheError::InitDatabase { source })?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(CacheError::Migration)?;

        Ok(pool)
    }

    /// True for the transient errors a concurrent FRESH-db open can raise: the
    /// first-connection WAL/init collision (`InitDatabase`) and the migration
    /// version race (`_sqlx_migrations` UNIQUE / "already exists"). A persistent
    /// (non-race) failure returns false so it surfaces immediately.
    fn is_concurrent_init_race(error: &CacheError) -> bool {
        match error {
            CacheError::InitDatabase { .. } => true,
            CacheError::Migration(migrate_error) => {
                let message = migrate_error.to_string();
                message.contains("_sqlx_migrations")
                    || message.contains("UNIQUE constraint failed")
                    || message.contains("already exists")
            }
            _ => false,
        }
    }

    /// What the reachability prune did when this pool was opened.
    ///
    /// Exposed rather than logged from in here: a library that prints to a
    /// user's terminal has decided something the caller owns, and the CLI, the
    /// desktop app and a test each want to present this differently. Silence is
    /// not an option though, which is why it is a value the caller must go out
    /// of its way to ignore: a 190 MB reclaim nobody is told about reads as a
    /// fix that did nothing.
    pub fn version_prune(&self) -> &VersionPruneOutcome {
        &self.version_prune
    }

    /// Clean up expired cache entries (older than 30 days).
    async fn clean_expired(pool: &SqlitePool) -> Result<(), CacheError> {
        let now_secs = cache_utils::now_secs()?;
        let cutoff = now_secs.saturating_sub(30 * 86_400) as i64;

        // AGE only. Reachability is a different question with a different
        // answer, and it is handled by `version_prune::prune_unreachable_versions`
        // rather than by widening this cutoff: a row can be recent and dead, or
        // old and live.
        sqlx::query("DELETE FROM file_cache WHERE cached_at < ?1")
            .bind(cutoff)
            .execute(pool)
            .await
            .map_err(|source| CacheError::Database { source })?;

        Ok(())
    }

    // ==================== Validation Operations ====================

    /// Get cached validation result: `Some(true)` = valid, `Some(false)` = invalid, `None` = miss.
    pub fn get_validation(&self, path: &Path, check_alignment: bool) -> Option<bool> {
        blocking::block_on(
            &self.rt,
            validation_ops::get_validation(&self.pool, &self.rules_version, path, check_alignment),
        )
    }

    /// Store validation result as pass/fail.
    pub fn set_validation(
        &self,
        path: &Path,
        check_alignment: bool,
        valid: bool,
    ) -> Result<(), CacheError> {
        blocking::block_on(
            &self.rt,
            validation_ops::set_validation(
                &self.pool,
                &self.rules_version,
                path,
                check_alignment,
                valid,
            ),
        )
    }

    // ==================== Roundtrip Operations ====================

    /// Get cached roundtrip result: `Some(true)` = passed, `Some(false)` = failed, `None` = miss.
    pub fn get_roundtrip(
        &self,
        path: &Path,
        check_alignment: bool,
        parser_kind: &str,
    ) -> Option<bool> {
        blocking::block_on(
            &self.rt,
            roundtrip_ops::get_roundtrip(
                &self.pool,
                &self.rules_version,
                path,
                check_alignment,
                parser_kind,
            ),
        )
    }

    /// Store roundtrip result as pass/fail.
    pub fn set_roundtrip(
        &self,
        path: &Path,
        check_alignment: bool,
        parser_kind: &str,
        passed: bool,
    ) -> Result<(), CacheError> {
        blocking::block_on(
            &self.rt,
            roundtrip_ops::set_roundtrip(
                &self.pool,
                &self.rules_version,
                path,
                check_alignment,
                parser_kind,
                passed,
            ),
        )
    }

    // ==================== Maintenance Operations ====================

    /// Clear cache entries for files matching a path prefix.
    pub fn clear_prefix(&self, prefix: &str) -> Result<usize, CacheError> {
        blocking::block_on(&self.rt, maintenance_ops::clear_prefix(&self.pool, prefix))
    }

    /// Clear the cache entries for an explicit set of files, batched.
    ///
    /// The `--force` seam: one bulk statement per chunk of paths, so a
    /// corpus-sized refresh stays linear (the per-file `clear_prefix`
    /// pattern this replaces was quadratic; v0.5.0 DOA, 2026-07-30).
    pub fn clear_paths(&self, paths: &[std::path::PathBuf]) -> Result<usize, CacheError> {
        let path_strings: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        blocking::block_on(
            &self.rt,
            maintenance_ops::clear_paths(&self.pool, &path_strings),
        )
    }

    /// Clear all cache entries.
    pub fn clear_all(&self) -> Result<(), CacheError> {
        blocking::block_on(&self.rt, maintenance_ops::clear_all(&self.pool))
    }

    /// Purge cache entries for files that no longer exist on disk.
    pub fn purge_nonexistent(&self) -> Result<usize, CacheError> {
        blocking::block_on(&self.rt, maintenance_ops::purge_nonexistent(&self.pool))
    }

    // ==================== Statistics ====================

    /// Get cache statistics.
    pub fn stats(&self) -> Result<CacheStats, CacheError> {
        blocking::block_on(&self.rt, async {
            let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM file_cache")
                .fetch_one(&self.pool)
                .await
                .map_err(|source| CacheError::Database { source })?;

            let cache_dir = cache_location::default_cache_dir()?;

            Ok(CacheStats {
                total_entries: row.0 as usize,
                cache_dir,
            })
        })
    }
}

// -- ValidationCache impl for CachePool --------------------------------------

impl ValidationCache for CachePool {
    /// Look up cached validation outcome for a file.
    fn get(&self, path: &Path, check_alignment: bool) -> Option<CacheOutcome> {
        self.get_validation(path, check_alignment).map(|valid| {
            if valid {
                CacheOutcome::Valid
            } else {
                CacheOutcome::Invalid
            }
        })
    }

    /// Store a validation outcome for a file.
    fn set(&self, path: &Path, check_alignment: bool, outcome: CacheOutcome) -> Result<(), String> {
        self.set_validation(path, check_alignment, outcome == CacheOutcome::Valid)
            .map_err(|err| err.to_string())
    }

    /// Returns roundtrip outcome.
    fn get_roundtrip(
        &self,
        path: &Path,
        check_alignment: bool,
        parser_kind: &str,
    ) -> Option<CacheOutcome> {
        CachePool::get_roundtrip(self, path, check_alignment, parser_kind).map(|passed| {
            if passed {
                CacheOutcome::Valid
            } else {
                CacheOutcome::Invalid
            }
        })
    }

    /// Updates roundtrip outcome.
    fn set_roundtrip(
        &self,
        path: &Path,
        check_alignment: bool,
        parser_kind: &str,
        outcome: CacheOutcome,
    ) -> Result<(), String> {
        CachePool::set_roundtrip(
            self,
            path,
            check_alignment,
            parser_kind,
            outcome == CacheOutcome::Valid,
        )
        .map_err(|err| err.to_string())
    }
}

/// Compile-time assertion that `CachePool` is `Send + Sync`.
fn _assert_cache_pool_send_sync() {
    /// Helper used only for type-checking trait bounds.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CachePool>();
}
