//! The cache must open from inside a Tokio runtime.
//!
//! Tauri runs an `async fn` command ON its async runtime, so the desktop app's
//! `validate` command opens the cache from within a runtime context. The CLI
//! opens it from a plain thread, which is why every existing test passed while
//! the desktop app could not validate anything at all: `UnifiedCache::open`
//! builds its own runtime and calls `block_on`, and nesting runtimes panics.
//!
//! This is a REGRESSION GUARD, not the fix. The fix is for the runtime to be
//! owned by the application boundary rather than manufactured inside the cache,
//! which makes the nesting unrepresentable instead of merely tested for.
use talkbank_cache::UnifiedCache;

#[test]
fn cache_opens_from_within_a_tokio_runtime() {
    let dir = std::env::temp_dir().join("chatter-open-in-runtime-probe");
    let _ = std::fs::remove_dir_all(&dir);

    let outer = tokio::runtime::Runtime::new().expect("outer runtime");
    let outcome =
        outer.block_on(async { std::panic::catch_unwind(|| UnifiedCache::with_directory(dir)) });

    match outcome {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => panic!("cache open failed inside a runtime: {error}"),
        Err(_) => panic!(
            "cache open PANICKED inside a Tokio runtime: this is the desktop app's \
             validate path, which has been unable to start a run since the cache \
             was wired into it"
        ),
    }
}
