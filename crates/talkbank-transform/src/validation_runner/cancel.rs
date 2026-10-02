//! Why and whether a validation run was told to stop early.
//!
//! Three pieces, each owning one fact:
//!
//! - [`Canceller`], the caller's handle: the only way to ask a run to stop
//!   from outside it.
//! - [`ErrorLimit`] and the runner's private `ErrorBudget`: the run stops
//!   ITSELF when the errors its workers found reach a limit. The runner
//!   counts, from each file's own status, so warnings never count and the
//!   stop is decided where the files are, not by a consumer reading events
//!   after the fact.
//! - [`CancelSignal`], the run's latch: the first reason to stop is recorded
//!   once, and every worker, the feeder and the end-of-run decision read the
//!   same answer.
//!
//! The run's ending is decided from the latched reason AND the run's
//! coverage: a run told to stop that nevertheless covered every file is
//! `Complete`, never "stopped", so no consumer reports a stop that did not
//! happen.

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use std::num::NonZeroUsize;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Why a run stopped before it ran out of files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    /// The errors found reached the run's [`ErrorLimit::StopAfter`].
    ErrorLimit {
        /// The limit that was reached.
        limit: NonZeroUsize,
    },
    /// The caller asked the run to stop, through its [`Canceller`].
    Requested,
}

impl std::fmt::Display for CancelReason {
    /// The stop as a phrase every surface shows verbatim (the CLI, the TUI,
    /// the desktop app), so one stop is never described two ways.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ErrorLimit { limit } => {
                write!(f, "Stopped after reaching the error limit ({limit})")
            }
            Self::Requested => f.write_str("Validation cancelled"),
        }
    }
}

/// How many errors a run may find before it stops itself.
///
/// Errors only: a diagnostic of `Severity::Error` that the run shows (after
/// suppression), or a failed roundtrip. Warnings never count, so a
/// warnings-only corpus can never be stopped by a limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorLimit {
    /// Validate every file (the default).
    #[default]
    Unlimited,
    /// Stop once this many errors have been found. Files already being
    /// validated finish; no new file starts.
    StopAfter(NonZeroUsize),
}

/// The caller's handle for stopping a run it started.
///
/// A newtype over the channel rather than a bare `Sender<()>`, so the one
/// thing a holder can do is ask for a stop, and the runner alone decides
/// what that stop was ([`CancelReason::Requested`]).
#[derive(Debug, Clone)]
pub struct Canceller {
    tx: Sender<()>,
}

impl Canceller {
    /// Ask the run to stop. Idempotent, and a no-op once the run has ended:
    /// a finished run has nothing left to stop, so a closed channel is not
    /// an error the caller could act on.
    pub fn cancel(&self) {
        match self.tx.try_send(()) {
            // Sent, or a request is already pending (the channel holds one),
            // or the run has ended. Every case leaves the run stopped or
            // stopping, which is all the caller asked for.
            Ok(())
            | Err(crossbeam_channel::TrySendError::Full(()))
            | Err(crossbeam_channel::TrySendError::Disconnected(())) => {}
        }
    }
}

/// A canceller and the receiving end the runner wraps in a [`CancelSignal`].
pub(super) fn cancel_channel() -> (Canceller, Receiver<()>) {
    let (tx, rx) = crossbeam_channel::bounded(1);
    (Canceller { tx }, rx)
}

/// The run's stop latch, shared by every thread that must honour it.
///
/// # Why a latch and not the channel
///
/// `try_recv` REMOVES the single cancel token. When the feeder, every
/// worker and the end-of-run check polled the channel directly, exactly ONE
/// of them consumed the token: a cancel stopped one worker out of N, and the
/// end-of-run decision usually lost the race and recorded no cancellation.
/// Latching turns the message into a fact: the first observer records it,
/// and every later reader sees the same reason.
pub(super) struct CancelSignal {
    /// The caller's cancel channel.
    cancel_rx: Receiver<()>,
    /// The first reason to stop, set once and never cleared.
    latched: OnceLock<CancelReason>,
}

impl CancelSignal {
    /// Wrap the caller's cancel channel.
    pub(super) fn new(cancel_rx: Receiver<()>) -> Self {
        Self {
            cancel_rx,
            latched: OnceLock::new(),
        }
    }

    /// Why the run must stop, if it must. Once `Some`, always the same
    /// `Some`.
    pub(super) fn reason(&self) -> Option<CancelReason> {
        if let Some(reason) = self.latched.get() {
            return Some(*reason);
        }
        match self.cancel_rx.try_recv() {
            Ok(()) => Some(self.latch(CancelReason::Requested)),
            // A dropped canceller is NOT a request: the caller simply stopped
            // holding its handle, which happens routinely.
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    /// Record `reason` unless another reason got there first; returns the
    /// reason that holds.
    fn latch(&self, reason: CancelReason) -> CancelReason {
        *self.latched.get_or_init(|| reason)
    }
}

/// The run's error count against its [`ErrorLimit`], spent by the workers.
///
/// The one counter workers share. It only ever grows, and the single
/// transition that matters (reaching the limit) is detected by the worker
/// whose addition crossed it, which latches the stop.
pub(super) struct ErrorBudget {
    limit: ErrorLimit,
    spent: AtomicUsize,
}

impl ErrorBudget {
    /// A budget for `limit`, nothing spent.
    pub(super) fn new(limit: ErrorLimit) -> Self {
        Self {
            limit,
            spent: AtomicUsize::new(0),
        }
    }

    /// Count `errors` more, latching [`CancelReason::ErrorLimit`] on the
    /// addition that reaches the limit.
    pub(super) fn spend(&self, errors: usize, cancel: &CancelSignal) {
        match self.limit {
            ErrorLimit::Unlimited => {}
            ErrorLimit::StopAfter(limit) => {
                if errors == 0 {
                    return;
                }
                let before = self.spent.fetch_add(errors, Ordering::Relaxed);
                if before < limit.get() && before.saturating_add(errors) >= limit.get() {
                    cancel.latch(CancelReason::ErrorLimit { limit });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The addition that reaches the limit latches it, once; a later caller
    /// cancel does not overwrite the reason; an unlimited budget never stops.
    #[test]
    fn the_budget_latches_its_limit_once() {
        let (canceller, rx) = cancel_channel();
        let signal = CancelSignal::new(rx);
        let limit = NonZeroUsize::new(3).expect("non-zero");
        let budget = ErrorBudget::new(ErrorLimit::StopAfter(limit));
        budget.spend(2, &signal);
        assert_eq!(signal.reason(), None);
        budget.spend(1, &signal);
        assert_eq!(signal.reason(), Some(CancelReason::ErrorLimit { limit }));
        canceller.cancel();
        assert_eq!(signal.reason(), Some(CancelReason::ErrorLimit { limit }));

        let (_canceller, rx) = cancel_channel();
        let unlimited = CancelSignal::new(rx);
        ErrorBudget::new(ErrorLimit::Unlimited).spend(1_000, &unlimited);
        assert_eq!(unlimited.reason(), None);
    }

    /// A caller's cancel is latched as `Requested` and read the same by
    /// every later reader; a dropped canceller is no request.
    #[test]
    fn a_requested_cancel_is_latched() {
        let (canceller, rx) = cancel_channel();
        let signal = CancelSignal::new(rx);
        canceller.cancel();
        canceller.cancel();
        assert_eq!(signal.reason(), Some(CancelReason::Requested));
        assert_eq!(signal.reason(), Some(CancelReason::Requested));

        let (canceller, rx) = cancel_channel();
        drop(canceller);
        assert_eq!(CancelSignal::new(rx).reason(), None);
    }
}
