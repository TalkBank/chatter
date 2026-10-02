//! Regression tests for validation-runner orchestration behavior.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::runner::{TerminalGuard, run_ending};
use super::types::{CacheUse, RoundtripVerdict, ValidationTally};
use super::{
    AbortReason, CacheLookup, CacheOutcome, ContentHash, FileStatus, RoundtripCheck, RunCache,
    RunEnding, ValidationCache, ValidationConfig, ValidationEvent, ValidationRun,
    ValidationStatsSnapshot, validate_directory_streaming,
};
use crate::worker_pool::PoolOutcome;
use std::fs;
use std::num::NonZeroUsize;
use std::time::Duration;
use talkbank_cache::{CacheError, RoundtripOutcome};
use talkbank_model::validation::AlignmentValidation;
use tempfile::tempdir;

#[test]
fn internal_failure_is_neither_valid_nor_invalid_and_never_cached() {
    use super::cancel::CancelSignal;
    use super::worker::WorkerContext;
    use super::worker::{ParserDispatch, worker_loop_with_parser};
    let dir = tempdir().unwrap();
    let path = dir.path().join("producer-fault.cha");
    fs::write(
        &path,
        include_str!("../../../../corpus/reference/languages/eng-conversation.cha"),
    )
    .unwrap();
    let (event_tx, event_rx) = crossbeam_channel::unbounded();
    let (_canceller, cancel_rx) = crossbeam_channel::unbounded();
    let cache = std::sync::Arc::new(RecordingCache::new());
    let run_cache = RunCache::ReadWrite(cache.clone());
    let cancel = CancelSignal::new(cancel_rx);
    let budget = super::cancel::ErrorBudget::new(super::ErrorLimit::Unlimited);
    let config = ValidationConfig {
        roundtrip: RoundtripCheck::Run,
        presentation: crate::PresentationPolicy::new()
            .disable(talkbank_model::ErrorCode::InternalError),
        ..ValidationConfig::default()
    };
    let tally = worker_loop_with_parser(
        std::iter::once(crate::paths::StoredTranscript::resolve(&path).expect("stored name")),
        &WorkerContext {
            event_tx: &event_tx,
            cancel: &cancel,
            budget: &budget,
            cache: &run_cache,
            config: &config,
        },
        ParserDispatch::InternalFailure,
    );
    let events: Vec<_> = event_rx.try_iter().collect();
    assert!(
        matches!(&events[..], [ValidationEvent::FileComplete(complete)]
        if matches!(complete.status, FileStatus::InternalFailure {
                attempt: super::FailedAttempt::Validation { .. }, ..
            })
            && complete.status.shown().is_some_and(|shown|
                shown.errors[0].code == talkbank_model::ErrorCode::InternalError))
    );
    assert!(cache.outcomes().is_empty());
    let stats = tally.snapshot(NonZeroUsize::MIN);
    assert_eq!(stats.internal_failures(), 1);
    assert_eq!(stats.valid_files() + stats.invalid_files(), 0);
    assert_eq!(stats.roundtrip_passed() + stats.roundtrip_failed(), 0);
    assert_eq!(stats.files_accounted_for(), 1);
}

/// Cache that records what was stored, for verifying cache semantics.
struct RecordingCache {
    identity: talkbank_cache::CacheIdentity,
    stored: std::sync::Mutex<Vec<(std::path::PathBuf, CacheOutcome)>>,
    roundtrips: std::sync::Mutex<Vec<RoundtripOutcome>>,
}

impl RecordingCache {
    /// A cache opened for the default configuration's identity, which every
    /// test that uses it validates under.
    fn new() -> Self {
        Self {
            identity: ValidationConfig::default().cache_identity(),
            stored: std::sync::Mutex::new(Vec::new()),
            roundtrips: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn outcomes(&self) -> Vec<(std::path::PathBuf, CacheOutcome)> {
        self.stored.lock().unwrap().clone()
    }
}

impl talkbank_cache::VerdictReader for RecordingCache {
    fn identity(&self) -> &talkbank_cache::CacheIdentity {
        &self.identity
    }

    fn get(
        &self,
        _: &talkbank_cache::ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
    ) -> Result<CacheLookup<CacheOutcome>, CacheError> {
        Ok(CacheLookup::Miss)
    }

    fn get_roundtrip(
        &self,
        _: &talkbank_cache::ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
    ) -> Result<CacheLookup<RoundtripOutcome>, CacheError> {
        Ok(CacheLookup::Miss)
    }
}

impl ValidationCache for RecordingCache {
    fn set(
        &self,
        path: &talkbank_cache::ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
        outcome: CacheOutcome,
    ) -> Result<(), CacheError> {
        self.stored
            .lock()
            .unwrap()
            .push((path.as_path().to_path_buf(), outcome));
        Ok(())
    }

    fn set_roundtrip(
        &self,
        _: &talkbank_cache::ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
        outcome: RoundtripOutcome,
    ) -> Result<(), CacheError> {
        self.roundtrips.lock().unwrap().push(outcome);
        Ok(())
    }
}

#[test]
fn roundtrip_internal_failure_is_not_a_mismatch_or_cached_verdict() {
    assert_roundtrip_internal_failure(include_str!(
        "../../../../corpus/reference/languages/eng-conversation.cha"
    ));
}

#[test]
fn roundtrip_internal_failure_does_not_publish_earlier_input_warnings() {
    assert_roundtrip_internal_failure(include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E546_1.cha"
    ));
}

fn assert_roundtrip_internal_failure(input: &str) {
    use super::cancel::CancelSignal;
    use super::worker::WorkerContext;
    use super::worker::{ParserDispatch, worker_loop_with_parser};
    let dir = tempdir().unwrap();
    let path = dir.path().join("roundtrip-producer-fault.cha");
    fs::write(&path, input).unwrap();
    let (event_tx, event_rx) = crossbeam_channel::unbounded();
    let (_canceller, cancel_rx) = crossbeam_channel::unbounded();
    let cache = std::sync::Arc::new(RecordingCache::new());
    let run_cache = RunCache::ReadWrite(cache.clone());
    let cancel = CancelSignal::new(cancel_rx);
    let budget = super::cancel::ErrorBudget::new(super::ErrorLimit::Unlimited);
    let config = ValidationConfig {
        roundtrip: RoundtripCheck::Run,
        presentation: crate::PresentationPolicy::new().downgrade(
            talkbank_model::ErrorCode::UnsupportedSesValue,
            talkbank_model::Severity::Warning,
        ),
        ..ValidationConfig::default()
    };
    let tally = worker_loop_with_parser(
        std::iter::once(crate::paths::StoredTranscript::resolve(&path).expect("stored name")),
        &WorkerContext {
            event_tx: &event_tx,
            cancel: &cancel,
            budget: &budget,
            cache: &run_cache,
            config: &config,
        },
        ParserDispatch::FailAfterInitial(std::cell::Cell::new(Some(
            talkbank_parser::TreeSitterParser::new().unwrap(),
        ))),
    );
    let events: Vec<_> = event_rx.try_iter().collect();
    assert!(
        matches!(&events[..], [ValidationEvent::FileComplete(complete)]
        if complete.status.shown().is_none()
            && matches!(&complete.status, FileStatus::InternalFailure {
                failure,
                attempt: super::FailedAttempt::RoundtripReparse,
            } if failure.diagnostics()[0].code == talkbank_model::ErrorCode::InternalError)),
        "unexpected events: {events:?}"
    );
    assert!(cache.outcomes().is_empty());
    assert!(cache.roundtrips.lock().unwrap().is_empty());
    let stats = tally.snapshot(NonZeroUsize::MIN);
    assert_eq!(stats.internal_failures(), 1);
    assert_eq!(stats.valid_files() + stats.invalid_files(), 0);
    assert_eq!(stats.roundtrip_passed() + stats.roundtrip_failed(), 0);
    assert_eq!(stats.files_accounted_for(), 1);
}

/// A file producing only warnings (no errors) is cached as Invalid, so its
/// warnings are shown on every run rather than hidden behind a cached Valid.
#[test]
fn warnings_only_file_cached_as_invalid() {
    let dir = tempdir().expect("create temp dir");
    // W110 only: the `@Media` name differs from the file's own name in
    // letter case alone, a warning, and nothing else is wrong.
    let file_path = dir.path().join("Session.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tsession, audio\n\
         *CHI:\thello .\u{15}0_1500\u{15}\n@End\n",
    )
    .expect("write a warning-only test chat file");

    let cache = std::sync::Arc::new(RecordingCache::new());
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..ValidationConfig::default()
    };

    let (events, _canceller) = validate_directory_streaming(
        dir.path(),
        &bound(&config, RunCache::ReadWrite(cache.clone())),
    );

    // Drain events to completion.
    let mut completed = Vec::new();
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("runner should finish");
        match event {
            ValidationEvent::FileComplete(complete) => completed.push(complete),
            ValidationEvent::Finished(_) => break,
            ValidationEvent::Discovering | ValidationEvent::Started { .. } => {}
        }
    }

    // The file is valid, and its one result carries its warning (W110).
    assert!(
        matches!(&completed[..], [complete]
            if matches!(complete.status, FileStatus::Valid { warnings: Some(_), .. })),
        "a warnings-only file is one valid result with its warnings: {completed:?}"
    );

    // The file must be cached as Invalid so warnings are shown on subsequent runs.
    let outcomes = cache.outcomes();
    assert_eq!(outcomes.len(), 1, "exactly one file should be cached");
    assert_eq!(
        outcomes[0].1,
        CacheOutcome::Invalid,
        "warnings-only file must be cached as Invalid to prevent hiding warnings"
    );
}

/// A valid file with no warnings should be cached as Valid.
#[test]
fn clean_file_cached_as_valid() {
    let dir = tempdir().expect("create temp dir");
    let file_path = dir.path().join("clean.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|demo|CHI|2;00.00|male|||Target_Child|||\n*CHI:\thello .\n@End\n",
    )
    .expect("write clean test chat file");

    let cache = std::sync::Arc::new(RecordingCache::new());
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..ValidationConfig::default()
    };

    let (events, _canceller) = validate_directory_streaming(
        dir.path(),
        &bound(&config, RunCache::ReadWrite(cache.clone())),
    );

    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("runner should finish");
        if matches!(event, ValidationEvent::Finished(_)) {
            break;
        }
    }

    let outcomes = cache.outcomes();
    assert_eq!(outcomes.len(), 1, "exactly one file should be cached");
    assert_eq!(
        outcomes[0].1,
        CacheOutcome::Valid,
        "clean file should be cached as Valid"
    );
}

#[test]
fn a_dropped_canceller_does_not_cancel() {
    let dir = tempdir().expect("create temp dir");
    let file_path = dir.path().join("sample.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|demo|CHI|2;00.00|male|||Target_Child|||\n*CHI:\thello .\n@End\n",
    )
    .expect("write test chat file");

    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        roundtrip: RoundtripCheck::Skip,
        ..ValidationConfig::default()
    };

    let (events, canceller) =
        validate_directory_streaming(dir.path(), &ValidationRun::uncached(config.clone()));
    drop(canceller);

    let (files, ending) = drain(events);
    let file_complete_count = files.len();
    let finished = expect_complete(ending);

    assert_eq!(
        file_complete_count, 1,
        "exactly one file should be processed"
    );
    // Reaching `Complete` (not `Stopped`) is itself the proof that dropping
    // the canceller did not stop the run.
    assert_eq!(
        finished.valid_files() + finished.invalid_files(),
        1,
        "one file should be accounted for in final stats"
    );
}

/// Drain a run to its ending, or fail after a generous timeout.
fn ending_of(events: crossbeam_channel::Receiver<ValidationEvent>) -> RunEnding {
    drain(events).1
}

/// Drain a run's events to its ending, keeping each file's result.
fn drain(
    events: crossbeam_channel::Receiver<ValidationEvent>,
) -> (Vec<super::FileCompleteEvent>, RunEnding) {
    let mut files = Vec::new();
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(30))
            .expect("runner should end its stream");
        match event {
            ValidationEvent::Discovering | ValidationEvent::Started { .. } => {}
            ValidationEvent::FileComplete(file) => files.push(file),
            ValidationEvent::Finished(ending) => return (files, ending),
        }
    }
}

/// The totals of a complete run; any other ending fails the test, named,
/// instead of leaving a drain waiting for a `Complete` that never comes.
fn expect_complete(ending: RunEnding) -> ValidationStatsSnapshot {
    match ending {
        RunEnding::Complete(stats) => stats.snapshot().clone(),
        other => panic!("expected a complete run, got {other:?}"),
    }
}

/// The error limit is the runner's, counted from each file's status: with
/// one worker and a limit of 1, the first invalid file stops the run before
/// the next file starts, deterministically, and the run says so.
#[test]
fn the_error_limit_stops_the_run_itself() {
    let dir = tempdir().expect("create temp dir");
    for name in ["a", "b", "c"] {
        fs::write(
            dir.path().join(format!("{name}.cha")),
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello .\n",
        )
        .expect("write invalid file");
    }
    let limit = std::num::NonZeroUsize::MIN;
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        error_limit: super::ErrorLimit::StopAfter(limit),
        ..ValidationConfig::default()
    };
    let (events, _canceller) =
        validate_directory_streaming(dir.path(), &ValidationRun::uncached(config.clone()));
    match ending_of(events) {
        RunEnding::Stopped { stats, reason } => {
            let unprocessed = stats.missing_files();
            let stats = stats.snapshot();
            assert_eq!(reason, super::CancelReason::ErrorLimit { limit });
            assert_eq!(unprocessed.get(), 2, "{stats:?}");
            assert_eq!(stats.invalid_files(), 1);
        }
        other => panic!("the limit must stop the run, got {other:?}"),
    }
}

/// Warnings never spend the limit: warning-only files (W110, a `@Media`
/// name that differs from the file's only in letter case) under a limit of
/// 1 are all validated and the run is `Complete`.
#[test]
fn warnings_do_not_spend_the_error_limit() {
    let dir = tempdir().expect("create temp dir");
    for name in ["Session", "Other"] {
        fs::write(
            dir.path().join(format!("{name}.cha")),
            format!(
                "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                 @ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\t{}, audio\n\
                 *CHI:\thello .\u{15}0_1500\u{15}\n@End\n",
                name.to_lowercase()
            ),
        )
        .expect("write warning-only file");
    }
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        error_limit: super::ErrorLimit::StopAfter(std::num::NonZeroUsize::MIN),
        ..ValidationConfig::default()
    };
    let (events, _canceller) =
        validate_directory_streaming(dir.path(), &ValidationRun::uncached(config.clone()));
    match ending_of(events) {
        RunEnding::Complete(stats) => assert_eq!(stats.snapshot().valid_files(), 2),
        other => panic!("warnings must not stop the run, got {other:?}"),
    }
}

// =============================================================================
// Terminal-event guarantee
// =============================================================================

/// A validation thread that unwinds must still put a terminal event on the
/// stream, so no consumer is left waiting on a run that is already dead.
///
/// WHAT THIS DOES NOT COVER: it constructs the guard the way
/// [`super::runner::validate_directory_streaming`] does and then unwinds the
/// thread, rather than making the real runner panic. There is no injection
/// seam for a panic inside the orchestrator itself (a panicking WORKER is a
/// different case: `join` catches it and the run ends with
/// `RunEnding::Incomplete`, covered separately below), so forcing one through the
/// public entrypoint is not possible without adding a fault switch to
/// production code. What is verified here is the mechanism the
/// entrypoints rely on: armed guard plus unwinding thread yields exactly one
/// `Aborted` ending.
#[test]
fn a_thread_that_unwinds_while_holding_the_guard_still_delivers_a_run_ending() {
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<ValidationEvent>();

    // The panic is the subject of the test, not an accident, so its unwind is
    // deliberately allowed to reach the thread boundary and is absorbed by the
    // `join` below. It prints a panic message to stderr; that is expected.
    let unwound = std::thread::spawn(move || {
        let _guard = TerminalGuard::armed(event_tx.clone());
        let _ = event_tx.send(ValidationEvent::Discovering);
        #[allow(clippy::panic)]
        {
            panic!("simulated orchestrator failure");
        }
    })
    .join();

    assert!(unwound.is_err(), "the spawned thread should have unwound");

    let events: Vec<ValidationEvent> = event_rx.into_iter().collect();

    assert!(
        matches!(events.first(), Some(ValidationEvent::Discovering)),
        "events sent before the unwind should still arrive, got {events:?}"
    );
    assert!(
        matches!(
            events.last(),
            Some(ValidationEvent::Finished(RunEnding::Aborted(
                AbortReason::Panicked
            )))
        ),
        "an unwound run must end the stream with Aborted, got {events:?}"
    );
}

/// The normal path must stay silent: a disarmed guard reports nothing, so a
/// completed run is never also reported as aborted.
#[test]
fn a_disarmed_guard_reports_nothing() {
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<ValidationEvent>();

    {
        let mut guard = TerminalGuard::armed(event_tx.clone());
        guard.disarm();
    }
    drop(event_tx);

    let events: Vec<ValidationEvent> = event_rx.into_iter().collect();

    assert!(
        events.is_empty(),
        "a disarmed guard must add nothing to the stream, got {events:?}"
    );
}

// =============================================================================
// Coverage: a run that lost files must not be reportable as a clean finish
// =============================================================================

/// What one coverage case describes.
///
/// A named struct rather than positional arguments, because the two counts
/// are both `usize`, so transposing them would silently invert the case
/// under test (a run that lost 2 files versus one that discovered fewer than
/// it processed).
struct Coverage {
    /// Files discovery found.
    discovered: usize,
    /// Files actually accounted for by the run.
    accounted_for: usize,
}

/// Build a snapshot describing one [`Coverage`] case, the way the runner
/// does: one recorded status per file accounted for.
fn snapshot_covering(coverage: Coverage) -> ValidationStatsSnapshot {
    let Coverage {
        discovered,
        accounted_for,
    } = coverage;
    let mut tally = ValidationTally::default();
    for _ in 0..accounted_for {
        tally.record(
            &FileStatus::Valid {
                roundtrip: RoundtripVerdict::NotRequested,
                warnings: None,
            },
            CacheUse::Miss,
        );
    }
    tally.snapshot(NonZeroUsize::new(discovered).expect("a case discovers files"))
}

/// Five discovered, three accounted for.
const SHORT: Coverage = Coverage {
    discovered: 5,
    accounted_for: 3,
};

/// A run whose workers abandoned files must NOT end with `Complete`, even
/// if a stop was also requested: the crash, not the stop, explains files a
/// worker took and never finished.
///
/// It calls the terminal-event decision directly. That a real worker panic
/// reaches it as `PoolOutcome::SomeUnwound`, with the panicking worker's
/// items missing, is tested on the shared pool itself
/// (`worker_pool::tests::an_unwinding_worker_is_counted_and_the_rest_return`).
/// The decision function under test IS the one the runner ships.
#[test]
fn a_run_that_lost_files_is_incomplete() {
    for stop in [None, Some(super::CancelReason::Requested)] {
        let faults = super::WorkerFaults::observe(
            &PoolOutcome::SomeUnwound {
                unwound_workers: std::num::NonZeroUsize::MIN,
            },
            [],
        );
        let ending = run_ending(snapshot_covering(SHORT), stop, faults);
        match ending {
            RunEnding::Incomplete { stats, cause } => {
                let lost_files = stats.missing_files();
                let stats = stats.snapshot();
                assert_eq!(
                    cause.to_string(),
                    "1 worker(s) failed with an internal error."
                );
                assert_eq!(
                    lost_files.get(),
                    2,
                    "two discovered files produced no result"
                );
                assert_eq!(
                    stats.valid_files(),
                    3,
                    "the stats must describe only what was processed"
                );
            }
            other => panic!("a run that lost files must be Incomplete, got {other:?}"),
        }
    }
}

/// A run told to stop that stopped short is `Stopped`, with the files it
/// never reached and the reason: a requested shortfall, not lost data, and
/// never a `Complete` that a consumer could read as covering everything.
#[test]
fn a_stopped_run_is_stopped_not_complete() {
    let limit = std::num::NonZeroUsize::MIN;
    for reason in [
        super::CancelReason::Requested,
        super::CancelReason::ErrorLimit { limit },
    ] {
        let event = run_ending(snapshot_covering(SHORT), Some(reason), None);
        assert!(
            matches!(event, RunEnding::Stopped { ref stats, reason: got }
                if stats.missing_files().get() == 2 && got == reason),
            "got {event:?}"
        );
    }
}

/// Full coverage ends with `Complete` whether or not a stop arrived: a stop
/// latched after the last file stopped nothing, so nothing is reported as
/// stopped.
#[test]
fn a_fully_covered_run_is_complete_even_if_told_to_stop() {
    let full = || Coverage {
        discovered: 5,
        accounted_for: 5,
    };
    for stop in [None, Some(super::CancelReason::Requested)] {
        let event = run_ending(snapshot_covering(full()), stop, None);
        assert!(
            matches!(event, RunEnding::Complete(_)),
            "a run that accounted for every file must be Complete, got {event:?}"
        );
    }
}

/// Workers that cannot create their parser take no file, so when every one
/// fails the whole input is lost. That is a fault the run reports, with the
/// parser's reason, even when a stop was also latched: the stop did not lose
/// those files.
#[test]
fn a_worker_without_a_parser_is_a_reported_fault_not_a_stop() {
    let failure = super::worker::WorkerSetupFailure {
        parser: talkbank_model::ParserKind::TreeSitter,
        reason: "grammar version mismatch".to_owned(),
    };
    let faults =
        super::WorkerFaults::observe(&PoolOutcome::AllReturned, [&failure, &failure, &failure]);
    for stop in [None, Some(super::CancelReason::Requested)] {
        let event = run_ending(snapshot_covering(SHORT), stop, faults.clone());
        let RunEnding::Incomplete { cause, .. } = event else {
            panic!("a run whose workers could not start must be Incomplete, got {event:?}");
        };
        let super::LossCause::WorkerFaults(listed) = &cause else {
            panic!("the parser failure must be the cause, got {cause:?}");
        };
        assert_eq!(
            listed.iter().cloned().collect::<Vec<_>>(),
            vec![super::WorkerFault::ParserUnavailable {
                workers: std::num::NonZeroUsize::new(3).expect("non-zero"),
                parser: talkbank_model::ParserKind::TreeSitter,
                reason: "grammar version mismatch".to_owned(),
            }]
        );
    }
}

/// Faults are observed together, never one hiding another; a pool whose
/// workers all started and returned has none, and a loss with neither a
/// fault nor a stop is said to be unexplained rather than given a cause.
#[test]
fn worker_faults_are_all_observed_and_absent_when_nothing_failed() {
    assert_eq!(
        super::WorkerFaults::observe(&PoolOutcome::AllReturned, []),
        None
    );
    let failure = super::worker::WorkerSetupFailure {
        parser: talkbank_model::ParserKind::Re2c,
        reason: "unavailable".to_owned(),
    };
    let faults = super::WorkerFaults::observe(
        &PoolOutcome::CouldNotStart {
            error: std::io::Error::other("no threads"),
            unwound_workers: 1,
        },
        [&failure],
    )
    .expect("three faults");
    assert_eq!(
        super::LossCause::WorkerFaults(faults).to_string(),
        "a worker thread could not be started (no threads); \
         1 worker(s) failed with an internal error; \
         1 worker(s) could not start the Re2c parser (unavailable)."
    );
    let event = run_ending(snapshot_covering(SHORT), None, None);
    assert!(
        matches!(
            event,
            RunEnding::Incomplete {
                cause: super::LossCause::Unexplained,
                ..
            }
        ),
        "got {event:?}"
    );
}

/// A directory the walk cannot read is reported as a file that could not be
/// read and counted in the totals, so the run cannot look like a clean run
/// over a smaller corpus.
#[cfg(unix)]
#[test]
fn an_unreadable_directory_is_reported_as_a_read_error() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().expect("create temp dir");
    let locked = dir.path().join("locked");
    fs::create_dir_all(&locked).expect("create locked dir");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("lock dir");
    let readable_anyway = fs::read_dir(&locked).is_ok();

    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..ValidationConfig::default()
    };
    let (events, _canceller) =
        validate_directory_streaming(dir.path(), &ValidationRun::uncached(config.clone()));
    let (read_errors, ending) = drain(events);
    let finished = expect_complete(ending);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("unlock dir");
    assert!(
        !readable_anyway,
        "a mode 000 directory was readable: run this test as an ordinary user, not root"
    );
    assert_eq!(finished.total_files().get(), 1);
    assert!(
        matches!(&read_errors[..], [complete]
            if complete.path == locked
                && matches!(complete.status, super::FileStatus::ReadError { .. })),
        "{read_errors:?}"
    );
}

/// A cache that fails every read and write, as a locked or corrupt database
/// does.
struct FailingCache(talkbank_cache::CacheIdentity);

impl FailingCache {
    fn failure() -> CacheError {
        CacheError::CorruptColumn {
            column: "is_valid",
            value: Some(7),
        }
    }
}

impl talkbank_cache::VerdictReader for FailingCache {
    fn identity(&self) -> &talkbank_cache::CacheIdentity {
        &self.0
    }

    fn get(
        &self,
        _: &talkbank_cache::ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
    ) -> Result<CacheLookup<CacheOutcome>, CacheError> {
        Err(Self::failure())
    }

    fn get_roundtrip(
        &self,
        _: &talkbank_cache::ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
    ) -> Result<CacheLookup<RoundtripOutcome>, CacheError> {
        Err(Self::failure())
    }
}

impl ValidationCache for FailingCache {
    fn set(
        &self,
        _: &talkbank_cache::ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
        _: CacheOutcome,
    ) -> Result<(), CacheError> {
        Err(Self::failure())
    }

    fn set_roundtrip(
        &self,
        _: &talkbank_cache::ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
        _: RoundtripOutcome,
    ) -> Result<(), CacheError> {
        Err(Self::failure())
    }
}

/// A failing cache is counted in the run's totals, not mistaken for a cold
/// one, and the file is still validated without it: one failed read and one
/// failed write.
#[test]
fn a_failing_cache_is_counted_and_the_file_still_validated() {
    let dir = tempdir().expect("create temp dir");
    fs::write(
        dir.path().join("clean.cha"),
        include_str!("../../../../corpus/reference/languages/eng-conversation.cha"),
    )
    .expect("write test file");
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..ValidationConfig::default()
    };
    let failing = std::sync::Arc::new(FailingCache(config.cache_identity()));
    let (events, _cancel) =
        validate_directory_streaming(dir.path(), &bound(&config, RunCache::ReadWrite(failing)));
    let finished = expect_complete(ending_of(events));
    assert_eq!(finished.files_accounted_for(), 1);
    assert_eq!(finished.cache_hits(), 0);
    assert_eq!(finished.cache_errors(), 2, "{finished:?}");
}

/// An input with no transcript at all ends `NothingFound`, the one ending
/// for "validated nothing", and does not pass: no total of zero for a
/// consumer to check, and no `Complete` claiming a whole input.
#[test]
fn an_input_with_nothing_to_validate_ends_nothing_found() {
    let dir = tempdir().expect("create temp dir");
    fs::write(dir.path().join("notes.txt"), "not a transcript").expect("write");
    let (events, _canceller) = validate_directory_streaming(
        dir.path(),
        &ValidationRun::uncached(ValidationConfig::default()),
    );
    let ending = ending_of(events);
    assert_eq!(ending, RunEnding::NothingFound);
    assert!(!ending.passed());
    assert!(ending.stats().is_none());
}

/// A diagnostic of `severity` over the first byte, for building statuses.
fn diagnostic(severity: talkbank_model::Severity) -> talkbank_model::ParseError {
    talkbank_model::ParseError::at_span(
        talkbank_model::ErrorCode::InternalError,
        severity,
        talkbank_model::Span::new(0, 1),
        "finding",
    )
}

/// An invalid file's diagnostics with `errors` errors, through the one
/// classifier that makes them.
fn invalid(errors: usize) -> super::InvalidDiagnostics {
    let shown = (0..errors)
        .map(|_| diagnostic(talkbank_model::Severity::Error))
        .collect();
    match super::types::Shown::of(shown, "x".to_owned()) {
        super::types::Shown::Errors(diagnostics) => diagnostics,
        super::types::Shown::Nothing | super::types::Shown::Warnings(_) => {
            panic!("{errors} errors classify as invalid")
        }
    }
}

/// The classifier decides validity and the error count from one list: no
/// diagnostic is nothing, warnings alone leave the file valid, and any
/// error makes it invalid with every diagnostic kept and only errors
/// counted.
#[test]
fn shown_diagnostics_are_classified_once() {
    use super::types::Shown;
    use talkbank_model::Severity;
    assert!(matches!(
        Shown::of(Vec::new(), String::new()),
        Shown::Nothing
    ));
    assert!(matches!(
        Shown::of(vec![diagnostic(Severity::Warning)], String::new()),
        Shown::Warnings(warnings) if warnings.errors().len() == 1
    ));
    assert!(matches!(
        Shown::of(
            vec![diagnostic(Severity::Warning), diagnostic(Severity::Error)],
            String::new(),
        ),
        Shown::Errors(diagnostics)
            if diagnostics.error_count().get() == 1 && diagnostics.shown().errors().len() == 2
    ));
}

/// A tool failure's diagnostics are placed in the file's text only when
/// the failed attempt was on that text: a roundtrip reparse failure keeps
/// them as evidence but shows none against the file.
#[test]
fn only_a_failure_on_the_files_own_text_shows_against_it() {
    let failure = || {
        talkbank_model::CompletedDiagnostics::admit(vec![diagnostic(
            talkbank_model::Severity::Warning,
        )])
        .expect_err("an internal error is a failure")
    };
    let on_source = FileStatus::InternalFailure {
        failure: failure(),
        attempt: super::FailedAttempt::Validation {
            source: "text".to_owned(),
        },
    };
    assert!(
        on_source
            .shown()
            .is_some_and(|shown| shown.errors.len() == 1 && shown.source == "text")
    );
    let on_reparse = FileStatus::InternalFailure {
        failure: failure(),
        attempt: super::FailedAttempt::RoundtripReparse,
    };
    assert!(on_reparse.shown().is_none());
    assert!(on_source.failed() && on_reparse.failed());
}

/// `passed` is the one answer every surface gives: a complete run with no
/// invalid, unreadable or tool-failed file passes, warnings or not; any
/// such file, or any other ending, fails.
#[test]
fn only_a_complete_run_without_failed_files_passes() {
    let one = NonZeroUsize::MIN;
    let complete = |status: FileStatus| {
        let mut tally = ValidationTally::default();
        tally.record(&status, CacheUse::NotConsulted);
        run_ending(tally.snapshot(one), None, None)
    };
    assert!(
        complete(FileStatus::Valid {
            roundtrip: RoundtripVerdict::NotRequested,
            warnings: None,
        })
        .passed()
    );
    assert!(
        !complete(FileStatus::ReadError {
            message: "gone".to_owned()
        })
        .passed()
    );
    assert!(
        !complete(FileStatus::Invalid {
            diagnostics: invalid(1)
        })
        .passed()
    );
    let failure =
        talkbank_model::CompletedDiagnostics::admit(vec![talkbank_model::ParseError::at_span(
            talkbank_model::ErrorCode::InternalError,
            talkbank_model::Severity::Warning,
            talkbank_model::Span::new(0, 1),
            "fault",
        )])
        .expect_err("an internal error is a failure");
    assert!(
        !complete(FileStatus::InternalFailure {
            failure,
            attempt: super::FailedAttempt::RoundtripReparse,
        })
        .passed()
    );
    let stopped = run_ending(
        ValidationTally::default().snapshot(one),
        Some(super::CancelReason::Requested),
        None,
    );
    assert!(matches!(stopped, RunEnding::Stopped { .. }));
    assert!(!stopped.passed());
    assert!(!RunEnding::Aborted(AbortReason::NoEnding).passed());
}

// =============================================================================
// Tally
// =============================================================================

/// A cloned stopped snapshot remains partial when admitted again. The
/// shortfall and read-only counts travel together, including through cloning.
#[test]
fn partial_statistics_cannot_be_promoted_by_cloning() {
    let stopped = run_ending(
        snapshot_covering(SHORT),
        Some(super::CancelReason::Requested),
        None,
    );
    let cloned = stopped.stats().expect("partial counts").clone();
    let ending = run_ending(cloned, None, None);
    assert!(!ending.passed());
    let RunEnding::Incomplete { stats, cause } = ending else {
        panic!("partial counts must stay incomplete");
    };
    assert_eq!(cause, super::LossCause::Unexplained);
    let cloned = stats.clone();
    assert_eq!(cloned.missing_files().get(), 2);
    assert_eq!(
        cloned.snapshot().files_accounted_for() + cloned.missing_files().get(),
        cloned.snapshot().total_files().get()
    );
}

#[test]
fn an_empty_tally_reports_zero() {
    let snap = ValidationTally::default().snapshot(NonZeroUsize::new(10).expect("ten"));
    assert_eq!(snap.total_files().get(), 10);
    assert_eq!(snap.files_accounted_for(), 0);
    assert_eq!(
        snap.cache_hits() + snap.cache_misses() + snap.cache_errors(),
        0
    );
}

/// Each status lands in exactly one outcome bucket, and its cache use in at
/// most one cache bucket: a file that consulted no cache is neither a hit nor
/// a miss. Roundtrip failures and read errors count as invalid files.
#[test]
fn the_tally_counts_each_status_once() {
    let mut tally = ValidationTally::default();
    for (status, cache) in [
        (
            FileStatus::Valid {
                roundtrip: RoundtripVerdict::Passed,
                warnings: None,
            },
            CacheUse::Hit,
        ),
        (
            FileStatus::Valid {
                roundtrip: RoundtripVerdict::NotRequested,
                warnings: None,
            },
            CacheUse::Miss,
        ),
        (
            FileStatus::Invalid {
                diagnostics: invalid(2),
            },
            CacheUse::Miss,
        ),
        (
            FileStatus::RoundtripFailed {
                reason: "changed".to_owned(),
                diff: None,
                warnings: None,
            },
            CacheUse::Miss,
        ),
        (
            FileStatus::ReadError {
                message: "denied".to_owned(),
            },
            CacheUse::NotConsulted,
        ),
    ] {
        tally.record(&status, cache);
    }
    tally.record_cache_error();

    let snap = tally.snapshot(NonZeroUsize::new(10).expect("ten"));
    assert_eq!(snap.total_files().get(), 10);
    assert_eq!(snap.valid_files(), 2);
    assert_eq!(snap.invalid_files(), 3);
    assert_eq!(snap.files_accounted_for(), 5);
    assert_eq!(snap.cache_hits(), 1);
    assert_eq!(snap.cache_misses(), 3, "the unread file consulted no cache");
    assert_eq!(snap.cache_errors(), 1);
    assert_eq!(snap.roundtrip_passed(), 1);
    assert_eq!(snap.roundtrip_failed(), 1);
    // One hit in four consultations.
    assert_eq!(snap.cache_hit_rate(), Some(25.0));
}

/// A run that consulted no cache has no hit rate, not a 0% one.
#[test]
fn a_run_without_a_cache_has_no_hit_rate() {
    let mut tally = ValidationTally::default();
    tally.record(
        &FileStatus::Valid {
            roundtrip: RoundtripVerdict::NotRequested,
            warnings: None,
        },
        CacheUse::NotConsulted,
    );
    let snap = tally.snapshot(NonZeroUsize::MIN);
    assert_eq!(snap.cache_hits() + snap.cache_misses(), 0);
    assert_eq!(snap.cache_hit_rate(), None);
}

/// Two workers' tallies sum field by field.
#[test]
fn tallies_add() {
    let mut one = ValidationTally::default();
    one.record(
        &FileStatus::Valid {
            roundtrip: RoundtripVerdict::NotRequested,
            warnings: None,
        },
        CacheUse::Hit,
    );
    let mut two = ValidationTally::default();
    two.record(
        &FileStatus::Invalid {
            diagnostics: invalid(1),
        },
        CacheUse::Miss,
    );
    two.record_cache_error();
    let snap = one.add(two).snapshot(NonZeroUsize::new(2).expect("two"));
    assert_eq!(snap.valid_files(), 1);
    assert_eq!(snap.invalid_files(), 1);
    assert_eq!(snap.cache_hits(), 1);
    assert_eq!(snap.cache_misses(), 1);
    assert_eq!(snap.cache_errors(), 1);
}

/// A cache opened for one rule set cannot be bound to a run under another,
/// so it cannot serve its verdicts there: verdicts recorded with strict
/// linkers are not a lenient run's answers. A cache opened for the run's own
/// identity binds.
#[test]
fn a_cache_opened_for_other_rules_cannot_be_bound_to_this_run() {
    let strict = ValidationConfig {
        rules: talkbank_model::RuleSelection::new().with_strict_linkers(),
        ..ValidationConfig::default()
    };
    let lenient = ValidationConfig::default();
    let strict_cache = std::sync::Arc::new(
        talkbank_cache::CachePool::in_memory(strict.cache_identity()).expect("cache opens"),
    );
    let refused = ValidationRun::new(lenient.clone(), RunCache::ReadWrite(strict_cache.clone()));
    assert_eq!(
        refused.map(|_| ()),
        Err(super::CacheIdentityMismatch {
            run: lenient.cache_identity(),
            cache: strict.cache_identity(),
        })
    );
    let read_only = RunCache::ReadOnly(strict_cache.clone());
    assert!(ValidationRun::new(lenient, read_only).is_err());
    assert!(ValidationRun::new(strict, RunCache::ReadWrite(strict_cache)).is_ok());
}

/// The run for `config` with `cache`, which the test opened for `config`.
fn bound(config: &ValidationConfig, cache: RunCache) -> ValidationRun {
    ValidationRun::new(config.clone(), cache).expect("a cache opened for this run's identity")
}

/// A file list that names one transcript twice (`dir/a.cha` and
/// `dir/./a.cha`) validates it once: one result, one file in the totals,
/// as the argument path already did.
#[test]
fn a_file_named_twice_in_a_file_list_is_validated_once() {
    let dir = tempdir().expect("create temp dir");
    let file = dir.path().join("conversation.cha");
    fs::write(
        &file,
        include_str!("../../../../corpus/reference/languages/eng-conversation.cha"),
    )
    .expect("write a reference transcript");
    let respelled = dir.path().join(".").join("conversation.cha");
    let run = ValidationRun::uncached(ValidationConfig {
        jobs: Some(NonZeroUsize::MIN),
        ..ValidationConfig::default()
    });
    let (events, _canceller) = super::validate_files_streaming(vec![file.clone(), respelled], &run);
    let (files, ending) = drain(events);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(files[0].path, file, "the first spelling given");
    assert_eq!(expect_complete(ending).total_files().get(), 1);
}
