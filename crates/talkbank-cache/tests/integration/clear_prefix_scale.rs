// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! `clear_prefix` at corpus scale: one statement, not one per file.
//!
//! Regression pin for the v0.5.0 `--force` "hang" (2026-07-30): the
//! original implementation fetched EVERY `file_path` in the cache into
//! memory and then issued an individual autocommitted `DELETE` per
//! matching file. On the operator's real cache (136k distinct paths)
//! that turned `chatter validate --force <corpus-root>` into minutes of
//! silent CPU before validation began. The corpus differential could
//! never catch this class because it always runs on fresh isolated
//! caches; this test exercises the big-warm-cache path directly.

use std::path::Path;
use std::time::{Duration, Instant};

use talkbank_cache::CachePool;
use tempfile::tempdir;

/// Seed `count` cached validation entries for real (tiny) files under `dir`.
fn seed_entries(cache: &CachePool, dir: &Path, count: usize) {
    for i in 0..count {
        let file = dir.join(format!("f{i:05}.cha"));
        std::fs::write(&file, b"@UTF8\n").unwrap();
        cache.set_validation(&file, true, true).unwrap();
    }
}

/// Clearing a corpus-sized prefix must be a single bulk statement:
/// thousands of entries clear in well under a second. The pre-fix
/// implementation (per-file DELETE loop) takes multiple seconds at this
/// size and minutes at real corpus size, so the bound separates the two
/// implementations with a wide margin on any development machine.
#[test]
fn clear_prefix_is_bulk_not_per_file() {
    let cache_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let cache = CachePool::with_directory(cache_dir.path().to_path_buf()).unwrap();

    seed_entries(&cache, data_dir.path(), 4000);

    let started = Instant::now();
    let removed = cache
        .clear_prefix(&data_dir.path().to_string_lossy())
        .unwrap();
    let elapsed = started.elapsed();

    assert_eq!(removed, 4000, "every entry under the prefix is cleared");
    assert!(
        elapsed < Duration::from_secs(2),
        "clear_prefix must be a bulk operation; {elapsed:?} for 4000 entries \
         means the per-file delete loop is back"
    );
}

/// `clear_prefix` semantics are PATH-COMPONENT prefix semantics
/// (`Path::starts_with`), not string prefix: `/a/b` covers `/a/b/c.cha`
/// and `/a/b` itself, but never `/a/bc/…`. Pins the boundary so the SQL
/// rewrite cannot broaden it.
#[test]
fn clear_prefix_respects_path_component_boundaries() {
    let cache_dir = tempdir().unwrap();
    let root = tempdir().unwrap();
    let inside = root.path().join("data");
    // A sibling whose name extends the prefix STRING but not the path.
    let sibling = root.path().join("datax");
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::create_dir_all(&sibling).unwrap();

    let cache = CachePool::with_directory(cache_dir.path().to_path_buf()).unwrap();
    let in_file = inside.join("kept.cha");
    let out_file = sibling.join("outside.cha");
    std::fs::write(&in_file, b"@UTF8\n").unwrap();
    std::fs::write(&out_file, b"@UTF8\n").unwrap();
    cache.set_validation(&in_file, true, true).unwrap();
    cache.set_validation(&out_file, true, true).unwrap();

    let removed = cache.clear_prefix(&inside.to_string_lossy()).unwrap();

    assert_eq!(removed, 1, "only the entry under the prefix directory goes");
    assert_eq!(
        cache.get_validation(&out_file, true),
        Some(true),
        "an entry in a sibling directory sharing the string prefix survives"
    );
    assert_eq!(cache.get_validation(&in_file, true), None);
}
