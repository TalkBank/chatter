//! Desktop validation orchestration for a single selected target.
//!
//! Chatter's desktop contract is one target at a time:
//! - one `.cha` file
//! - or one directory
//!
//! Both cases route through the exact same shared streaming entrypoints the
//! CLI uses (`talkbank_transform::validation_runner::{validate_directory_streaming,
//! validate_files_streaming}`), with a real on-disk cache. Desktop must not
//! reimplement cache lookups, stats accounting, or per-file rule dispatch;
//! see `apps/chatter-desktop/CLAUDE.md` ("No desktop-local domain logic").

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::errors::TargetError;
use crossbeam_channel::{Receiver, Sender, unbounded};
use talkbank_transform::validation_runner::{
    ParserKind, ValidationConfig, ValidationEvent, is_chat_transcript_path,
    validate_directory_streaming, validate_files_streaming,
};
use talkbank_transform::{RulesVersion, UnifiedCache};

use crate::events::{FrontendEvent, to_frontend_event};
use crate::protocol::commands::{ParserKindRequest, ValidateRequest};

impl From<ParserKindRequest> for ParserKind {
    fn from(value: ParserKindRequest) -> Self {
        match value {
            ParserKindRequest::TreeSitter => ParserKind::TreeSitter,
            ParserKindRequest::Re2c => ParserKind::Re2c,
        }
    }
}

impl From<&ValidateRequest> for ValidationConfig {
    fn from(request: &ValidateRequest) -> Self {
        // `strict_linkers` selects RULES (it turns on E351-E355), so it lives
        // in the rule selection, which is also what keys the cache. The desktop
        // request carries no `--suppress` equivalent yet, so the presentation
        // policy stays the default: show everything the validator computed.
        let rules = if request.strict_linkers {
            talkbank_model::RuleSelection::new().with_strict_linkers()
        } else {
            talkbank_model::RuleSelection::new()
        };
        Self {
            roundtrip: request.roundtrip,
            parser_kind: request.parser_kind.into(),
            rules,
            jobs: request.jobs.map(|jobs| jobs as usize),
            ..Self::default()
        }
    }
}

/// Start validation for a single desktop target with an explicit config and
/// cache, used by the `validate` Tauri command once a `ValidateRequest`
/// carries user-chosen settings (roundtrip, parser kind, strict linkers,
/// jobs). The cache is a parameter, not built here, so the app can open it
/// once at startup (`ValidationState::new()`) and reuse it across every
/// validate/re-validate call instead of paying SQLite-pool setup cost per run.
pub fn validate_target_streaming_with_config(
    target: PathBuf,
    config: ValidationConfig,
    cache: Option<Arc<UnifiedCache>>,
) -> Result<(Receiver<FrontendEvent>, Sender<()>), TargetError> {
    if !target.exists() {
        return Err(TargetError::Missing { path: target });
    }

    if target.is_dir() {
        let (validation_rx, cancel_tx) = validate_directory_streaming(&target, &config, cache);
        Ok((bridge_validation_events(validation_rx, target), cancel_tx))
    } else if target.is_file() {
        if !is_chat_transcript_path(&target) {
            return Err(TargetError::NotChatTranscript { path: target });
        }
        let root = target
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let (validation_rx, cancel_tx) = validate_files_streaming(vec![target], &config, cache);
        Ok((bridge_validation_events(validation_rx, root), cancel_tx))
    } else {
        Err(TargetError::NotFileOrDirectory { path: target })
    }
}

/// Construct the shared on-disk validation cache, the exact same construction
/// the CLI uses (`crates/chatter/src/commands/validate/cache.rs`), minus the
/// `--force`-clear step the desktop has no flag for. Zero CLI dependency:
/// `UnifiedCache::new()` resolves the OS cache dir on its own.
///
/// Keyed to [`RulesVersion::current`] (the default rule selection).
/// Callers that need a pool keyed to a specific active
/// `talkbank_model::RuleSelection` (any desktop request whose settings
/// might not be the default) use [`initialize_cache_with_rules_version`]
/// instead; see `ValidationState::cache_for_rules` in `commands.rs`, the
/// seam that opens (and memoizes) one pool per distinct config a session
/// actually uses, rather than one pool for the app's whole lifetime.
pub fn initialize_cache() -> Option<Arc<UnifiedCache>> {
    UnifiedCache::open_or_else(|error| {
        eprintln!("Warning: Failed to initialize validation cache: {error}");
    })
}

/// [`initialize_cache`], keyed to an explicit [`RulesVersion`] instead of
/// [`RulesVersion::current`].
///
/// Opening a SQLite pool and a dedicated tokio runtime costs real time, so
/// this is not called per validation run either: `ValidationState` opens (at
/// most) one pool per DISTINCT active config a session has actually used,
/// reusing it for every subsequent request under that same config, the same
/// "open once, reuse" behavior the single-cache design had back when only
/// one config could ever exist.
pub fn initialize_cache_with_rules_version(
    rules_version: RulesVersion,
) -> Option<Arc<UnifiedCache>> {
    UnifiedCache::open_or_else_with_rules_version(rules_version, |error| {
        eprintln!("Warning: Failed to initialize validation cache: {error}");
    })
}

/// Open the validation cache rooted at an explicit directory.
///
/// [`initialize_cache`] resolves the cache root from the platform default
/// (or the `TALKBANK_CHAT_CACHE_DIR` override), which is right for the
/// shipping app: there is exactly one cache and the user does not choose
/// where it lives.
///
/// In-process callers that need their OWN cache root, notably tests that
/// must not touch the developer's real cache and must not collide with
/// each other, take this instead. Before it existed the only lever was
/// mutating `TALKBANK_CHAT_CACHE_DIR` with `unsafe { set_var }`, which is
/// a process global: two tests in one binary overwrote each other's root,
/// and the `unsafe` was only sound under a one-process-per-test runner.
/// Passing the directory removes both problems, and the environment
/// variable goes back to its legitimate job of configuring CHILD
/// processes (the CLI subprocess tests set it via `Command::env`).
pub fn initialize_cache_at(cache_dir: PathBuf) -> Option<Arc<UnifiedCache>> {
    match UnifiedCache::with_directory(cache_dir) {
        Ok(cache) => Some(Arc::new(cache)),
        Err(error) => {
            eprintln!("Warning: Failed to initialize validation cache: {error}");
            None
        }
    }
}

/// [`initialize_cache_at`], keyed to an explicit [`RulesVersion`]. Test
/// counterpart of [`initialize_cache_with_rules_version`], used by
/// `ValidationState::new_at` so cache-key tests can isolate BOTH the
/// directory (never the developer's real cache) and the rule set (never a
/// row left behind by a differently-configured earlier test run).
pub fn initialize_cache_at_with_rules_version(
    cache_dir: PathBuf,
    rules_version: RulesVersion,
) -> Option<Arc<UnifiedCache>> {
    match UnifiedCache::with_directory_and_rules_version(cache_dir, rules_version) {
        Ok(cache) => Some(Arc::new(cache)),
        Err(error) => {
            eprintln!("Warning: Failed to initialize validation cache: {error}");
            None
        }
    }
}

/// Why the bridge stopped forwarding events.
///
/// This exists so the loop's exit reason is a VALUE, matched exhaustively at
/// one place below, rather than something each `break` is trusted to handle.
/// The previous loop was `while let Ok(event) = validation_rx.recv()`, which
/// silently discarded the `Err` that `recv` returns on disconnect: a run whose
/// worker thread panicked ended the stream with no terminal event at all, and
/// the frontend waited forever showing whatever phase it was in. That is the
/// shape of the 2026-08-02 field report. Adding an end reason here means a
/// future one cannot be added without the match below failing to compile.
enum StreamEnd {
    /// The runner sent a terminal event; it is already forwarded.
    RunnerFinished,
    /// The runner's sender dropped with no terminal event, so the run died.
    ///
    /// SHOULD NOW BE UNREACHABLE: the runner arms a drop guard that emits
    /// `ValidationEvent::Aborted` when its thread unwinds, so a dead run ends
    /// the stream with a terminal event of its own and exits through
    /// `RunnerFinished` above. Retained anyway, deliberately: this arm is the
    /// last line of defence against a frontend that waits forever, and the
    /// cost of keeping it is one unreachable branch, whereas the cost of
    /// removing it is the 2026-08-02 hang returning silently if the guard ever
    /// regresses. Belt and braces on a failure mode with no other detector.
    RunnerVanished,
    /// The frontend receiver was dropped (window closed, run superseded).
    /// Nobody is listening, so there is nothing to report.
    FrontendGone,
}

/// Whether this event ends the run, so the bridge should stop forwarding.
///
/// Exhaustive by design: a new terminal event added upstream must be
/// classified here, or this fails to compile. Getting it wrong in the
/// forgiving direction (treating a terminal event as non-terminal) leaves the
/// bridge waiting for a channel close it will then report as a vanished
/// runner, which is exactly the duplicate-report confusion this avoids.
fn is_terminal(event: &FrontendEvent) -> bool {
    match event {
        FrontendEvent::Finished { .. }
        | FrontendEvent::FinishedIncomplete { .. }
        | FrontendEvent::Aborted { .. } => true,
        FrontendEvent::Discovering
        | FrontendEvent::Started { .. }
        | FrontendEvent::Errors { .. }
        | FrontendEvent::FileComplete { .. } => false,
    }
}

fn bridge_validation_events(
    validation_rx: Receiver<ValidationEvent>,
    root: PathBuf,
) -> Receiver<FrontendEvent> {
    let (frontend_tx, frontend_rx) = unbounded();

    std::thread::spawn(move || {
        let end = loop {
            let Ok(event) = validation_rx.recv() else {
                break StreamEnd::RunnerVanished;
            };
            let Some(frontend_event) = to_frontend_event(event, &root) else {
                continue;
            };
            // Read the outcome off `frontend_event`, the thing actually about
            // to be sent, rather than off the pre-mapping `event`. Today every
            // terminal event maps to `Some`, so this is latent, but the
            // ordering matters: if a future mapping ever stopped translating a
            // terminal event to a frontend event, computing this from `event`
            // would silently swallow it and the frontend would sit waiting
            // forever, exactly the 2026-08-02 hang this bridge exists to
            // prevent. Deriving it from what was actually delivered means
            // that failure mode instead falls through to `RunnerVanished`
            // below, once the channel closes, which is the correct answer:
            // the frontend was never told how the run ended.
            let terminal = is_terminal(&frontend_event);
            if frontend_tx.send(frontend_event).is_err() {
                break StreamEnd::FrontendGone;
            }
            if terminal {
                break StreamEnd::RunnerFinished;
            }
        };

        // Exhaustive on purpose: every way this stream can end must decide
        // what the frontend is told, and "tell it nothing" has to be a
        // deliberate arm rather than a fall-through.
        match end {
            StreamEnd::RunnerFinished | StreamEnd::FrontendGone => {}
            StreamEnd::RunnerVanished => {
                let _ = frontend_tx.send(FrontendEvent::Aborted {
                    reason: "The validator stopped without finishing. \
                             No results were produced for this run."
                        .to_owned(),
                });
            }
        }
    });

    frontend_rx
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crossbeam_channel::unbounded as unbounded_channel;

    /// A validation run that dies without finishing must still terminate the
    /// event stream with something the UI can act on.
    ///
    /// This is the shape of the 2026-08-02 field report ("doesn't seem to go
    /// beyond the Discovering files step"): the bridge's receive loop ended on
    /// channel disconnect and emitted nothing, so a run whose worker thread
    /// panicked, or otherwise dropped its sender before `Finished`, left the
    /// frontend waiting forever with no error and no completion. The user
    /// cannot distinguish that from a slow run, and neither could the UI.
    #[test]
    fn a_run_that_dies_before_finishing_still_terminates_the_stream() {
        let (validation_tx, validation_rx) = unbounded_channel();
        let frontend_rx = bridge_validation_events(validation_rx, PathBuf::from("/corpus"));

        // The runner announces itself, then dies: exactly what a panicking
        // worker thread looks like from this side of the channel.
        validation_tx
            .send(talkbank_transform::ValidationEvent::Discovering)
            .unwrap();
        drop(validation_tx);

        let events: Vec<FrontendEvent> = frontend_rx.into_iter().collect();

        assert!(
            matches!(events.first(), Some(FrontendEvent::Discovering)),
            "the discovering event should still be forwarded, got {events:?}"
        );
        assert!(
            matches!(events.last(), Some(FrontendEvent::Aborted { .. })),
            "a stream that ends without Finished must emit Aborted so the UI \
             stops waiting; got {events:?}"
        );
    }

    /// The normal path must NOT report an abort: `Finished` is a clean end.
    #[test]
    fn a_run_that_finishes_normally_reports_no_abort() {
        let (validation_tx, validation_rx) = unbounded_channel();
        let frontend_rx = bridge_validation_events(validation_rx, PathBuf::from("/corpus"));

        validation_tx
            .send(talkbank_transform::ValidationEvent::Discovering)
            .unwrap();
        validation_tx
            .send(talkbank_transform::ValidationEvent::Finished(
                talkbank_transform::ValidationStatsSnapshot {
                    total_files: 0,
                    valid_files: 0,
                    invalid_files: 0,
                    cache_hits: 0,
                    cache_misses: 0,
                    parse_errors: 0,
                    roundtrip_passed: 0,
                    roundtrip_failed: 0,
                    cancelled: false,
                },
            ))
            .unwrap();
        drop(validation_tx);

        let events: Vec<FrontendEvent> = frontend_rx.into_iter().collect();

        assert!(
            events
                .iter()
                .all(|e| !matches!(e, FrontendEvent::Aborted { .. })),
            "a clean run must not report an abort; got {events:?}"
        );
        assert!(
            matches!(events.last(), Some(FrontendEvent::Finished { .. })),
            "last event should be Finished, got {events:?}"
        );
    }
}
