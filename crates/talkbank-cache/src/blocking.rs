//! Running the cache's async database work from a synchronous caller, on any
//! thread, without ever nesting Tokio runtimes.
//!
//! # The bug this module exists to make unrepresentable
//!
//! `CachePool` is a synchronous API over an asynchronous database: it owns a
//! Tokio runtime and every operation calls `block_on`. That is a deliberate
//! choice, not an accident. The callers are CPU-bound validation worker threads
//! doing synchronous work, and colouring them `async` to satisfy a cache would
//! make the architecture worse.
//!
//! What the design could not survive was being entered from a thread that is
//! ALREADY driving a runtime: `Runtime::block_on` panics there with "Cannot
//! start a runtime from within a runtime". Nothing in a signature like
//!
//! ```ignore
//! pub fn with_directory(cache_dir: PathBuf) -> Result<Self, CacheError>
//! ```
//!
//! said either that it blocks or that it must own the only runtime on this
//! thread, so an illegal calling context was indistinguishable from a legal one
//! and the compiler had nothing to object to.
//!
//! It cost four weeks. Tauri runs an `async fn` command ON its async runtime,
//! so the desktop app's `validate` command opened the cache from inside a
//! runtime context and panicked every time, on every machine. The panic unwound
//! out of the command, the IPC promise never settled, and the UI waited forever
//! with no error to show. Every test passed throughout, because the CLI opens
//! the cache from a plain thread and no test had ever entered an async context.
//! Introduced 2026-07-07 in `5cea49bd`; shipped in v0.6.0 and v0.7.0; reported
//! repeatedly by a user whose files were fine.
//!
//! # Why confinement rather than a witness type
//!
//! The obvious type-level fix is an unforgeable witness ("proof that this thread
//! is not driving a runtime") threaded through every blocking entry point. That
//! makes the nesting CHECKED, at the cost of touching every call site.
//!
//! This module makes it IMPOSSIBLE instead, which is strictly stronger: there is
//! no longer any code path on which `Runtime::block_on` can run on a thread that
//! already has a runtime, so there is no condition left for a witness to attest
//! and no call site that can get it wrong. A witness would have been a check for
//! a state that cannot arise, which is precisely the sort of check a type is
//! supposed to obsolete.
//!
//! The fast path is unchanged: a caller with no ambient runtime blocks directly,
//! exactly as before, with no extra thread and no channel. The detour is paid
//! only by callers that would otherwise have panicked.

use std::future::Future;

use tokio::runtime::{Handle, Runtime};

/// Drive `future` to completion on `runtime`, blocking the current thread.
///
/// Safe to call from ANY thread, including one driving an async runtime. When
/// an ambient runtime is detected the work is moved to a scoped thread that has
/// none, because `Runtime::block_on` panics if the calling thread is already
/// being used to drive asynchronous tasks.
///
/// `std::thread::scope` rather than `std::thread::spawn` so that `future` may
/// borrow from the caller (every call site here borrows `&self.pool`); a
/// `'static` bound would force a pool clone at every one of them.
pub(crate) fn block_on<T, F>(runtime: &Runtime, future: F) -> T
where
    F: Future<Output = T> + Send,
    T: Send,
{
    if Handle::try_current().is_err() {
        // No ambient runtime: this thread is ours to block. The overwhelmingly
        // common case (CLI, validation worker threads), and identical to what
        // this code did before confinement existed.
        return runtime.block_on(future);
    }

    // This thread is driving a runtime, so blocking it here would nest and
    // panic. A scoped thread inherits no runtime context, so `block_on` there
    // is legal; the calling thread simply waits for the join, which is an
    // ordinary blocking wait rather than a nested runtime.
    std::thread::scope(|scope| {
        match scope.spawn(|| runtime.block_on(future)).join() {
            Ok(value) => value,
            // The worker only panics if `future` itself panicked. Re-raise it on
            // this thread so the failure surfaces where the caller can see it,
            // rather than being converted into a different kind of error that
            // would misattribute a bug in the future to the cache.
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_thread_blocks_directly() {
        let runtime = Runtime::new().expect("build a runtime");
        assert_eq!(block_on(&runtime, async { 41 + 1 }), 42);
    }

    /// The regression that four weeks of green tests missed. Before
    /// confinement this panicked with "Cannot start a runtime from within a
    /// runtime", which is what the desktop app's `validate` command did on
    /// every single run.
    #[test]
    fn a_runtime_thread_does_not_nest() {
        let inner = Runtime::new().expect("build the cache's runtime");
        let outer = Runtime::new().expect("build the caller's runtime");

        let value = outer.block_on(async { block_on(&inner, async { 41 + 1 }) });

        assert_eq!(
            value, 42,
            "blocking from inside a runtime must complete rather than panic"
        );
    }

    /// Borrowing is the reason this uses a scoped thread, so it is pinned:
    /// a `'static` bound would compile but force every call site to clone.
    #[test]
    fn the_future_may_borrow_from_the_caller() {
        let inner = Runtime::new().expect("build the cache's runtime");
        let outer = Runtime::new().expect("build the caller's runtime");
        let borrowed = String::from("owned by the caller");

        let length = outer.block_on(async { block_on(&inner, async { borrowed.len() }) });

        assert_eq!(length, borrowed.len());
    }
}
