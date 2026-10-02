//! Orchestration for directory-scale streaming validation.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::cancel::{CancelReason, CancelSignal, Canceller, ErrorBudget, cancel_channel};
use super::config::ValidationRun;
use super::types::{
    AbortReason, CacheUse, FileCompleteEvent, FileStatus, LossCause, RunCoverage, RunEnding,
    ValidationEvent, ValidationStatsSnapshot, ValidationTally, WorkerFaults,
};
use super::worker::{WorkerContext, worker_loop};
use crate::worker_pool::fan_out;
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::num::NonZeroUsize;
use std::path::Path;
use std::thread;

/// How a runner call ended, from the spawning closure's point of view.
///
/// A VALUE rather than a `()` return so that terminality has exactly one
/// owner: the runner states how the stream ended, and the closure matches
/// that statement exhaustively to disarm the [`TerminalGuard`].
#[must_use]
pub(super) enum RunOutcome {
    /// An ending was sent; the stream is properly terminated and consumers
    /// have the run's real totals.
    Finished,
    /// The receiver was gone before anything could be reported, so there is
    /// nobody to tell. Not a fault: the caller dropped the stream (window
    /// closed, run superseded), and sending an ending into a dead channel
    /// would accomplish nothing.
    ReceiverGone,
}

/// Decide how a run that reached its end ended.
///
/// Three facts decide it, each from its owner: COVERAGE (what the snapshot
/// proves was processed), the latched STOP reason (whether the run was told
/// to stop), and the WORKER FAULTS (unwound workers, refused threads,
/// workers that could not create their parser). They differ in every
/// direction: a run told to stop may already have covered every file (then
/// nothing was stopped, and it is `Complete`); files can go missing with no
/// stop requested; and a worker that fails during a stop loses files the
/// stop did not explain, so a fault outranks a stop.
pub(super) fn run_ending(
    stats: ValidationStatsSnapshot,
    stop: Option<CancelReason>,
    faults: Option<WorkerFaults>,
) -> RunEnding {
    let stats = match stats.coverage() {
        RunCoverage::Complete(stats) => return RunEnding::Complete(stats),
        RunCoverage::Shortfall(stats) => stats,
    };
    let missing = stats.missing_files();
    let cause = match (faults, stop) {
        (None, Some(reason)) => {
            tracing::info!(
                unprocessed_files = missing.get(),
                ?reason,
                "Validation stopped before covering every discovered file"
            );
            return RunEnding::Stopped { stats, reason };
        }
        (Some(faults), _) => LossCause::WorkerFaults(faults),
        (None, None) => LossCause::Unexplained,
    };
    tracing::error!(lost_files = missing.get(), %cause, "Validation lost files");
    RunEnding::Incomplete { stats, cause }
}

/// Whether [`TerminalGuard`] will still report an abort when dropped.
///
/// Two named states rather than an `Option<Sender>`, because "disarmed" and
/// "has no sender" are different facts and only one of them is reachable here.
enum GuardState {
    /// The runner has not returned normally yet. Dropping in this state means
    /// the thread unwound, so the abort must be reported on this sender.
    Armed(Sender<ValidationEvent>),
    /// The runner returned and took responsibility for the terminal event.
    /// Dropping in this state must add nothing to the stream.
    Disarmed,
}

/// Guarantees that a validation stream ends with an ending even when the
/// thread driving it unwinds.
///
/// # What firing this guard proves
///
/// It fires ONLY on an unwind. The spawning closure disarms it immediately
/// after the runner returns, matching [`RunOutcome`] exhaustively, so every
/// normal exit (including "the receiver went away") is a disarm. The single
/// remaining path to `drop` while armed is a panic propagating out of the
/// runner, which is why [`AbortReason::Panicked`] is an honest report rather
/// than a guess. Do not add reasons here that this reasoning cannot support.
///
/// The guard holds its own CLONE of the event sender, so the channel stays
/// open through the unwind: the runner's own sender is dropped first, and the
/// guard's send still reaches a live receiver.
pub(super) struct TerminalGuard {
    state: GuardState,
}

impl TerminalGuard {
    /// Create an ARMED guard. There is deliberately no disarmed constructor:
    /// a guard that starts disarmed guarantees nothing, so it should not be
    /// constructible.
    pub(super) fn armed(event_tx: Sender<ValidationEvent>) -> Self {
        Self {
            state: GuardState::Armed(event_tx),
        }
    }

    /// Hand responsibility for the terminal event back to the runner.
    pub(super) fn disarm(&mut self) {
        self.state = GuardState::Disarmed;
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        match std::mem::replace(&mut self.state, GuardState::Disarmed) {
            GuardState::Armed(event_tx) => {
                // A send failure here means the receiver is already gone, so
                // there is nobody to inform and nothing to propagate to: this
                // is running inside `drop`, frequently during an unwind. This
                // is the one place where discarding the result is the correct
                // behavior rather than a silent swallow.
                let _ = event_tx.send(ValidationEvent::Finished(RunEnding::Aborted(
                    AbortReason::Panicked,
                )));
            }
            GuardState::Disarmed => {}
        }
    }
}

/// Run validation for all discovered files and stream progress/events.
///
/// Returns the event stream and the run's [`Canceller`].
///
/// # Example
/// Keep the canceller alive while consuming events. Only
/// [`RunEnding::passed`] may be reported as a successful validation of the
/// whole input; a stream that closes with no ending is an abort.
///
/// ```no_run
/// use std::path::Path;
/// use talkbank_transform::validation_runner::{
///     validate_directory_streaming, AbortReason, RunEnding, ValidationConfig, ValidationEvent,
///     ValidationRun,
/// };
/// # fn observe(dir: &Path, config: ValidationConfig) -> RunEnding {
/// let run = ValidationRun::uncached(config);
/// let (events, _canceller) = validate_directory_streaming(dir, &run);
///
/// for event in events {
///     match event {
///         ValidationEvent::Discovering
///         | ValidationEvent::Started { .. }
///         | ValidationEvent::FileComplete(_) => println!("{event:?}"),
///         ValidationEvent::Finished(ending) => {
///             println!("passed: {}", ending.passed());
///             return ending;
///         }
///     }
/// }
/// RunEnding::Aborted(AbortReason::NoEnding)
/// # }
/// ```
pub fn validate_directory_streaming(
    directory: &Path,
    run: &ValidationRun,
) -> (Receiver<ValidationEvent>, Canceller) {
    // Use unbounded channel for events to prevent backpressure
    // Errors are cheap to store, and we want workers to never block on sending events
    let (event_tx, event_rx) = unbounded::<ValidationEvent>();
    let (canceller, cancel_rx) = cancel_channel();

    let dir = directory.to_path_buf();
    let run = run.clone();

    thread::spawn(move || {
        // Armed before any work, so even a failure during discovery terminates
        // the stream rather than closing it silently.
        let mut guard = TerminalGuard::armed(event_tx.clone());
        // Send discovering event immediately so UI shows something is happening
        let _ = event_tx.send(ValidationEvent::Discovering);
        // Exhaustive, no catch-all: a future outcome must be decided here
        // rather than defaulting into a disarm.
        match run_validation(dir, run, event_tx, cancel_rx) {
            RunOutcome::Finished | RunOutcome::ReceiverGone => guard.disarm(),
        }
    });

    (event_rx, canceller)
}

/// Validate a pre-collected list of .cha files using the same streaming
/// pipeline as [`validate_directory_streaming`].
///
/// For a caller holding plain file paths (the desktop app, tests). No
/// filtering and no extension check: each path is resolved to its stored
/// name once, before any worker starts, and one that cannot be resolved is
/// a [`FileStatus::ReadError`] in the run's results. Command-line arguments
/// go through [`validate_arguments_streaming`] instead.
///
/// Cancellation, event semantics, and worker behavior are identical
/// to the directory variant.
pub fn validate_files_streaming(
    files: Vec<std::path::PathBuf>,
    run: &ValidationRun,
) -> (Receiver<ValidationEvent>, Canceller) {
    let (event_tx, event_rx) = unbounded::<ValidationEvent>();
    let (canceller, cancel_rx) = cancel_channel();

    let run = run.clone();

    thread::spawn(move || {
        // Same terminal-event guarantee as the directory entrypoint; see
        // [`TerminalGuard`].
        let mut guard = TerminalGuard::armed(event_tx.clone());
        // Send discovering event immediately so the renderer transitions
        // out of "starting up" the same way it would for a directory walk.
        let _ = event_tx.send(ValidationEvent::Discovering);
        // Each path is resolved to its stored name here, once, with one
        // resolver; one that cannot be is a read error like any other.
        let (files, unreadable) = crate::paths::resolve_transcripts(files);
        match run_validation_with_unreadable(files, unreadable, run, event_tx, cancel_rx) {
            RunOutcome::Finished | RunOutcome::ReceiverGone => guard.disarm(),
        }
    });

    (event_rx, canceller)
}

/// Validate command-line arguments as expanded by
/// [`crate::paths::expand_transcript_arguments`]: every file it found, and
/// every argument or directory entry it could not read, reported as a
/// [`FileStatus::ReadError`] counted in the run's totals.
///
/// The one policy for unreadable input, shared with the directory entry
/// point: a run over partly unreadable input is a run with read errors
/// (it fails, and says which paths), never a refusal printed outside the
/// event stream, where a JSON consumer could not see it.
pub fn validate_arguments_streaming(
    input: crate::paths::ExpandedArguments,
    run: &ValidationRun,
) -> (Receiver<ValidationEvent>, Canceller) {
    let (event_tx, event_rx) = unbounded::<ValidationEvent>();
    let (canceller, cancel_rx) = cancel_channel();
    let run = run.clone();
    thread::spawn(move || {
        // Same terminal-event guarantee as the other entry points; see
        // [`TerminalGuard`].
        let mut guard = TerminalGuard::armed(event_tx.clone());
        let _ = event_tx.send(ValidationEvent::Discovering);
        let (files, unreadable) = input.into_parts();
        match run_validation_with_unreadable(files, unreadable, run, event_tx, cancel_rx) {
            RunOutcome::Finished | RunOutcome::ReceiverGone => guard.disarm(),
        }
    });
    (event_rx, canceller)
}

/// Internal runner implementation used by the directory streaming
/// entrypoint: walks `directory` (every level, following links) and hands
/// the stored transcripts it found, and what it could not read, to the
/// shared run body.
///
/// Returns how the stream ended; see [`RunOutcome`].
pub(super) fn run_validation(
    directory: std::path::PathBuf,
    run: ValidationRun,
    event_tx: Sender<ValidationEvent>,
    cancel_rx: Receiver<()>,
) -> RunOutcome {
    let (found, failures) = crate::paths::walk_transcripts(&directory).into_parts();
    let files = crate::paths::DistinctTranscripts::new(
        found
            .into_iter()
            .map(crate::paths::FoundTranscript::into_stored),
    );
    run_validation_with_unreadable(files, failures, run, event_tx, cancel_rx)
}

/// The run body, shared by every entry point. `files` are already stored
/// transcripts, each location once, so no worker resolves a name and no file
/// is validated or counted twice; `unreadable` holds what could
/// not be read or named, each reported as a file that could not be read
/// ([`FileStatus::ReadError`]) and counted in the run's totals, so a run
/// with an unreadable directory can never look like a clean run over a
/// smaller corpus.
fn run_validation_with_unreadable(
    files: crate::paths::DistinctTranscripts,
    unreadable: Vec<crate::paths::WalkFailure>,
    run: ValidationRun,
    event_tx: Sender<ValidationEvent>,
    cancel_rx: Receiver<()>,
) -> RunOutcome {
    let total_files = files.len() + unreadable.len();

    // Send start event
    if event_tx
        .send(ValidationEvent::Started { total_files })
        .is_err()
    {
        return RunOutcome::ReceiverGone; // Receiver dropped
    }

    // The one place a run with nothing to validate is recognised.
    let Some(total_files) = NonZeroUsize::new(total_files) else {
        event_tx
            .send(ValidationEvent::Finished(RunEnding::NothingFound))
            .ok();
        return RunOutcome::Finished;
    };

    // What the walk could not read, counted before any worker starts.
    let mut unreadable_tally = ValidationTally::default();
    for failure in unreadable {
        let status = FileStatus::ReadError {
            message: failure.error.to_string(),
        };
        unreadable_tally.record(&status, CacheUse::NotConsulted);
        if event_tx
            .send(ValidationEvent::FileComplete(FileCompleteEvent {
                path: failure.path,
                status,
                cache: CacheUse::NotConsulted,
            }))
            .is_err()
        {
            return RunOutcome::ReceiverGone;
        }
    }

    // One shared latch rather than N direct readers of the cancel channel; see
    // `CancelSignal` for the token-stealing bug that made cancellation reach
    // only one worker and lost the stop from the final decision.
    let cancel = CancelSignal::new(cancel_rx);
    let config = run.config();
    let budget = ErrorBudget::new(config.error_limit);
    let context = WorkerContext {
        event_tx: &event_tx,
        cancel: &cancel,
        budget: &budget,
        cache: run.cache(),
        config,
    };

    // The shared pool. The feeder stops at the first item after a
    // cancellation, through the shared latch so that observing it here does
    // not hide it from the workers, which check it themselves before each
    // file. Each worker returns its own tally, summed after the join.
    let pool = fan_out(
        files.into_iter().take_while(|_| cancel.reason().is_none()),
        config.jobs,
        |work| worker_loop(work, &context),
    );
    let tally = pool
        .results
        .iter()
        .filter_map(|result| result.as_ref().ok())
        .copied()
        .fold(unreadable_tally, ValidationTally::add);
    let faults = WorkerFaults::observe(
        &pool.outcome,
        pool.results
            .iter()
            .filter_map(|result| result.as_ref().err()),
    );

    let final_stats = tally.snapshot(total_files);
    tracing::info!(
        cache_hits = final_stats.cache_hits(),
        cache_misses = final_stats.cache_misses(),
        total_files = final_stats.total_files().get(),
        hit_rate_percent = ?final_stats.cache_hit_rate(),
        valid_files = final_stats.valid_files(),
        invalid_files = final_stats.invalid_files(),
        "Validation complete"
    );

    // The latch answers truthfully however many other threads already
    // observed the same stop.
    let ending = run_ending(final_stats, cancel.reason(), faults);
    if let Err(e) = event_tx.send(ValidationEvent::Finished(ending)) {
        tracing::warn!(event = ?e.0, "Failed to send the run's ending: receiver dropped");
    }

    // The ending was intended whether or not a departed receiver was still
    // there to hear it. Reporting `ReceiverGone` here would be equally true
    // but less useful: what the guard needs to know is that this run reached
    // its end deliberately, not that a listener left.
    RunOutcome::Finished
}
