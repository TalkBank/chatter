//! Regression tests for validation-runner orchestration behavior.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::runner::{TerminalGuard, WorkerPoolOutcome, terminal_event};
use super::{
    AbortReason, CacheMode, CacheOutcome, ValidationCache, ValidationConfig, ValidationEvent,
    ValidationStatsSnapshot, validate_directory_streaming,
};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;

struct NoopCache;

impl ValidationCache for NoopCache {
    fn get(&self, _path: &Path, _check_alignment: bool) -> Option<CacheOutcome> {
        None
    }

    fn set(
        &self,
        _path: &Path,
        _check_alignment: bool,
        _outcome: CacheOutcome,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Cache that records what was stored, for verifying cache semantics.
struct RecordingCache {
    stored: std::sync::Mutex<Vec<(std::path::PathBuf, CacheOutcome)>>,
}

impl RecordingCache {
    fn new() -> Self {
        Self {
            stored: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn outcomes(&self) -> Vec<(std::path::PathBuf, CacheOutcome)> {
        self.stored.lock().unwrap().clone()
    }
}

impl ValidationCache for RecordingCache {
    fn get(&self, _path: &Path, _check_alignment: bool) -> Option<CacheOutcome> {
        None
    }

    fn set(
        &self,
        path: &Path,
        _check_alignment: bool,
        outcome: CacheOutcome,
    ) -> Result<(), String> {
        self.stored
            .lock()
            .unwrap()
            .push((path.to_path_buf(), outcome));
        Ok(())
    }
}

/// Regression test: a file producing only warnings (no errors) must be cached
/// as Invalid so warnings are shown on every run. Previously, warnings-only
/// files were cached as Valid, silently hiding warnings on subsequent runs.
#[test]
fn warnings_only_file_cached_as_invalid() {
    let dir = tempdir().expect("create temp dir");
    // This file triggers E546 warning (unsupported SES value "badses") but is
    // otherwise valid CHAT, producing warnings but no errors.
    let file_path = dir.path().join("warnings.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|3;06.|female||badses|Target_Child|||\n*CHI:\thello world .\n@End\n",
    )
    .expect("write test chat file with warning-producing SES value");

    let cache = std::sync::Arc::new(RecordingCache::new());
    let config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        ..ValidationConfig::default()
    };

    let (events, _cancel_tx) =
        validate_directory_streaming(dir.path(), &config, Some(cache.clone()));

    // Drain events to completion.
    let mut error_events = Vec::new();
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("runner should finish");
        match event {
            ValidationEvent::Errors(e) => error_events.push(e),
            ValidationEvent::Finished(_) => break,
            _ => {}
        }
    }

    // The file should produce at least one warning (E546).
    assert!(
        !error_events.is_empty(),
        "file with unsupported SES should produce warning events"
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
        jobs: Some(1),
        cache: CacheMode::Enabled,
        ..ValidationConfig::default()
    };

    let (events, _cancel_tx) =
        validate_directory_streaming(dir.path(), &config, Some(cache.clone()));

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
fn dropped_cancel_sender_does_not_cancel_and_jobs_zero_still_processes_files() {
    let dir = tempdir().expect("create temp dir");
    let file_path = dir.path().join("sample.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|demo|CHI|2;00.00|male|||Target_Child|||\n*CHI:\thello .\n@End\n",
    )
    .expect("write test chat file");

    let config = ValidationConfig {
        jobs: Some(0),
        cache: CacheMode::Disabled,
        roundtrip: false,
        ..ValidationConfig::default()
    };

    let (events, cancel_tx) = validate_directory_streaming::<NoopCache>(dir.path(), &config, None);
    drop(cancel_tx);

    let mut file_complete_count = 0usize;
    let finished = loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("runner should emit events and finish");
        match event {
            ValidationEvent::FileComplete(_) => {
                file_complete_count += 1;
            }
            ValidationEvent::Finished(stats) => break stats,
            _ => {}
        }
    };

    assert_eq!(
        file_complete_count, 1,
        "exactly one file should be processed"
    );
    assert!(
        !finished.cancelled,
        "dropping cancel sender must not cancel run"
    );
    assert_eq!(
        finished.valid_files + finished.invalid_files + finished.parse_errors,
        1,
        "one file should be accounted for in final stats"
    );
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
/// `FinishedIncomplete`, covered separately below), so forcing one through the
/// public entrypoint is not possible without adding a fault switch to
/// production code. What is verified here is the mechanism the
/// entrypoints rely on: armed guard plus unwinding thread yields exactly one
/// `Aborted` event.
#[test]
fn a_thread_that_unwinds_while_holding_the_guard_still_delivers_a_terminal_event() {
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
            Some(ValidationEvent::Aborted(AbortReason::Panicked))
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
/// A named struct rather than three positional arguments, because
/// a three-positional-argument call said nothing at the call site about which
/// number was which, and the two counts are both `usize`, so transposing them
/// would silently invert the case under test (a run that lost 2 files versus
/// one that discovered fewer than it processed). The bool had the same
/// problem: `false` meant "the user did not cancel", which is the whole
/// difference between a legitimate shortfall and lost data.
struct Coverage {
    /// Files discovery found.
    discovered: usize,
    /// Files actually accounted for by the run.
    accounted_for: usize,
    /// Whether the user asked the run to stop.
    cancelled: bool,
}

/// Build a snapshot describing one [`Coverage`] case.
fn snapshot_covering(coverage: Coverage) -> ValidationStatsSnapshot {
    let Coverage {
        discovered,
        accounted_for,
        cancelled,
    } = coverage;
    ValidationStatsSnapshot {
        total_files: discovered,
        valid_files: accounted_for,
        invalid_files: 0,
        cache_hits: 0,
        cache_misses: accounted_for,
        parse_errors: 0,
        roundtrip_passed: 0,
        roundtrip_failed: 0,
        cancelled,
    }
}

/// A run whose workers abandoned files must NOT end with `Finished`.
///
/// `Finished` is the only warrant for a claim about the whole input, so a run
/// that validated 3 of 5 files and reported `Finished` would tell every
/// consumer that 5 files are clean when 2 were never opened. Before this, the
/// runner logged `had_panic` to `tracing` (invisible to any GUI user) and sent
/// `Finished` with the partial stats regardless.
///
/// WHAT THIS DOES NOT COVER: it calls the terminal-event decision directly
/// rather than making a real worker panic, because `worker_loop` has no fault
/// injection seam and adding one would put test-only machinery in the hot
/// path. The decision function under test IS the one the runner ships.
#[test]
fn a_run_that_lost_files_is_not_reported_as_finished() {
    let event = terminal_event(
        snapshot_covering(Coverage {
            discovered: 5,
            accounted_for: 3,
            cancelled: false,
        }),
        WorkerPoolOutcome::SomeUnwound { unwound_workers: 1 },
    );

    match event {
        ValidationEvent::FinishedIncomplete { stats, lost_files } => {
            assert_eq!(lost_files, 2, "two discovered files produced no result");
            assert_eq!(
                stats.valid_files, 3,
                "the stats must describe only what was processed"
            );
        }
        other => {
            panic!("a run that lost files must report FinishedIncomplete, got {other:?}")
        }
    }
}

/// Cancellation is a requested shortfall, not lost data: a cancelled run
/// still ends with `Finished`, carrying its own `cancelled` flag. Reporting it
/// as incomplete would train users to ignore the incomplete report.
#[test]
fn a_cancelled_run_is_finished_not_incomplete() {
    let event = terminal_event(
        snapshot_covering(Coverage {
            discovered: 5,
            accounted_for: 3,
            cancelled: true,
        }),
        WorkerPoolOutcome::AllReturned,
    );

    assert!(
        matches!(event, ValidationEvent::Finished(stats) if stats.cancelled),
        "a cancelled run must end with Finished"
    );
}

/// Full coverage still ends with `Finished`, so the guarantee above cannot be
/// satisfied by declaring every run incomplete.
#[test]
fn a_fully_covered_run_is_finished() {
    let event = terminal_event(
        snapshot_covering(Coverage {
            discovered: 5,
            accounted_for: 5,
            cancelled: false,
        }),
        WorkerPoolOutcome::AllReturned,
    );

    assert!(
        matches!(event, ValidationEvent::Finished(_)),
        "a run that accounted for every file must end with Finished"
    );
}
