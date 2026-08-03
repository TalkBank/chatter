//! Orchestration for directory-scale streaming validation.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::cancel::CancelSignal;
use super::config::ValidationConfig;
use super::helpers::collect_cha_files;
use super::types::{
    AbortReason, RunCoverage, ValidationEvent, ValidationStats, ValidationStatsSnapshot,
};
use super::worker::worker_loop;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use std::path::Path;
use std::sync::Arc;
use std::thread;
use talkbank_cache::ValidationCache;

/// How a runner call ended, from the spawning closure's point of view.
///
/// This is a VALUE rather than a `()` return so that terminality has exactly
/// one owner: the runner states how the stream ended, and the closure matches
/// that statement exhaustively. Previously the runner returned nothing and
/// three separate consumers each invented their own answer for "the stream
/// closed without `Finished`", one of which (the TUI) answered "the run
/// completed" and rendered partial counts as final.
#[must_use]
pub(super) enum RunOutcome {
    /// [`ValidationEvent::Finished`] was sent; the stream is properly
    /// terminated and consumers have the run's real totals.
    Finished,
    /// The receiver was gone before anything could be reported, so there is
    /// nobody to tell. Not a fault: the caller dropped the stream (window
    /// closed, run superseded, `--max-errors` short-circuit), and emitting a
    /// terminal event into a dead channel would accomplish nothing.
    ReceiverGone,
}

/// What became of the worker pool once every thread was joined.
///
/// A typed value rather than the `had_panic` bool it replaces, because that
/// bool reached nothing but a `tracing::error!` line that no GUI user ever
/// sees, and the run then reported a clean `Finished` carrying partial stats.
/// Naming the outcome puts it in the terminal-event decision, where it has to
/// be dealt with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorkerPoolOutcome {
    /// Every worker thread returned normally, so nothing was abandoned by a
    /// worker. Files can still be missing for other reasons (cancellation),
    /// which is why coverage is checked rather than inferred from this.
    AllReturned,
    /// At least one worker unwound, abandoning whatever files it had taken off
    /// the queue. Those files produce no result and no counter.
    SomeUnwound {
        /// How many worker threads unwound.
        unwound_workers: usize,
    },
}

impl WorkerPoolOutcome {
    /// Classify a raw count of unwound workers.
    ///
    /// Private constructor rather than letting callers build the variants, so
    /// `SomeUnwound { unwound_workers: 0 }` (a state that contradicts its own
    /// name) cannot be assembled by the runner.
    pub(super) fn from_unwound_count(unwound_workers: usize) -> Self {
        match unwound_workers {
            0 => Self::AllReturned,
            unwound_workers => Self::SomeUnwound { unwound_workers },
        }
    }
}

/// Decide which terminal event a completed run is entitled to send.
///
/// The verdict comes from COVERAGE (what the snapshot proves was processed),
/// not from whether a worker panicked, because the two can differ in both
/// directions: a worker can unwind after its last file and lose nothing, and
/// files can go missing without any panic. `pool_outcome` supplies the cause
/// for the log, so an operator reading the incompleteness report learns why.
pub(super) fn terminal_event(
    stats: ValidationStatsSnapshot,
    pool_outcome: WorkerPoolOutcome,
) -> ValidationEvent {
    match stats.coverage() {
        RunCoverage::Complete => ValidationEvent::Finished(stats),
        // A cancelled run stopped short because it was told to. Reporting that
        // as incompleteness would make the incompleteness report routine, and
        // a routine warning is an ignored one.
        RunCoverage::Cancelled { unprocessed_files } => {
            tracing::info!(
                unprocessed_files,
                "Validation cancelled before covering every discovered file"
            );
            ValidationEvent::Finished(stats)
        }
        RunCoverage::Lost { lost_files } => {
            match pool_outcome {
                WorkerPoolOutcome::AllReturned => tracing::error!(
                    lost_files,
                    "Validation lost files with no worker panic to explain it"
                ),
                WorkerPoolOutcome::SomeUnwound { unwound_workers } => tracing::error!(
                    lost_files,
                    unwound_workers,
                    "Validation workers panicked and abandoned files"
                ),
            }
            ValidationEvent::FinishedIncomplete { stats, lost_files }
        }
    }
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

/// Guarantees that a validation stream ends with a terminal event even when
/// the thread driving it unwinds.
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
                let _ = event_tx.send(ValidationEvent::Aborted(AbortReason::Panicked));
            }
            GuardState::Disarmed => {}
        }
    }
}

/// Run validation for all discovered files and stream progress/events.
///
/// Returns a tuple of:
/// - `Receiver<ValidationEvent>` - Events stream as validation progresses
/// - `Sender<()>` - Send to this channel to cancel validation
///
/// # Example
/// ```ignore
/// let (events, cancel) = validate_directory_streaming(&dir, &config, cache);
///
/// for event in events {
///     match event {
///         ValidationEvent::Errors(e) => print_errors(&e),
///         ValidationEvent::FileComplete(f) => update_progress(&f),
///         ValidationEvent::Finished(stats) => print_summary(&stats),
///         _ => {}
///     }
/// }
/// ```
pub fn validate_directory_streaming<C>(
    directory: &Path,
    config: &ValidationConfig,
    cache: Option<Arc<C>>,
) -> (Receiver<ValidationEvent>, Sender<()>)
where
    C: ValidationCache + Send + Sync + 'static,
{
    // Use unbounded channel for events to prevent backpressure
    // Errors are cheap to store, and we want workers to never block on sending events
    let (event_tx, event_rx) = unbounded::<ValidationEvent>();
    let (cancel_tx, cancel_rx) = bounded::<()>(1);

    let dir = directory.to_path_buf();
    let cfg = config.clone();

    thread::spawn(move || {
        // Armed before any work, so even a failure during discovery terminates
        // the stream rather than closing it silently.
        let mut guard = TerminalGuard::armed(event_tx.clone());
        // Send discovering event immediately so UI shows something is happening
        let _ = event_tx.send(ValidationEvent::Discovering);
        // Exhaustive, no catch-all: a future outcome must be decided here
        // rather than defaulting into a disarm.
        match run_validation(dir, cfg, cache, event_tx, cancel_rx) {
            RunOutcome::Finished | RunOutcome::ReceiverGone => guard.disarm(),
        }
    });

    (event_rx, cancel_tx)
}

/// Validate a pre-collected list of .cha files using the same streaming
/// pipeline as [`validate_directory_streaming`].
///
/// This exists so the `chatter validate` CLI can route a list of file
/// paths (e.g. `chatter validate a.cha b.cha c.cha`) through the same
/// renderer/progress/TUI surface as a directory walk, instead of
/// reinventing per-file output. The CLI is responsible for resolving
/// arguments to a flat file list (walking any directories first); this
/// entry point trusts the list verbatim, no filtering, no extension
/// check.
///
/// Cancellation, event semantics, and worker behavior are identical
/// to the directory variant.
pub fn validate_files_streaming<C>(
    files: Vec<std::path::PathBuf>,
    config: &ValidationConfig,
    cache: Option<Arc<C>>,
) -> (Receiver<ValidationEvent>, Sender<()>)
where
    C: ValidationCache + Send + Sync + 'static,
{
    let (event_tx, event_rx) = unbounded::<ValidationEvent>();
    let (cancel_tx, cancel_rx) = bounded::<()>(1);

    let cfg = config.clone();

    thread::spawn(move || {
        // Same terminal-event guarantee as the directory entrypoint; see
        // [`TerminalGuard`].
        let mut guard = TerminalGuard::armed(event_tx.clone());
        // Send discovering event immediately so the renderer transitions
        // out of "starting up" the same way it would for a directory walk.
        let _ = event_tx.send(ValidationEvent::Discovering);
        match run_validation_on_files(files, cfg, cache, event_tx, cancel_rx) {
            RunOutcome::Finished | RunOutcome::ReceiverGone => guard.disarm(),
        }
    });

    (event_rx, cancel_tx)
}

/// Internal runner implementation used by the directory streaming
/// entrypoint. Discovers `.cha` files under `directory` and forwards
/// the collected list to [`run_validation_on_files`]. Kept thin so the
/// directory-walk and explicit-file-list paths share all worker /
/// event-stream / stats logic.
///
/// Returns how the stream ended; see [`RunOutcome`].
pub(super) fn run_validation<C>(
    directory: std::path::PathBuf,
    config: ValidationConfig,
    cache: Option<Arc<C>>,
    event_tx: Sender<ValidationEvent>,
    cancel_rx: Receiver<()>,
) -> RunOutcome
where
    C: ValidationCache + Send + Sync + 'static,
{
    let mut files = Vec::new();
    collect_cha_files(
        &directory,
        config.directory == super::config::DirectoryMode::Recursive,
        &mut files,
    );
    files.sort();
    run_validation_on_files(files, config, cache, event_tx, cancel_rx)
}

/// Worker-pool body shared by the directory and explicit-file-list
/// streaming entrypoints. Takes a pre-collected list of files,
/// dispatches to N worker threads, streams the standard
/// `ValidationEvent` sequence (Started → Errors / FileComplete /
/// RoundtripComplete → Finished) on `event_tx`, and respects
/// `cancel_rx`.
///
/// Returns how the stream ended; see [`RunOutcome`].
pub(super) fn run_validation_on_files<C>(
    files: Vec<std::path::PathBuf>,
    config: ValidationConfig,
    cache: Option<Arc<C>>,
    event_tx: Sender<ValidationEvent>,
    cancel_rx: Receiver<()>,
) -> RunOutcome
where
    C: ValidationCache + Send + Sync + 'static,
{
    let total_files = files.len();

    // Send start event
    if event_tx
        .send(ValidationEvent::Started { total_files })
        .is_err()
    {
        return RunOutcome::ReceiverGone; // Receiver dropped
    }

    if total_files == 0 {
        let stats = ValidationStats::new(0);
        event_tx
            .send(ValidationEvent::Finished(stats.snapshot()))
            .ok();
        return RunOutcome::Finished;
    }

    // Set up work queue
    let (work_tx, work_rx) = bounded::<std::path::PathBuf>(total_files);
    let stats = Arc::new(ValidationStats::new(total_files));

    // One shared latch rather than N direct readers of the cancel channel; see
    // `CancelSignal` for the token-stealing bug that made cancellation reach
    // only one worker and left `cancelled` false in the final stats.
    let cancel = Arc::new(CancelSignal::new(cancel_rx));

    // Determine number of workers. Treat `jobs=0` as `1` to preserve progress.
    let num_workers = match config.jobs {
        Some(0) => {
            tracing::warn!("validation jobs=0 requested; using 1 worker instead");
            1
        }
        Some(n) => n,
        None => num_cpus::get(),
    };

    // Spawn worker threads
    let workers: Vec<_> = (0..num_workers)
        .map(|_| {
            let rx = work_rx.clone();
            let tx = event_tx.clone();
            let cancel = Arc::clone(&cancel);
            let cache_ref = cache.clone();
            let cfg = config.clone();
            let stats = stats.clone();

            thread::spawn(move || {
                worker_loop(rx, tx, cancel, cache_ref, cfg, stats);
            })
        })
        .collect();

    // Send all work to the queue
    for file in files {
        // Check for early cancellation, through the shared latch so that
        // observing it here does not hide it from the workers.
        if cancel.is_cancelled() {
            break;
        }

        if work_tx.send(file).is_err() {
            break; // Workers died
        }
    }
    drop(work_tx); // Signal no more work

    // Wait for all workers to complete
    let mut unwound_workers = 0usize;
    for (worker_id, worker) in workers.into_iter().enumerate() {
        match worker.join() {
            Ok(()) => {
                // Worker completed successfully
            }
            Err(panic_payload) => {
                tracing::error!(
                    worker_id = worker_id,
                    "Worker panicked: {:?}",
                    panic_payload
                );
                unwound_workers += 1;
            }
        }
    }
    let pool_outcome = WorkerPoolOutcome::from_unwound_count(unwound_workers);

    // Send final stats. The latch answers truthfully however many other
    // threads already observed the same cancellation.
    if cancel.is_cancelled() {
        stats.mark_cancelled();
    }

    let final_stats = stats.snapshot();

    // Log cache statistics for debugging
    let hit_rate = if final_stats.total_files > 0 {
        (final_stats.cache_hits as f64 / final_stats.total_files as f64) * 100.0
    } else {
        0.0
    };
    tracing::info!(
        cache_hits = final_stats.cache_hits,
        cache_misses = final_stats.cache_misses,
        total_files = final_stats.total_files,
        hit_rate_percent = hit_rate,
        valid_files = final_stats.valid_files,
        invalid_files = final_stats.invalid_files,
        "Validation complete"
    );

    if let Err(e) = event_tx.send(terminal_event(final_stats, pool_outcome)) {
        tracing::warn!(event = ?e.0, "Failed to send terminal event: receiver dropped");
    }

    // `Finished` was the intended terminal event whether or not a departed
    // receiver was still there to hear it. Reporting `ReceiverGone` here would
    // be equally true but less useful: what the guard needs to know is that
    // this run reached its end deliberately, not that a listener left.
    RunOutcome::Finished
}
