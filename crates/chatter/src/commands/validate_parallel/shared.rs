//! The typed outcome every `chatter validate` run reports back to its
//! caller, and the wording of a stop that every surface shares.

use std::num::NonZeroUsize;

use talkbank_transform::validation_runner::{
    CancelReason, LossCause, RunEnding, ValidationStatsSnapshot,
};

/// How far the run an interactive session showed had got when the session
/// closed: still running (the user quit first), or ended, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunPhase {
    /// No ending has arrived yet.
    Running,
    /// The run ended this way.
    Ended(RunEnding),
}

impl RunPhase {
    /// Whether the run has ended, by any of its endings: the affordances
    /// every ended run offers (Rerun, a still progress bar).
    pub fn is_ended(&self) -> bool {
        match self {
            Self::Running => false,
            Self::Ended(_) => true,
        }
    }
}

/// How an interactive (TUI) session ended. The TUI has already SHOWN the
/// run's ending; this carries it so the exit status agrees with what was
/// shown.
#[derive(Debug)]
pub enum InteractiveEnd {
    /// The user closed the session, with the run where it was.
    Closed(RunPhase),
    /// The terminal interface itself failed (its error already printed);
    /// nothing reliable was shown.
    TerminalFailed,
}

/// A fact about the run, said before it starts, in the renderer's channel:
/// never a stray stderr line that would break JSON mode's empty stderr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunNotice {
    /// These codes are suppressed (`--suppress`).
    Suppressing(crate::commands::error_codes::SuppressedCodes),
    /// `--check-xphon` was passed; it is deprecated and does nothing.
    CheckXphonIgnored,
    /// The Ctrl-C handler could not be installed, so an interrupt ends the
    /// process without the run's own stop being reported.
    InterruptUnavailable(String),
}

impl RunNotice {
    /// The notices a run's flags call for: the deprecated `--check-xphon`,
    /// and what `--suppress` hides (named, because a suppressed code is
    /// never emitted, so no count of what it hid exists afterwards).
    pub(crate) fn for_flags(
        check_xphon: bool,
        suppressed: &crate::commands::error_codes::SuppressedCodes,
    ) -> Vec<Self> {
        let deprecated = check_xphon.then_some(Self::CheckXphonIgnored);
        let suppressing = (!suppressed.is_empty()).then(|| Self::Suppressing(suppressed.clone()));
        deprecated.into_iter().chain(suppressing).collect()
    }

    /// The note as a sentence for a terminal.
    pub(crate) fn sentence(&self) -> String {
        match self {
            Self::Suppressing(codes) => format!(
                "note: suppressing {} code(s): {}",
                codes.codes().len(),
                codes
                    .codes()
                    .iter()
                    .map(|code| code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::CheckXphonIgnored => "note: --check-xphon is deprecated and has no effect; \
                 Phon %x validation now runs by default (use --suppress xphon to silence it)"
                .to_owned(),
            Self::InterruptUnavailable(error) => {
                format!("note: Ctrl-C cannot stop this run cleanly: {error}")
            }
        }
    }
}

/// Whether the Ctrl-C handler may say "Cancelling..." on stderr. A
/// renderer's answer, since it alone knows its channel: JSON mode keeps
/// stderr empty, and the stop itself is reported as a `stop` record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptReport {
    /// Say it on stderr (a terminal surface).
    OnStderr,
    /// Say nothing; the run's own ending reports the stop.
    Silent,
}

/// How one `chatter validate` run ended.
///
/// The exit status is decided from the run's [`RunEnding`], through
/// [`RunEnding::passed`], on every surface: a snapshot alone would invite
/// `if invalid > 0 { exit(1) }`, which answers 0 for a run that lost files
/// or validated nothing.
#[derive(Debug)]
pub enum ValidationOutcome {
    /// A streamed run (text, JSON or audit): how it ended, and whether its
    /// output is whole.
    Streamed {
        /// How the run ended.
        ending: RunEnding,
        /// Whether the renderer wrote everything it promised.
        output: super::renderer::RenderedOutput,
    },
    /// An interactive session and how it closed.
    Interactive(InteractiveEnd),
    /// The `--audit` output file could not be created, so nothing ran.
    AuditFileUnwritable {
        /// The path asked for.
        path: std::path::PathBuf,
        /// Why it could not be created.
        error: std::io::Error,
    },
}

impl ValidationOutcome {
    /// Whether the command must exit unsuccessfully: whenever the run did
    /// not pass, an interactive session included, or its output (an audit
    /// file, JSON on stdout) is incomplete. A session closed before its run ended vouches for
    /// nothing.
    pub fn failed(&self) -> bool {
        use super::renderer::RenderedOutput;
        match self {
            Self::Streamed {
                output: RenderedOutput::AuditFileIncomplete | RenderedOutput::StdoutIncomplete,
                ..
            } => true,
            Self::Streamed {
                ending,
                output: RenderedOutput::Complete,
            }
            | Self::Interactive(InteractiveEnd::Closed(RunPhase::Ended(ending))) => {
                !ending.passed()
            }
            Self::Interactive(InteractiveEnd::Closed(RunPhase::Running)) => true,
            Self::Interactive(InteractiveEnd::TerminalFailed) => true,
            Self::AuditFileUnwritable { .. } => true,
        }
    }
}

/// The full stop sentence for a terminal: the reason and what it left.
pub(super) fn stop_sentence(reason: CancelReason, unprocessed: NonZeroUsize) -> String {
    format!("{reason}; {unprocessed} file(s) were not validated.")
}

/// The terminal sentence for a run that lost files: how many, why, and that
/// the counts after it are not totals for the input.
pub(super) fn incomplete_sentence(
    stats: &ValidationStatsSnapshot,
    lost_files: NonZeroUsize,
    cause: &LossCause,
) -> String {
    format!(
        "Error: validation did not cover {lost_files} of {} discovered file(s): {cause} \
         The counts below describe only what was processed.",
        stats.total_files()
    )
}

/// The terminal sentence for a run that found nothing to validate.
pub(super) fn nothing_found_sentence(label: &std::path::Path) -> String {
    format!("Error: no .cha files found in {}", label.display())
}

#[cfg(test)]
mod outcome_tests {
    use super::*;
    use talkbank_transform::validation_runner::AbortReason;

    /// The interactive exit status follows the run the session showed: a
    /// session closed on a failed or unfinished run fails, as the streamed
    /// run would; only a session whose run passed succeeds.
    #[test]
    fn an_interactive_session_fails_unless_its_run_passed() {
        for phase in [
            RunPhase::Running,
            RunPhase::Ended(RunEnding::NothingFound),
            RunPhase::Ended(RunEnding::Aborted(AbortReason::NoEnding)),
        ] {
            assert!(ValidationOutcome::Interactive(InteractiveEnd::Closed(phase)).failed());
        }
        assert!(ValidationOutcome::Interactive(InteractiveEnd::TerminalFailed).failed());
        assert!(
            ValidationOutcome::Streamed {
                ending: RunEnding::NothingFound,
                output: super::super::renderer::RenderedOutput::Complete,
            }
            .failed()
        );
    }
}
