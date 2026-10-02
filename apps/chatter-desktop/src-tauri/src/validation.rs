//! Desktop validation orchestration for a single selected target.
//!
//! Chatter's desktop contract is one target at a time:
//! - one `.cha` file
//! - or one directory
//!
//! Both cases go through the shared streaming runner the CLI uses: a
//! directory through `validate_directory_streaming`, a file through
//! `validate_files_streaming` (the CLI's `validate_arguments_streaming` feeds
//! the same run body), with a real on-disk cache. Desktop must not
//! reimplement cache lookups, stats accounting, or per-file rule dispatch;
//! see `apps/chatter-desktop/AGENTS.md` ("No desktop-local domain logic").

use std::path::PathBuf;
use std::sync::Arc;

use crate::errors::TargetError;
use crossbeam_channel::{Receiver, unbounded};
use talkbank_transform::UnifiedCache;
use talkbank_transform::validation_runner::{
    AbortReason, ParserKind, ValidationConfig, ValidationEvent, ValidationRun,
    is_chat_transcript_path, validate_directory_streaming, validate_files_streaming,
};

use crate::events::{FrontendEvent, to_frontend_events};
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
        // `strict_linkers` selects RULES (it turns on the opt-in
        // cross-utterance linker checks), so it lives
        // in the rule selection, which is also what keys the cache. The desktop
        // request carries no `--suppress` equivalent yet, so the presentation
        // policy stays the default: show everything the validator computed.
        //
        // The wire carries the checkbox as a boolean; this is its one
        // translation into the linker mode.
        let rules =
            talkbank_model::RuleSelection::new().with_linkers(match request.strict_linkers {
                true => talkbank_model::LinkerChecks::Strict,
                false => talkbank_model::LinkerChecks::Lenient,
            });
        Self {
            // The wire carries the checkbox as a boolean; this is its one
            // translation into the runner's mode.
            roundtrip: match request.roundtrip {
                true => talkbank_transform::RoundtripCheck::Run,
                false => talkbank_transform::RoundtripCheck::Skip,
            },
            parser_kind: request.parser_kind.into(),
            rules,
            jobs: request.jobs,
            ..Self::default()
        }
    }
}

/// Start validation for a single desktop target with `run`: the request's
/// configuration (roundtrip, parser kind, strict linkers, jobs) bound to the
/// cache opened for it. The cache is opened by the caller, not here, so the
/// app memoizes one pool per identity (`ValidationState::cache_for_config`)
/// and reuses it across every validate/re-validate call instead of paying
/// SQLite-pool setup cost per run; the binding refuses a pool opened for
/// another identity.
pub fn validate_target_streaming_with_config(
    target: PathBuf,
    run: &ValidationRun,
) -> Result<(Receiver<FrontendEvent>, talkbank_transform::Canceller), TargetError> {
    if !target.exists() {
        return Err(TargetError::Missing { path: target });
    }

    if target.is_dir() {
        let (validation_rx, canceller) = validate_directory_streaming(&target, run);
        Ok((bridge_validation_events(validation_rx), canceller))
    } else if target.is_file() {
        if !is_chat_transcript_path(&target) {
            return Err(TargetError::NotChatTranscript { path: target });
        }
        let (validation_rx, canceller) = validate_files_streaming(vec![target], run);
        Ok((bridge_validation_events(validation_rx), canceller))
    } else {
        Err(TargetError::NotFileOrDirectory { path: target })
    }
}

/// Construct the shared on-disk validation cache, the exact same construction
/// the CLI uses (`crates/chatter/src/commands/validate/cache.rs`), minus the
/// `--force`-clear step the desktop has no flag for. Zero CLI dependency:
/// `UnifiedCache::new(identity)` resolves the OS cache dir on its own.
///
/// Open the default cache for the request's complete validation identity.
/// A cache that will not open is an error the caller shows the user (the run
/// goes on without it), never a line on a GUI app's invisible stderr.
pub fn initialize_cache(
    identity: talkbank_transform::CacheIdentity,
) -> Result<Arc<UnifiedCache>, talkbank_transform::CacheError> {
    UnifiedCache::new(identity).map(Arc::new)
}

/// Open an isolated cache directory for the same complete request identity.
pub fn initialize_cache_at(
    cache_dir: PathBuf,
    identity: talkbank_transform::CacheIdentity,
) -> Result<Arc<UnifiedCache>, talkbank_transform::CacheError> {
    UnifiedCache::with_directory(cache_dir, identity).map(Arc::new)
}

/// Why the bridge stopped forwarding events: a value, matched exhaustively
/// once, rather than something each `break` is trusted to handle.
enum StreamEnd {
    /// The runner sent its ending; it is already forwarded.
    RunnerFinished,
    /// The runner's sender dropped with no ending. The runner's drop guard
    /// makes this unreachable; the frontend is still told, as an abort,
    /// so a regression of the guard can never leave it waiting forever.
    RunnerVanished,
    /// The frontend receiver was dropped (window closed, run superseded).
    /// Nobody is listening, so there is nothing to report.
    FrontendGone,
}

/// Whether this event ends the run, so the bridge should stop forwarding.
///
/// Read off the event actually sent, and exhaustive: a new ending added
/// upstream must be classified here, or this fails to compile.
fn is_terminal(event: &FrontendEvent) -> bool {
    match event {
        FrontendEvent::Finished { .. }
        | FrontendEvent::NothingFound
        | FrontendEvent::Stopped { .. }
        | FrontendEvent::FinishedIncomplete { .. }
        | FrontendEvent::Aborted { .. } => true,
        FrontendEvent::CacheUnavailable { .. }
        | FrontendEvent::Discovering
        | FrontendEvent::Started { .. }
        | FrontendEvent::Errors { .. }
        | FrontendEvent::FileComplete { .. } => false,
    }
}

fn bridge_validation_events(validation_rx: Receiver<ValidationEvent>) -> Receiver<FrontendEvent> {
    let (frontend_tx, frontend_rx) = unbounded();

    std::thread::spawn(move || {
        let end = 'stream: loop {
            let Ok(event) = validation_rx.recv() else {
                break StreamEnd::RunnerVanished;
            };
            for frontend_event in to_frontend_events(event).into_iter().flatten() {
                let terminal = is_terminal(&frontend_event);
                if frontend_tx.send(frontend_event).is_err() {
                    break 'stream StreamEnd::FrontendGone;
                }
                if terminal {
                    break 'stream StreamEnd::RunnerFinished;
                }
            }
        };

        // Exhaustive on purpose: every way this stream can end decides what
        // the frontend is told.
        match end {
            StreamEnd::RunnerFinished | StreamEnd::FrontendGone => {}
            StreamEnd::RunnerVanished => {
                let _ = frontend_tx.send(FrontendEvent::Aborted {
                    reason: AbortReason::NoEnding.to_string(),
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

    /// A validation run that dies without finishing still terminates the
    /// event stream, with an abort the UI can act on, so the frontend is
    /// never left waiting on a dead run.
    #[test]
    fn a_run_that_dies_before_finishing_still_terminates_the_stream() {
        let (validation_tx, validation_rx) = unbounded_channel();
        let frontend_rx = bridge_validation_events(validation_rx);

        // The runner announces itself, then dies.
        validation_tx.send(ValidationEvent::Discovering).unwrap();
        drop(validation_tx);

        let events: Vec<FrontendEvent> = frontend_rx.into_iter().collect();

        assert!(
            matches!(events.first(), Some(FrontendEvent::Discovering)),
            "the discovering event should still be forwarded, got {events:?}"
        );
        assert!(
            matches!(events.last(), Some(FrontendEvent::Aborted { .. })),
            "a stream that ends without an ending must emit Aborted so the UI \
             stops waiting; got {events:?}"
        );
    }

    /// A run's own ending ends the stream with no abort after it.
    #[test]
    fn a_run_that_ends_normally_reports_no_abort() {
        let (validation_tx, validation_rx) = unbounded_channel();
        let frontend_rx = bridge_validation_events(validation_rx);

        validation_tx.send(ValidationEvent::Discovering).unwrap();
        validation_tx
            .send(ValidationEvent::Finished(
                talkbank_transform::RunEnding::NothingFound,
            ))
            .unwrap();
        drop(validation_tx);

        let events: Vec<FrontendEvent> = frontend_rx.into_iter().collect();

        assert!(
            matches!(
                &events[..],
                [FrontendEvent::Discovering, FrontendEvent::NothingFound]
            ),
            "the ending is the last event, with no abort; got {events:?}"
        );
    }
}
