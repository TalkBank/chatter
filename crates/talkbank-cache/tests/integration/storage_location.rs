//! Statistics must describe the storage opened by this handle.

use talkbank_cache::{
    CacheError, CacheIdentity, CacheOnDisk, CachePool, DatabaseFile, InspectionCache, ParserKind,
    ReadOnlyCache, RulesVersion, StorageStats,
};

/// What an inspection of `dir` found, expected to be a current cache.
fn current(dir: &std::path::Path) -> InspectionCache {
    match CacheOnDisk::inspect_directory(dir.to_path_buf()).unwrap() {
        CacheOnDisk::Current(cache) => cache,
        CacheOnDisk::Absent(_) => panic!("no database in {dir:?}"),
        CacheOnDisk::OlderSchema(_) => panic!("an older schema in {dir:?}"),
    }
}

/// A directory cache reports the directory it opened and its database file,
/// which opening created, through the validation, inspection and
/// maintenance handles.
#[test]
fn explicit_directory_statistics_retain_opened_location() {
    let dir = tempfile::tempdir().unwrap();
    let identity = CacheIdentity::new(
        RulesVersion::for_testing("location"),
        ParserKind::TreeSitter,
    );
    let cache = CachePool::with_directory(dir.path().to_path_buf(), identity).unwrap();
    let inspection = current(dir.path());
    let inspected = inspection.stats().unwrap().storage;
    let maintenance = inspection.into_maintenance().unwrap();
    for storage in [
        cache.stats().unwrap().storage,
        inspected,
        maintenance.stats().unwrap().storage,
    ] {
        match storage {
            StorageStats::Directory {
                cache_dir,
                database: DatabaseFile::Present { size_bytes, .. },
                ..
            } => {
                assert_eq!(cache_dir, dir.path());
                assert!(size_bytes > 0, "an opened database file is not empty");
            }
            other => panic!("expected the opened directory with its file, got {other:?}"),
        }
    }
}

#[test]
fn in_memory_statistics_have_no_filesystem_location() {
    let identity = CacheIdentity::new(
        RulesVersion::for_testing("location"),
        ParserKind::TreeSitter,
    );
    let cache = CachePool::in_memory(identity).unwrap();
    let stats = cache.stats().unwrap();
    assert_eq!(stats.storage, StorageStats::InMemory);
    assert_eq!(stats.total_entries, 0);
}

/// A read-only open writes nothing: with no cache it creates no directory,
/// lock or database and refuses; over a database no build migrated it
/// refuses without migrating it.
#[test]
fn a_read_only_open_creates_and_migrates_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let identity = || {
        CacheIdentity::new(
            RulesVersion::for_testing("read-only"),
            ParserKind::TreeSitter,
        )
    };
    let absent = dir.path().join("absent");
    assert!(matches!(
        ReadOnlyCache::open_directory(absent.clone(), identity()),
        Err(CacheError::NoCacheDatabase { .. })
    ));
    assert!(!absent.exists(), "a read-only open created its directory");

    let unmigrated = dir.path().join("unmigrated");
    std::fs::create_dir_all(&unmigrated).unwrap();
    let database = talkbank_cache::cache_db_path(&unmigrated);
    std::fs::write(&database, b"").unwrap();
    assert!(matches!(
        ReadOnlyCache::open_directory(unmigrated.clone(), identity()),
        Err(CacheError::SchemaNotCurrent { .. })
    ));
    assert_eq!(std::fs::metadata(&database).unwrap().len(), 0, "migrated");
    let entries: Vec<_> = std::fs::read_dir(&unmigrated)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(entries, [database.file_name().unwrap().to_owned()]);

    // A cache a writing run made is read.
    let made = dir.path().join("made");
    CachePool::with_directory(made.clone(), identity())
        .unwrap()
        .close();
    ReadOnlyCache::open_directory(made, identity())
        .expect("an existing cache opens")
        .close();
}

/// One look tells the three states of a cache directory apart and writes
/// nothing: no database (nothing created), an older schema (not migrated),
/// a current one. Only the older schema's own transition migrates it, and a
/// schema newer than this build's is refused rather than called older.
#[test]
fn an_inspection_tells_the_directory_states_apart_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let absent = dir.path().join("absent");
    match CacheOnDisk::inspect_directory(absent.clone()).unwrap() {
        CacheOnDisk::Absent(found) => assert_eq!(found.cache_dir(), absent),
        _ => panic!("an absent directory holds no database"),
    }
    assert!(!absent.exists(), "an inspection created its directory");

    let older = dir.path().join("older");
    std::fs::create_dir_all(&older).unwrap();
    let database = talkbank_cache::cache_db_path(&older);
    std::fs::write(&database, b"").unwrap();
    let CacheOnDisk::OlderSchema(found) = CacheOnDisk::inspect_directory(older.clone()).unwrap()
    else {
        panic!("an unmigrated database has an older schema");
    };
    assert_eq!(std::fs::metadata(&database).unwrap().len(), 0, "migrated");
    let maintenance = found.migrate().unwrap();
    assert_eq!(maintenance.stats().unwrap().total_entries, 0);
    maintenance.close();
    current(&older).close();

    // A newer build's ledger: a migration version past every one this build
    // knows. Written through a migrated cache, then stamped one version on.
    let newer = dir.path().join("newer");
    CachePool::with_directory(
        newer.clone(),
        CacheIdentity::new(RulesVersion::for_testing("newer"), ParserKind::TreeSitter),
    )
    .unwrap()
    .close();
    let database = talkbank_cache::cache_db_path(&newer);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&database);
            let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
            sqlx::query(
                "INSERT INTO _sqlx_migrations \
                 (version, description, success, checksum, execution_time) \
                 SELECT MAX(version) + 1, 'newer', 1, X'00', 0 FROM _sqlx_migrations",
            )
            .execute(&pool)
            .await
            .unwrap();
            pool.close().await;
        });
    assert!(matches!(
        CacheOnDisk::inspect_directory(newer.clone()),
        Err(CacheError::SchemaNewer { .. })
    ));
    assert!(matches!(
        CachePool::with_directory(
            newer,
            CacheIdentity::new(RulesVersion::for_testing("refused"), ParserKind::TreeSitter),
        ),
        Err(CacheError::Migration(_))
    ));
    // Refusal retains no query capability or pending inspection worker.
    std::fs::remove_file(&database).unwrap();
}

/// A failed ledger query also completes inspection shutdown. This is an
/// external resource contract: a mock cannot prove the OS releases the file.
#[test]
fn a_failed_ledger_read_releases_its_database() {
    let dir = tempfile::tempdir().unwrap();
    let database = talkbank_cache::cache_db_path(dir.path());
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let options = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&database)
                .create_if_missing(true);
            let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
            // Deliberately malformed external schema: the ledger reader's
            // query requires the missing `success` column.
            sqlx::query("CREATE TABLE _sqlx_migrations (version INTEGER)")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        });
    assert!(matches!(
        CacheOnDisk::inspect_directory(dir.path().to_path_buf()),
        Err(CacheError::Database { .. })
    ));
    std::fs::remove_file(&database).unwrap();
}

/// Maintenance acts only on the database the look found: one that vanished
/// between the look and the writable open is refused, not recreated.
#[test]
fn a_database_gone_since_the_look_is_not_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let database = talkbank_cache::cache_db_path(dir.path());
    std::fs::write(&database, b"").unwrap();
    let CacheOnDisk::OlderSchema(found) =
        CacheOnDisk::inspect_directory(dir.path().to_path_buf()).unwrap()
    else {
        panic!("an unmigrated database has an older schema");
    };
    std::fs::remove_file(&database).unwrap();
    assert!(matches!(
        found.migrate(),
        Err(CacheError::NoCacheDatabase { .. })
    ));
    assert!(!database.exists(), "maintenance recreated the database");
}
