// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Every blocking cache operation must work from inside a Tokio runtime.
//!
//! `open_inside_runtime.rs` pins the CONSTRUCTOR, which is where the four-week
//! desktop outage actually bit. This file pins the rest of the surface, because
//! opening is not the only thing the desktop does from inside Tauri's runtime:
//! it reads and writes verdicts during a run, and the cache-maintenance
//! commands clear and purge. Each of those goes through the same `block_on`,
//! and each would have panicked identically had it been reached first.
//!
//! Covering only the operation that happened to be reported is how a bug class
//! survives its own fix. The whole class is one table below.
//!
//! Confinement (`ConfinedRuntime`, which owns the runtime and never lends it
//! out) is what makes this structurally impossible now; these are regression
//! guards over a context no signature can express, not the enforcement.

use std::path::PathBuf;

use talkbank_cache::UnifiedCache;

/// A cache rooted somewhere disposable, never the developer's real one.
fn scratch_cache(name: &str) -> (UnifiedCache, PathBuf) {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let cache = UnifiedCache::with_directory(dir.clone()).expect("open cache");
    (cache, dir)
}

/// Drive `operation` the way Tauri drives a command, and report a panic as one.
///
/// `catch_unwind` rather than letting it propagate, so a failure names the
/// operation that nested a runtime instead of just aborting the test binary.
///
/// `AssertUnwindSafe` because `CachePool` holds a `SqlitePool` and is therefore
/// not `UnwindSafe`. The assertion is honest here: the only panic this is meant
/// to catch is "cannot start a runtime from within a runtime", which happens
/// before any cache state is touched, and the test fails immediately on
/// catching one rather than continuing to use the cache.
fn inside_a_runtime(label: &str, operation: impl FnOnce()) {
    // ONE worker. `Runtime::new()` is the multi-thread default, which is a
    // worker per core (28 on the machine this was written on) spun up and torn
    // down for a single `block_on`. Eight calls in this file made that 224
    // thread spawns for work that never needs more than one.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("outer runtime");
    let outcome = runtime
        .block_on(async { std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) });
    if let Err(panic) = outcome {
        // Report the ORIGINAL message. An earlier version of this helper
        // substituted its own explanation, which would have asserted a cause it
        // had not read: any panic at all would have been reported as a nested
        // runtime.
        let cause = panic
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_owned());
        panic!(
            "`{label}` PANICKED when called from inside a Tokio runtime, which is \
             the desktop app's calling context. Original panic: {cause}"
        );
    }
}

/// The read and write path used during a run.
///
/// Surviving category: behaviour a signature cannot describe. Nothing in
/// `fn set_validation(&self, ...)` says which thread may call it.
#[test]
fn verdict_reads_and_writes_work_from_inside_a_runtime() {
    let (cache, dir) = scratch_cache("chatter-cache-verdicts-in-runtime");
    let probe = dir.join("probe.cha");
    std::fs::write(&probe, "@UTF8\n@Begin\n@End\n").expect("write probe");

    inside_a_runtime("set_validation / get_validation", || {
        cache
            .set_validation(&probe, true, true)
            .expect("set_validation inside a runtime");
        assert_eq!(
            cache.get_validation(&probe, true),
            Some(true),
            "a verdict written from inside a runtime must read back"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

/// The maintenance surface behind `chatter cache clear` and the desktop menu.
///
/// Surviving category: behaviour a signature cannot describe, as above. Kept as
/// one test over a list rather than five near-identical ones, so adding an
/// operation to the cache means adding a line here rather than copying a block.
#[test]
fn every_maintenance_operation_works_from_inside_a_runtime() {
    let (cache, dir) = scratch_cache("chatter-cache-maintenance-in-runtime");

    inside_a_runtime("stats", || {
        cache.stats().expect("stats inside a runtime");
    });
    inside_a_runtime("clear_prefix", || {
        cache
            .clear_prefix("/nothing/matches/this")
            .expect("clear_prefix inside a runtime");
    });
    inside_a_runtime("clear_paths", || {
        cache
            .clear_paths(&[dir.join("absent.cha")])
            .expect("clear_paths inside a runtime");
    });
    inside_a_runtime("purge_nonexistent", || {
        cache
            .purge_nonexistent()
            .expect("purge_nonexistent inside a runtime");
    });
    inside_a_runtime("clear_all", || {
        cache.clear_all().expect("clear_all inside a runtime");
    });

    let _ = std::fs::remove_dir_all(&dir);
}

/// An in-memory cache must open AND drop from inside a runtime.
///
/// This is the test that found the second half of the bug. `Drop for Runtime`
/// blocks and blocking is illegal in an async context, so before
/// `ConfinedRuntime` moved runtime ownership to its own thread, this panicked
/// with "Cannot drop a runtime in a context where blocking is not allowed".
///
/// Surviving category: behaviour a signature cannot describe. Nothing in
/// `fn in_memory() -> Result<Self, CacheError>` says which contexts may drop
/// the value it returns.
#[test]
fn an_in_memory_cache_opens_and_drops_inside_a_runtime() {
    inside_a_runtime("in_memory + drop", || {
        let cache = UnifiedCache::in_memory().expect("in_memory inside a runtime");
        drop(cache);
    });
}

/// The same for a file-backed cache, which is what both apps actually use.
///
/// Kept separate from the in-memory case rather than merged: they are different
/// constructors building different pools, and the first version of this file
/// covered only one of them, which is the mistake the whole file is about.
///
/// Surviving category: behaviour a signature cannot describe.
#[test]
fn a_file_backed_cache_opens_and_drops_inside_a_runtime() {
    let dir = std::env::temp_dir().join("chatter-cache-drop-probe");
    let _ = std::fs::remove_dir_all(&dir);
    inside_a_runtime("with_directory + drop", || {
        let cache = UnifiedCache::with_directory(dir.clone()).expect("open inside a runtime");
        drop(cache);
    });
    let _ = std::fs::remove_dir_all(&dir);
}
