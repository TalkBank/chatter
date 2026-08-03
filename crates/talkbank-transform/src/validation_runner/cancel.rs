//! Latched cancellation for one validation run.

use crossbeam_channel::{Receiver, TryRecvError};
use std::sync::atomic::{AtomicBool, Ordering};

/// The run's cancellation state, shared by every thread that must honour it.
///
/// # Why a latch and not the channel
///
/// Cancellation arrives as a single `()` on a `bounded(1)` channel, and
/// `try_recv` REMOVES it. The dispatch loop, all N worker threads, and the
/// end-of-run check each polled that channel directly, so exactly ONE of them
/// consumed the token and every other reader concluded the run had not been
/// cancelled. Two consequences, both real:
///
/// - a cancel request stopped one worker out of N, and the rest kept
///   validating until the queue drained;
/// - the final statistics recorded `cancelled: false` for runs the user had
///   explicitly cancelled, because the end-of-run check usually lost the race
///   to a worker.
///
/// The second one is what makes this load-bearing rather than cosmetic:
/// coverage reconciliation reads `cancelled` to tell a requested shortfall
/// apart from files lost to a crash, so a mis-recorded flag turns every
/// cancelled run into a false "the validator lost N files" alarm.
///
/// Latching turns the message into a FACT: the first observer records it, and
/// every later observer reads the same answer, including one that polls long
/// after the channel was drained.
pub(super) struct CancelSignal {
    /// The caller's cancel channel. Polled at most once more after the latch
    /// closes, and never again after that.
    cancel_rx: Receiver<()>,
    /// Set once cancellation has been observed. Monotonic: it never clears,
    /// which is what makes repeated reads agree.
    observed: AtomicBool,
}

impl CancelSignal {
    /// Wrap the caller's cancel channel.
    pub(super) fn new(cancel_rx: Receiver<()>) -> Self {
        Self {
            cancel_rx,
            observed: AtomicBool::new(false),
        }
    }

    /// Whether cancellation has been requested. Once true, always true.
    ///
    /// `Relaxed` is sufficient: the flag publishes no other memory, and every
    /// reader only needs to learn of the cancellation eventually rather than
    /// synchronized with any particular write.
    pub(super) fn is_cancelled(&self) -> bool {
        if self.observed.load(Ordering::Relaxed) {
            return true;
        }
        match self.cancel_rx.try_recv() {
            Ok(()) => {
                self.observed.store(true, Ordering::Relaxed);
                true
            }
            // A dropped sender is NOT a cancellation request: the caller simply
            // stopped holding its end of the handle, which happens routinely
            // when a caller keeps the event receiver but discards the canceller.
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => false,
        }
    }
}
