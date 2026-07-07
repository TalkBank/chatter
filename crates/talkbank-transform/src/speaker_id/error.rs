//! Domain errors for the speaker-id pipeline.

use talkbank_model::SpeakerCode;

use crate::PipelineError;

use super::identify::DonorMatchReport;
use super::types::ConfidenceThreshold;

/// Errors that can arise from the speaker-id operation.
///
/// `Parse` → CLI exit 1 (invalid input); `LowConfidence` → CLI exit
/// 4 (operator must adjudicate); every other variant → CLI exit 2
/// (precondition violation). The CLI layer is responsible for the
/// mapping; `SpeakerIdError` itself just classifies the failure
/// mode.
#[derive(Debug, thiserror::Error)]
pub enum SpeakerIdError {
    /// The mapping spec couldn't be parsed. The free-form message
    /// names which assignment failed so the operator can correct it
    /// without consulting the grammar.
    #[error("invalid --mapping spec: {0}")]
    InvalidMappingSpec(String),

    /// Reference mode: the reference transcript has no utterances for
    /// the requested anchor speaker, so there is no token bag to
    /// compare donor speakers against.
    #[error("reference transcript has no utterances for anchor speaker {anchor}")]
    ReferenceMissingAnchor {
        /// The anchor speaker code searched for in the reference.
        anchor: SpeakerCode,
    },

    /// Reference mode: the donor has fewer than two distinct speakers,
    /// so there is nothing for the multiset-Jaccard step to choose
    /// between. The operator should use explicit-mapping mode for
    /// single-speaker donors.
    #[error(
        "donor has only {} distinct speaker(s) ({speakers:?}); reference mode \
         needs at least 2 to discriminate",
        speakers.len()
    )]
    DonorTooFewSpeakers {
        /// The donor speaker codes encountered (in document order).
        speakers: Vec<SpeakerCode>,
    },

    /// Reference mode: the winner→runner-up Jaccard margin is below
    /// the supplied confidence threshold, so the auto-decision is
    /// refused. The operator inspects the per-speaker scores and
    /// resolves by lowering the threshold, supplying explicit
    /// `--mapping`, or loading a saved override.
    #[error(
        "speaker-id below confidence threshold (margin {margin}, threshold {threshold}); \
         scores={scores:?}",
        margin = report.margin,
        scores = report.scores,
    )]
    LowConfidence {
        /// The full match report, the winner the algorithm
        /// *would* have picked, the per-speaker scores, and the
        /// margin. Operators inspecting a low-confidence refusal
        /// (or `--write-pending` recording the would-have-been
        /// decision) need every field.
        report: DonorMatchReport,
        /// The threshold the call was made with, echoed verbatim.
        threshold: ConfidenceThreshold,
    },

    /// Override-file replay: the requested session ID is not present
    /// in the override file. The available IDs are surfaced so the
    /// operator can correct the spelling without re-reading the
    /// file by hand.
    #[error("override-file has no entry for session_id {session_id:?}; available: {available:?}")]
    SessionIdNotFound {
        /// The session ID the operator requested.
        session_id: String,
        /// Session IDs actually present in the override file (in
        /// alphabetical order).
        available: Vec<String>,
    },

    /// Override-file replay: an entry recorded a `Rename` action for a
    /// speaker with no matching `adult_roles` entry, so there is no CHAT
    /// identity to rename it to. Only reachable via a hand-corrupted
    /// override file; the sanctioned writer paths always cover every
    /// `Rename`. Reported (rather than panicking) so a bad file fails
    /// closed with a diagnostic instead of crashing.
    #[error(
        "override entry renames speaker {speaker} but has no adult_roles entry for it; \
         the file is internally inconsistent (hand-edited?)"
    )]
    OverrideRenameMissingRole {
        /// The speaker code whose `Rename` action had no role entry.
        speaker: SpeakerCode,
    },

    /// Underlying parse error from the input file.
    #[error("parse error: {0}")]
    Parse(#[from] PipelineError),
}
