//! Shared fallback validation statistics for interactive exits, and the typed
//! outcome every `chatter validate` run reports back to its caller.

use talkbank_transform::validation_runner::{AbortReason, ValidationStatsSnapshot};

/// How one `chatter validate` run ended.
///
/// Returned instead of a bare [`ValidationStatsSnapshot`] so the exit-status
/// decision cannot be taken without confronting the ways a run can end badly.
/// A snapshot alone invites `if invalid > 0 { exit(1) }`, which answers 0 for a
/// run that lost half its files to a crashed worker: the missing files
/// contributed to no counter, so partial totals look immaculate. As a closed
/// enum, the compiler asks about each ending exactly once, in
/// `commands::validate`.
#[derive(Debug)]
pub enum ValidationOutcome {
    /// Every discovered file was accounted for. The only outcome entitled to a
    /// success exit status (subject to the usual invalid/parse-error check).
    Complete {
        /// Totals for the whole input.
        stats: ValidationStatsSnapshot,
    },
    /// The run ended without covering everything it discovered.
    Incomplete {
        /// Totals for the files that WERE processed. Not totals for the input.
        stats: ValidationStatsSnapshot,
        /// Files discovered but never accounted for. Always non-zero.
        lost_files: usize,
    },
    /// The run died before producing any totals.
    Aborted {
        /// What the runner was able to determine about the failure.
        reason: AbortReason,
    },
    /// The event stream closed with no terminal event at all.
    ///
    /// Retained as a backstop rather than deleted: the runner's drop guard
    /// should make this unreachable, and a silent stream is exactly the
    /// condition that guard exists to prevent, so collapsing it into a
    /// success would re-open the hole if the guard ever regressed.
    NoTerminalEvent,
}

/// Whether a run stopped because the user asked it to.
///
/// A named pair rather than a `bool`, because the call sites read
/// `empty_stats(true)` and `empty_stats(false)` and neither says which fact it
/// is asserting. The two arms mean opposite things to a reader of the summary
/// (a cancelled run legitimately covered less than it discovered; an
/// uncancelled one that covered less LOST files), so getting them backwards is
/// a wrong answer, not a cosmetic slip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancellation {
    /// The user asked the run to stop.
    Requested,
    /// The run was left to finish on its own.
    NotRequested,
}

impl Cancellation {
    /// Lower to the snapshot's stored flag.
    fn was_requested(self) -> bool {
        match self {
            Cancellation::Requested => true,
            Cancellation::NotRequested => false,
        }
    }
}

/// Return an empty stats snapshot used when an interactive run exits before completion.
pub fn empty_stats(cancellation: Cancellation) -> ValidationStatsSnapshot {
    ValidationStatsSnapshot {
        total_files: 0,
        valid_files: 0,
        invalid_files: 0,
        cache_hits: 0,
        cache_misses: 0,
        parse_errors: 0,
        roundtrip_passed: 0,
        roundtrip_failed: 0,
        cancelled: cancellation.was_requested(),
    }
}
