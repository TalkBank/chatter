//! SQLite pool-based cache implementation.
//!
//! `CachePool` wraps a `SqlitePool` with an embedded tokio `Runtime` so that
//! callers (crossbeam worker threads in `validate_parallel.rs`) remain
//! synchronous while database operations run async internally.
//!
//! Every one of those bridges goes through `blocking::ConfinedRuntime::block_on`, never
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

use super::blocking;
use super::cache_location;
use super::cache_utils;
use super::error::CacheError;
use super::init_lock::InitLock;
use super::types::CacheIdentity;
use super::types::{CacheStats, CacheStorage};
use super::version_prune::{self, VersionPruneOutcome};
use super::{maintenance_ops, roundtrip_ops, validation_ops};
use crate::maintenance_ops::CacheScope;
use crate::trait_def::{CacheLookup, ContentHash, RoundtripOutcome};
use crate::{CacheOutcome, ValidationCache, VerdictReader};
use talkbank_model::ResolvedPath;
use talkbank_model::validation::AlignmentValidation;

/// How long a connection waits on another's lock before failing: shared by
/// every open of a database file, writable or read-only.
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(5000);

/// The pool width for a database file, writable or read-only.
const MAX_CONNECTIONS: u32 = 16;

/// Connection pool backed by sqlx `SqlitePool` with an embedded tokio runtime.
///
/// The `ValidationCache` trait is sync (required by crossbeam worker threads),
/// so `CachePool` holds a dedicated single-threaded tokio runtime and bridges
/// sync ↔ async through `blocking::ConfinedRuntime::block_on`, which is safe to call from any
/// thread including one already driving a runtime.
///
/// Every read and write binds `rules_version` into the `version` column, so a
/// pool only ever sees rows produced under the same validation rule set it was
/// opened with. Rows under any OTHER version are therefore unreachable to it,
/// and opening prunes all but one generation of them; see
/// [`Self::version_prune`].
pub struct CachePool<S = ValidationScope> {
    pool: SqlitePool,
    rt: blocking::ConfinedRuntime,
    scope: S,
    /// Where the database lives, fixed when the pool opened.
    storage: CacheStorage,
}

/// Admitted validation namespace and the retention result from opening it.
/// Only a validation cache can serve or record validation/roundtrip verdicts.
pub struct ValidationScope {
    identity: CacheIdentity,
    version_prune: VersionPruneOutcome,
}

/// A validation identity opened to READ verdicts only: no maintenance ran
/// when it opened (no expiry, no generation prune) and it has no way to
/// write a verdict or delete a row. What an audit, a reporting sweep, uses.
pub struct ReadOnlyScope {
    identity: CacheIdentity,
}

/// A cache handle that can only read verdicts.
pub type ReadOnlyCache = CachePool<ReadOnlyScope>;

/// The whole cache opened to INSPECT only: counts and statistics, read-only,
/// for no identity. Opening it writes nothing to the database, as
/// [`ReadOnlyScope`]'s open does not (SQLite may still create its
/// shared-memory and write-ahead-log files beside an existing database); it
/// serves no verdict and deletes no row. Reached only as
/// [`CacheOnDisk::Current`], from [`CacheOnDisk::inspect`]; what `cache
/// stats` and a preview (`cache clear --dry-run`) read.
pub struct InspectionScope {
    /// The directory inspected: an inspection is always of a directory's
    /// database, which [`InspectionCache::into_maintenance`] reopens.
    cache_dir: PathBuf,
}

/// A cache handle that counts entries and reports statistics, and nothing
/// else; see [`InspectionScope`].
pub type InspectionCache = CachePool<InspectionScope>;

/// A scope whose handle reads verdicts for one identity.
trait IdentityScope {
    fn identity(&self) -> &CacheIdentity;
}

impl IdentityScope for ValidationScope {
    fn identity(&self) -> &CacheIdentity {
        &self.identity
    }
}

impl IdentityScope for ReadOnlyScope {
    fn identity(&self) -> &CacheIdentity {
        &self.identity
    }
}

/// A scope whose handle may delete rows: a validation cache, or an explicit
/// maintenance handle. Never a read-only one.
trait WritableScope {}
impl WritableScope for ValidationScope {}
impl WritableScope for MaintenanceScope {}

/// Administrative access carries no validation generation or parser identity.
/// It cannot serve verdicts or prune generations based on a guessed identity.
pub struct MaintenanceScope;

/// A cache handle restricted to statistics and explicit maintenance operations.
///
/// Reached only from what [`CacheOnDisk::inspect`] found: a current cache
/// ([`InspectionCache::into_maintenance`]) or one of an older schema
/// ([`OlderSchema::migrate`]). There is no handle for a directory with no
/// database, so maintenance never creates a cache to act on.
pub type MaintenanceCache = CachePool<MaintenanceScope>;

/// What is in a cache directory, found by one read-only look that creates,
/// migrates and writes nothing: the value a preview and the operation it
/// previews both start from, so the two cannot disagree about the schema.
///
/// A database written by a NEWER build is not a state here but an error
/// ([`CacheError::SchemaNewer`]): this build can neither read nor migrate it.
pub enum CacheOnDisk {
    /// The directory holds no database.
    Absent(NoDatabase),
    /// A database whose schema is older than this build's, or that no build
    /// migrated; [`OlderSchema::migrate`] brings it current.
    OlderSchema(OlderSchema),
    /// A database at this build's schema, open read-only.
    Current(InspectionCache),
}

/// A cache directory with no database in it (perhaps no directory at all).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoDatabase {
    cache_dir: PathBuf,
}

impl NoDatabase {
    /// The directory looked in.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// The database file that is not there.
    pub fn database(&self) -> PathBuf {
        cache_location::cache_db_path(&self.cache_dir)
    }
}

/// A cache directory whose database has an older schema than this build's.
/// Nothing reads it until [`Self::migrate`], the one way forward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OlderSchema {
    cache_dir: PathBuf,
}

impl OlderSchema {
    /// The cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// The database file whose schema is older.
    pub fn database(&self) -> PathBuf {
        cache_location::cache_db_path(&self.cache_dir)
    }

    /// Migrate the database to this build's schema, under the initialization
    /// lock, and open it for maintenance. Migration can delete rows (a later
    /// schema's unique index drops duplicates), so what it holds afterwards
    /// is counted on the handle this returns, never before.
    pub fn migrate(self) -> Result<MaintenanceCache, CacheError> {
        MaintenanceCache::open_writable(self.cache_dir)
    }
}

impl CacheOnDisk {
    /// Look at the default cache directory; see [`Self::inspect_directory`].
    pub fn inspect() -> Result<Self, CacheError> {
        Self::inspect_directory(cache_location::default_cache_dir()?)
    }

    /// Look at the cache in `cache_dir` and write nothing: no directory, lock
    /// file or database is created and no migration runs. A current database
    /// is opened read-only; SQLite may still create its shared-memory and
    /// write-ahead-log files beside it, which every reader of a WAL database
    /// needs.
    pub fn inspect_directory(cache_dir: PathBuf) -> Result<Self, CacheError> {
        Ok(match look_read_only(&cache_dir)? {
            Found::Absent => Self::Absent(NoDatabase { cache_dir }),
            Found::OlderSchema => Self::OlderSchema(OlderSchema { cache_dir }),
            Found::Current(OpenedDatabase { pool, rt }) => Self::Current(CachePool {
                pool,
                rt,
                storage: CacheStorage::Directory(cache_dir.clone()),
                scope: InspectionScope { cache_dir },
            }),
        })
    }
}

/// What a writable open may do when the directory holds no database.
#[derive(Debug, Clone, Copy)]
enum WhenMissing {
    /// Create the directory and the database: a validation cache, which is
    /// where a cache comes into being.
    Create,
    /// Refuse ([`CacheError::NoCacheDatabase`]): maintenance, which acts only
    /// on a database [`CacheOnDisk::inspect`] found, even if it vanished
    /// between that look and this open.
    Refuse,
}

/// Storage admitted through the initialization lock and migrations.
struct OpenedDatabase {
    pool: SqlitePool,
    rt: blocking::ConfinedRuntime,
}

/// The runtime every pool bridges its async database work through.
///
/// One helper rather than the same sixteen lines in both constructors, which is
/// where it started.
///
/// MULTI-THREAD, with one worker, deliberately. A current-thread runtime only
/// advances while someone calls `Runtime::block_on` on it, and
/// [`blocking::ConfinedRuntime`] hands ownership to a thread that PARKS rather
/// than driving it, so `Handle::block_on` from a caller would queue the task
/// and then wait forever for nobody to run it. Measured 2026-08-04: that
/// deadlocked the suite outright. One worker thread drives the runtime itself,
/// which is what makes `Handle::block_on` work from any caller.
fn confined_runtime() -> Result<blocking::ConfinedRuntime, CacheError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .map_err(|e| CacheError::Message(format!("failed to create tokio runtime: {e}")))?;
    blocking::ConfinedRuntime::new(runtime)
        .map_err(|e| CacheError::Message(format!("failed to spawn the runtime owner: {e}")))
}

// -- CachePool constructors --------------------------------------------------

impl CachePool {
    /// Open the default directory for a fully specified validation identity.
    pub fn new(identity: CacheIdentity) -> Result<Self, CacheError> {
        Self::with_directory(cache_location::default_cache_dir()?, identity)
    }

    /// Open a directory for one rule generation and parser namespace.
    pub fn with_directory(cache_dir: PathBuf, identity: CacheIdentity) -> Result<Self, CacheError> {
        let OpenedDatabase { pool, rt } =
            Self::open_directory_storage(&cache_dir, WhenMissing::Create)?;
        let storage = CacheStorage::Directory(cache_dir);

        // Run expired entry cleanup eagerly so DB is ready before worker threads start.
        rt.block_on(Self::clean_expired(&pool))?;

        // Then drop what no reader can bind. Both passes run here, before any
        // worker thread starts, and they answer different questions: the one
        // above deletes what is STALE, this one deletes what is UNREACHABLE.
        let version_prune = rt.block_on(version_prune::prune_unreachable_versions(
            &pool,
            &storage,
            identity.rules_version(),
        ))?;

        Ok(Self {
            pool,
            rt,
            storage,
            scope: ValidationScope {
                identity,
                version_prune,
            },
        })
    }

    /// Create a single-connection in-memory cache for an explicit identity.
    pub fn in_memory(identity: CacheIdentity) -> Result<Self, CacheError> {
        let rt = confined_runtime()?;

        let pool = rt.block_on(async {
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
            storage: CacheStorage::InMemory,
            scope: ValidationScope {
                identity,
                version_prune: VersionPruneOutcome::FreshDatabase,
            },
        })
    }
}

impl<S> CachePool<S> {
    fn open_directory_storage(
        cache_dir: &Path,
        when_missing: WhenMissing,
    ) -> Result<OpenedDatabase, CacheError> {
        match when_missing {
            WhenMissing::Create => {
                std::fs::create_dir_all(cache_dir).map_err(|source| CacheError::Io {
                    path: cache_dir.display().to_string(),
                    source,
                })?
            }
            // A vanished directory fails the lock below; nothing recreates it.
            WhenMissing::Refuse => {}
        }

        let db_path = cache_location::cache_db_path(cache_dir);

        let rt = confined_runtime()?;

        // Serialize the one-time create + WAL setup + migrate against every
        // concurrent opener (other threads AND other processes) with an
        // exclusive advisory file lock beside the database. Exactly one
        // opener initializes; the rest wait boundedly, then connect to a
        // ready database where the migrator no-ops. See `init_lock` module
        // docs for the race this closes and the incident history.
        let init_lock = InitLock::acquire(cache_dir)?;
        // Under the lock, so no other opener can create it in between.
        match (when_missing, db_path.try_exists()) {
            (WhenMissing::Create, _) | (WhenMissing::Refuse, Ok(true)) => {}
            (WhenMissing::Refuse, Ok(false)) => {
                return Err(CacheError::NoCacheDatabase {
                    path: db_path.display().to_string(),
                });
            }
            (WhenMissing::Refuse, Err(source)) => {
                return Err(CacheError::Io {
                    path: db_path.display().to_string(),
                    source,
                });
            }
        }
        let pool = rt.block_on(Self::open_file_pool(&db_path, when_missing))?;
        // Release before maintenance: the lock guards initialization only.
        // `clean_expired` is an ordinary write, serialized like any other
        // by WAL + busy_timeout, and may be slow on a large cache.
        drop(init_lock);

        Ok(OpenedDatabase { pool, rt })
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
    async fn open_file_pool(
        db_path: &Path,
        when_missing: WhenMissing,
    ) -> Result<SqlitePool, CacheError> {
        // Bounded so a persistent (non-race) failure still terminates; the total
        // backoff budget comfortably covers a winner creating + migrating the db.
        const MAX_ATTEMPTS: u32 = 16;
        const BACKOFF: std::time::Duration = std::time::Duration::from_millis(15);

        let mut attempt: u32 = 0;
        loop {
            match Self::try_open_file_pool(db_path, when_missing).await {
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
    async fn try_open_file_pool(
        db_path: &Path,
        when_missing: WhenMissing,
    ) -> Result<SqlitePool, CacheError> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(match when_missing {
                WhenMissing::Create => true,
                WhenMissing::Refuse => false,
            })
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT)
            .pragma("cache_size", "-8000")
            .pragma("mmap_size", "268435456");

        let pool = SqlitePoolOptions::new()
            .max_connections(MAX_CONNECTIONS)
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

    /// Clean up expired cache entries (older than 30 days).
    async fn clean_expired(pool: &SqlitePool) -> Result<(), CacheError> {
        let cutoff = cache_utils::CachedAt::now().days_before(30);

        // AGE only. Reachability is a different question with a different
        // answer, and it is handled by `version_prune::prune_unreachable_versions`
        // rather than by widening this cutoff: a row can be recent and dead, or
        // old and live.
        sqlx::query("DELETE FROM file_cache WHERE cached_at < ?1")
            .bind(cutoff.column())
            .execute(pool)
            .await
            .map_err(|source| CacheError::Database { source })?;

        Ok(())
    }
}

impl CachePool {
    /// What the reachability prune did when this pool was opened.
    ///
    /// Exposed rather than logged from in here: a library that prints to a
    /// user's terminal has decided something the caller owns, and the CLI, the
    /// desktop app and a test each want to present this differently. Silence is
    /// not an option though, which is why it is a value the caller must go out
    /// of its way to ignore: a 190 MB reclaim nobody is told about reads as a
    /// fix that did nothing.
    pub fn version_prune(&self) -> &VersionPruneOutcome {
        &self.scope.version_prune
    }
}

impl<S> CachePool<S> {
    /// How many entries the scope covers, from the database alone: no
    /// filesystem read, so a caller that needs only a count cannot fail on
    /// the database file's metadata.
    pub fn count(&self, scope: &CacheScope) -> Result<usize, CacheError> {
        self.rt.block_on(maintenance_ops::count(&self.pool, scope))
    }

    /// Get cache statistics: the entry count, and the storage's own facts
    /// (for a directory cache, its database file's size and modification
    /// time, read now).
    pub fn stats(&self) -> Result<CacheStats, CacheError> {
        Ok(CacheStats {
            total_entries: self.count(&CacheScope::All)?,
            storage: self.storage.stats()?,
        })
    }
}

/// Deleting rows: a validation cache or a maintenance handle, never a
/// read-only one.
#[allow(private_bounds)]
impl<S: WritableScope> CachePool<S> {
    /// Delete the entries the scope covers; returns how many the delete
    /// removed, from the same statement.
    pub fn clear(&self, scope: &CacheScope) -> Result<usize, CacheError> {
        self.rt.block_on(maintenance_ops::clear(&self.pool, scope))
    }

    /// Clear every row for an explicit set of files, by KEY, in every
    /// namespace a row can be written under, batched.
    ///
    /// The `--force` seam. By key, not by the `file_path` text, so it clears
    /// exactly the rows the worker wrote for these files (both key by the
    /// file's [`ResolvedPath`]), and two non-UTF-8 names that share a lossy
    /// spelling cannot clear each other's rows. One bulk statement per
    /// chunk, so a corpus-sized refresh stays linear in the file count.
    pub fn clear_paths<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a ResolvedPath>,
    ) -> Result<usize, CacheError> {
        let keys: Vec<cache_utils::CacheKey> = paths
            .into_iter()
            .flat_map(|path| {
                cache_utils::KeyNamespace::every_written()
                    .map(|namespace| cache_utils::CacheKey::of(path, namespace))
            })
            .collect();
        self.rt
            .block_on(maintenance_ops::clear_keys(&self.pool, &keys))
    }

    /// Purge cache entries for files that no longer exist on disk.
    pub fn purge_nonexistent(&self) -> Result<usize, CacheError> {
        self.rt
            .block_on(maintenance_ops::purge_nonexistent(&self.pool))
    }
}

impl ReadOnlyCache {
    /// Open the default cache to read one identity's verdicts.
    pub fn open(identity: CacheIdentity) -> Result<Self, CacheError> {
        Self::open_directory(cache_location::default_cache_dir()?, identity)
    }

    /// Open an EXISTING cache in a directory to read one identity's
    /// verdicts, and write nothing: no directory, lock file or database is
    /// created, no migration runs, no expired row or generation is pruned,
    /// and the connection is read-only. A cache that does not exist, or whose
    /// schema is older than this build's, is refused
    /// ([`CacheError::NoCacheDatabase`], [`CacheError::SchemaNotCurrent`]);
    /// a writing run creates or upgrades it. One a newer build wrote is
    /// [`CacheError::SchemaNewer`]. The look is [`CacheOnDisk::inspect`]'s.
    /// SQLite may still create its shared-memory and write-ahead-log files
    /// beside an existing database, which every reader of a WAL database
    /// needs.
    pub fn open_directory(cache_dir: PathBuf, identity: CacheIdentity) -> Result<Self, CacheError> {
        let path = || {
            cache_location::cache_db_path(&cache_dir)
                .display()
                .to_string()
        };
        let OpenedDatabase { pool, rt } = match look_read_only(&cache_dir)? {
            Found::Current(opened) => opened,
            Found::Absent => return Err(CacheError::NoCacheDatabase { path: path() }),
            Found::OlderSchema => return Err(CacheError::SchemaNotCurrent { path: path() }),
        };
        Ok(Self {
            pool,
            rt,
            scope: ReadOnlyScope { identity },
            storage: CacheStorage::Directory(cache_dir),
        })
    }
}

impl InspectionCache {
    /// Reopen this current cache writable, for maintenance. Its schema is
    /// this build's, so the open migrates nothing; it does take the
    /// initialization lock, as every writable open does.
    pub fn into_maintenance(self) -> Result<MaintenanceCache, CacheError> {
        let Self {
            pool, rt, scope, ..
        } = self;
        // The read-only pool closes, its connections with it, before the
        // writable one opens.
        rt.block_on(pool.close());
        MaintenanceCache::open_writable(scope.cache_dir)
    }
}

/// What the one read-only look at a cache directory found. Private: callers
/// see it as a [`CacheOnDisk`], or, for a verdict reader, as the errors
/// [`ReadOnlyCache::open_directory`] maps the non-current states to.
enum Found {
    /// No database file.
    Absent,
    /// A database whose schema is older than this build's.
    OlderSchema,
    /// A database at this build's schema, connected read-only.
    Current(OpenedDatabase),
}

/// Look at the database in `cache_dir` and connect read-only when its schema
/// is this build's, writing nothing. A newer schema is
/// [`CacheError::SchemaNewer`].
fn look_read_only(cache_dir: &Path) -> Result<Found, CacheError> {
    let db_path = cache_location::cache_db_path(cache_dir);
    match db_path.try_exists() {
        Ok(true) => {}
        Ok(false) => return Ok(Found::Absent),
        Err(source) => {
            return Err(CacheError::Io {
                path: db_path.display().to_string(),
                source,
            });
        }
    }
    let rt = confined_runtime()?;
    Ok(match rt.block_on(connect_read_only(&db_path))? {
        Ledger::Older => Found::OlderSchema,
        Ledger::Current(pool) => Found::Current(OpenedDatabase { pool, rt }),
    })
}

/// What an existing database's migration ledger says against this build's.
enum Ledger {
    /// Older than this build's, or no build migrated it.
    Older,
    /// This build's, with the read-only pool that read it.
    Current(SqlitePool),
}

/// Connect read-only to an existing database and read its ledger:
/// [`CacheError::SchemaNewer`] when a newer build migrated it past every
/// migration this build knows.
async fn connect_read_only(db_path: &Path) -> Result<Ledger, CacheError> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .read_only(true)
        .create_if_missing(false)
        .busy_timeout(BUSY_TIMEOUT);
    let pool = SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect_with(options)
        .await
        .map_err(|source| CacheError::InitDatabase { source })?;
    let expected = sqlx::migrate!("./migrations")
        .iter()
        .map(|migration| migration.version)
        .max();
    let database = |source| CacheError::Database { source };
    // A database no build migrated has no ledger at all: as out of date
    // as one an older build left behind.
    let ledgers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(&pool)
    .await
    .map_err(database)?;
    let applied: Option<i64> = match ledgers {
        0 => None,
        _ => sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(&pool)
            .await
            .map_err(database)?,
    };
    // `None` on either side is "nothing applied" or "nothing known", which
    // compares below every version: an unmigrated database is older.
    match applied.cmp(&expected) {
        std::cmp::Ordering::Equal => Ok(Ledger::Current(pool)),
        std::cmp::Ordering::Less => Ok(Ledger::Older),
        std::cmp::Ordering::Greater => Err(CacheError::SchemaNewer {
            path: db_path.display().to_string(),
        }),
    }
}

impl MaintenanceCache {
    /// Open the EXISTING database in `cache_dir` writable, without
    /// expiration or generation pruning. Migrations run under the
    /// initialization lock before queries are permitted. A database that
    /// vanished since the look is [`CacheError::NoCacheDatabase`], never
    /// recreated. Private: the routes here are the transitions from what
    /// [`CacheOnDisk::inspect`] found, which never name an absent database.
    fn open_writable(cache_dir: PathBuf) -> Result<Self, CacheError> {
        let OpenedDatabase { pool, rt, .. } =
            Self::open_directory_storage(&cache_dir, WhenMissing::Refuse)?;
        Ok(Self {
            pool,
            rt,
            scope: MaintenanceScope,
            storage: CacheStorage::Directory(cache_dir),
        })
    }
}

// -- Verdict reads and writes ------------------------------------------------

#[allow(private_bounds)]
impl<S: IdentityScope + Send + Sync> VerdictReader for CachePool<S> {
    fn identity(&self) -> &CacheIdentity {
        self.scope.identity()
    }

    fn get(
        &self,
        path: &ResolvedPath,
        content: &ContentHash,
        alignment: AlignmentValidation,
    ) -> Result<CacheLookup<CacheOutcome>, CacheError> {
        self.rt.block_on(validation_ops::get_validation(
            &self.pool,
            self.scope.identity(),
            path,
            content,
            alignment,
        ))
    }

    fn get_roundtrip(
        &self,
        path: &ResolvedPath,
        content: &ContentHash,
        alignment: AlignmentValidation,
    ) -> Result<CacheLookup<RoundtripOutcome>, CacheError> {
        self.rt.block_on(roundtrip_ops::get_roundtrip(
            &self.pool,
            self.scope.identity(),
            path,
            content,
            alignment,
        ))
    }
}

impl ValidationCache for CachePool {
    fn set(
        &self,
        path: &ResolvedPath,
        content: &ContentHash,
        alignment: AlignmentValidation,
        outcome: CacheOutcome,
    ) -> Result<(), CacheError> {
        self.rt.block_on(validation_ops::set_validation(
            &self.pool,
            &self.scope.identity,
            path,
            content,
            alignment,
            outcome,
        ))
    }

    fn set_roundtrip(
        &self,
        path: &ResolvedPath,
        content: &ContentHash,
        alignment: AlignmentValidation,
        outcome: RoundtripOutcome,
    ) -> Result<(), CacheError> {
        self.rt.block_on(roundtrip_ops::set_roundtrip(
            &self.pool,
            &self.scope.identity,
            path,
            content,
            alignment,
            outcome,
        ))
    }
}

/// Compile-time assertion that `CachePool` is `Send + Sync`.
fn _assert_cache_pool_send_sync() {
    /// Helper used only for type-checking trait bounds.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CachePool>();
}
