//! Statistics must describe the storage opened by this handle.

use talkbank_cache::{CacheIdentity, CachePool, MaintenanceCache, ParserKind, RulesVersion};

#[test]
fn explicit_directory_statistics_retain_opened_location() {
    let dir = tempfile::tempdir().unwrap();
    let identity = CacheIdentity::new(
        RulesVersion::for_testing("location"),
        ParserKind::TreeSitter,
    );
    let cache = CachePool::with_directory(dir.path().to_path_buf(), identity).unwrap();
    assert_eq!(
        cache.stats().unwrap().cache_dir.as_deref(),
        Some(dir.path())
    );
    let maintenance = MaintenanceCache::open_directory(dir.path().to_path_buf()).unwrap();
    assert_eq!(
        maintenance.stats().unwrap().cache_dir.as_deref(),
        Some(dir.path())
    );
}

#[test]
fn in_memory_statistics_have_no_filesystem_location() {
    let identity = CacheIdentity::new(
        RulesVersion::for_testing("location"),
        ParserKind::TreeSitter,
    );
    let cache = CachePool::in_memory(identity).unwrap();
    let stats = cache.stats().unwrap();
    assert_eq!(stats.cache_dir, None);
    assert_eq!(stats.total_entries, 0);
}
