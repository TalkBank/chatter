//! Internal support helpers for `chatter speaker-id`: session-ID
//! derivation, role-spec parsing, and the contract-defined
//! reported failure classifiers for each typed error.

use std::path::Path;
use tracing::warn;

use crate::exit_codes::ReportedFailure;
use talkbank_model::{ParticipantRole, SpeakerCode};
use talkbank_transform::adjudication::AdjudicationError;
use talkbank_transform::speaker_id::{OverrideFileError, SpeakerIdError};

/// Default session ID: the input file's basename stem
/// (`donor.cha` → `donor`, `s12-t1.cha` → `s12-t1`). Falls back to
/// the full file name when the stem can't be derived.
pub(crate) fn derive_session_id(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| input.display().to_string())
}

/// Parse a single `CODE:ROLE` inserted-role spec into its typed
/// components. Accepts exactly one pair; multi-role specs
/// (`CODE:ROLE,CODE2:ROLE2`) are not yet supported.
pub(super) fn parse_inserted_role(spec: &str) -> Result<(SpeakerCode, ParticipantRole), String> {
    let trimmed = spec.trim();
    let (code, role) = trimmed.split_once(':').ok_or_else(|| {
        format!("--inserted-role must be in the form CODE:ROLE, got: {trimmed:?}")
    })?;
    let code = code.trim();
    let role = role.trim();
    if code.is_empty() || role.is_empty() {
        return Err(format!(
            "--inserted-role components must be non-empty (got CODE={code:?}, ROLE={role:?})"
        ));
    }
    Ok((SpeakerCode::new(code), ParticipantRole::new(role)))
}

/// Render a `SpeakerIdError` to stderr and return
/// the contract-defined exit code. `LowConfidence` (reference-mode
/// adjudication required) is the only variant that exits 4; every
/// other precondition violation exits 2; parse errors exit 1.
pub(super) fn report_speaker_id_error(e: SpeakerIdError) -> crate::exit_codes::CommandStop {
    warn!("speaker-id failed: {}", e);
    eprintln!("Error: {}", e);
    let code = match e {
        SpeakerIdError::SpeakerRewrite(_)
        | SpeakerIdError::InvalidAssignment(_)
        | SpeakerIdError::InvalidMappingSpec(_)
        | SpeakerIdError::ReferenceMissingAnchor { .. }
        | SpeakerIdError::DonorTooFewSpeakers { .. }
        | SpeakerIdError::SessionIdNotFound { .. }
        | SpeakerIdError::OverrideRenameMissingRole { .. } => ReportedFailure::Precondition,
        SpeakerIdError::LowConfidence { .. } => ReportedFailure::NeedsReview,
        SpeakerIdError::Parse(_) => ReportedFailure::Input,
    };
    crate::exit_codes::CommandStop::Reported(code)
}

/// Render an `OverrideFileError` to stderr. I/O / TOML failures exit
/// 1 (treated as input issues); unsupported schema versions exit 2
/// (operator must upgrade or re-adjudicate).
pub(crate) fn report_override_file_error(
    path: &Path,
    e: OverrideFileError,
) -> crate::exit_codes::CommandStop {
    warn!(
        "override-file operation on {} failed: {}",
        path.display(),
        e
    );
    eprintln!("Error: override-file {}: {}", path.display(), e);
    let code = match e {
        OverrideFileError::UnsupportedSchemaVersion { .. } => ReportedFailure::Precondition,
        OverrideFileError::Io(_) | OverrideFileError::Toml(_) => ReportedFailure::Input,
    };
    crate::exit_codes::CommandStop::Reported(code)
}

/// Render an `AdjudicationError` (from pending-file I/O on the
/// `--write-pending` path) to stderr. `Io` / `Toml` exit 1; other
/// variants are not reachable from this seam.
pub(super) fn report_adjudication_error(
    path: &Path,
    e: AdjudicationError,
) -> crate::exit_codes::CommandStop {
    warn!("pending-file operation on {} failed: {}", path.display(), e);
    eprintln!("Error: pending-file {}: {}", path.display(), e);
    crate::exit_codes::ReportedFailure::Input.into()
}
