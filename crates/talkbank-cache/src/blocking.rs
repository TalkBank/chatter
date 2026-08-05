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
//! no longer any code path on which a caller can block a thread that is already
//! driving a runtime, so there is no condition left for a witness to attest and
//! no call site that can get it wrong. A witness would have been a check for a
//! state that cannot arise, which is precisely the sort of check a type is
//! supposed to obsolete.
//!
//! The fast path is unchanged: a caller with no ambient runtime blocks directly,
//! exactly as before. The scoped-thread detour is paid only by callers that
//! would otherwise have panicked.
//!
//! # The second half, found 2026-08-04
//!
//! Confining entry left the EXIT standing: dropping a runtime blocks too, so
//! every cache panicked when dropped inside one. [`ConfinedRuntime`] settles
//! both halves; its docs carry the argument, and this module does not repeat
//! it.

use std::future::Future;

use tokio::runtime::{Handle, Runtime};

/// A runtime that can be neither entered nor dropped from an async context.
///
/// # Both halves of the bug, and why this owns the runtime elsewhere
///
/// Confining the ENTRY (below) fixed the reported outage and left a second,
/// unreported half standing: `tokio`'s own `Drop for Runtime` blocks, and
/// blocking is illegal in an async context, so dropping a cache from inside a
/// runtime panicked with "Cannot drop a runtime in a context where blocking is
/// not allowed". Measured 2026-08-04: EVERY `UnifiedCache`, from every
/// constructor, panicked that way. It had not been reported only because the
/// CLI and the desktop both happen to hold their caches for the process
/// lifetime.
///
/// It was also, for a while, only a CONVENTION that entry stayed confined:
/// `CachePool` held a bare `Runtime`, so any new method could write
/// `self.rt.block_on(...)` and reintroduce the original panic with nothing but
/// a reviewer in the way.
///
/// So the runtime is owned by a dedicated thread that does nothing else, and
/// this type keeps only a [`Handle`], which is a cheap clone that is safe to
/// drop anywhere. That settles both halves at once:
///
/// - There is no `Runtime` here to call `block_on` on directly, so the unsafe
///   entry cannot be written rather than merely being discouraged.
/// - The runtime is dropped by its owning thread, which drives nothing, so the
///   drop panic has no context in which to occur.
///
/// The shutdown signal is the CHANNEL ITSELF: dropping this type drops the
/// sender, the owner thread's `recv` returns `Err`, and it drops the runtime
/// and exits. Dropping a sender never blocks, so the drop path is legal in an
/// async context, which is the whole point.
#[derive(Debug)]
pub(crate) struct ConfinedRuntime {
    handle: Handle,
    /// Held only for its `Drop`. Closing this channel is what tells the owner
    /// thread to shut the runtime down.
    _shutdown: std::sync::mpsc::Sender<()>,
}

impl ConfinedRuntime {
    /// Move `runtime` onto a thread that owns it for the rest of its life.
    ///
    /// Fails only if the thread cannot be spawned, which is a real resource
    /// failure and is reported rather than swallowed.
    pub(crate) fn new(runtime: Runtime) -> std::io::Result<Self> {
        let handle = runtime.handle().clone();
        let (shutdown, closed) = std::sync::mpsc::channel::<()>();
        std::thread::Builder::new()
            .name("talkbank-cache-runtime".to_owned())
            .spawn(move || {
                // Parks until the owning `ConfinedRuntime` is dropped. The
                // value is never sent; the disconnect IS the message.
                let _ = closed.recv();
                // Dropped HERE, on a thread that drives nothing, which is the
                // context tokio requires and an async caller cannot provide.
                drop(runtime);
            })?;
        Ok(Self {
            handle,
            _shutdown: shutdown,
        })
    }

    /// Drive `future` to completion, safely, from any thread.
    ///
    /// See [`block_on`] for why this is safe on a thread that is already
    /// driving a runtime.
    pub(crate) fn block_on<T, F>(&self, future: F) -> T
    where
        F: Future<Output = T> + Send,
        T: Send,
    {
        block_on(&self.handle, future)
    }
}

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
fn block_on<T, F>(handle: &Handle, future: F) -> T
where
    F: Future<Output = T> + Send,
    T: Send,
{
    if Handle::try_current().is_err() {
        // No ambient runtime: this thread is ours to block. The overwhelmingly
        // common case (CLI, validation worker threads), and identical to what
        // this code did before confinement existed.
        return handle.block_on(future);
    }

    // This thread is driving a runtime, so blocking it here would nest and
    // panic. A scoped thread inherits no runtime context, so `block_on` there
    // is legal; the calling thread simply waits for the join, which is an
    // ordinary blocking wait rather than a nested runtime.
    std::thread::scope(|scope| {
        match scope.spawn(|| handle.block_on(future)).join() {
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
        assert_eq!(block_on(runtime.handle(), async { 41 + 1 }), 42);
    }

    /// The regression that four weeks of green tests missed. Before
    /// confinement this panicked with "Cannot start a runtime from within a
    /// runtime", which is what the desktop app's `validate` command did on
    /// every single run.
    #[test]
    fn a_runtime_thread_does_not_nest() {
        let inner = Runtime::new().expect("build the cache's runtime");
        let outer = Runtime::new().expect("build the caller's runtime");

        let value = outer.block_on(async { block_on(inner.handle(), async { 41 + 1 }) });

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

        let length = outer.block_on(async { block_on(inner.handle(), async { borrowed.len() }) });

        assert_eq!(length, borrowed.len());
    }
}
