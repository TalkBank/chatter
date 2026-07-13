//! Concurrency safety of opening a file-backed cache.
//!
//! Two openers racing a FRESH cache database both try to apply migration
//! version 1 to an empty `_sqlx_migrations`, and without serialization the
//! loser dies with `UNIQUE constraint failed: _sqlx_migrations.version`, which
//! breaks cache init. This exercises many simultaneous opens over one fresh
//! directory and requires every one to succeed. It reproduces the race against
//! the pre-fix code and guards the fix.

use std::sync::{Arc, Barrier};
use std::thread;

use talkbank_cache::{CacheOutcome, CachePool, ValidationCache};

#[test]
fn concurrent_opens_on_fresh_cache_dir_all_succeed() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let cache_dir = dir.path().join("cache");

    // The cache keys on a content fingerprint of the file, so the probe must
    // exist on disk for `set`/`get` to hash it.
    let probe = Arc::new(dir.path().join("probe.cha"));
    std::fs::write(probe.as_path(), "@UTF8\n@Begin\n@End\n").expect("write probe");

    // Enough openers, released together by a barrier, to make the migration
    // race reliable.
    const OPENERS: usize = 12;
    let barrier = Arc::new(Barrier::new(OPENERS));

    let handles: Vec<_> = (0..OPENERS)
        .map(|i| {
            let cache_dir = cache_dir.clone();
            let barrier = Arc::clone(&barrier);
            let probe = Arc::clone(&probe);
            thread::spawn(move || -> Result<(), String> {
                // Line every opener up so they hit migration at the same instant.
                barrier.wait();
                let cache =
                    CachePool::with_directory(cache_dir).map_err(|e| format!("open: {e}"))?;
                // Also exercise a write + read so the pool is actually usable,
                // not merely constructed.
                cache
                    .set(probe.as_path(), false, CacheOutcome::Valid)
                    .map_err(|e| format!("set (opener {i}): {e}"))?;
                match cache.get(probe.as_path(), false) {
                    Some(CacheOutcome::Valid) => Ok(()),
                    other => Err(format!("get (opener {i}) returned {other:?}")),
                }
            })
        })
        .collect();

    let mut failures = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(message)) => failures.push(message),
            Err(_) => failures.push("opener thread panicked".to_string()),
        }
    }

    assert!(
        failures.is_empty(),
        "concurrent opens on a fresh cache dir must all succeed, but {} failed: {:#?}",
        failures.len(),
        failures
    );
}
