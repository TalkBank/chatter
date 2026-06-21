//! Side-effect writers for `chatter speaker-id` reference mode:
//! the `--write-override` audit entry (auto-decision path) and the
//! `--write-pending` adjudication entry (low-confidence path).

use std::path::Path;
use tracing::{info, warn};

use chrono::Utc;
use talkbank_model::model::ChatFile;
use talkbank_model::{ParticipantRole, SpeakerCode};
use talkbank_transform::adjudication::{
    PendingAdjudications, PendingEntry, PendingKindData, SuggestedSpeakerIdMapping,
};
use talkbank_transform::speaker_id::{
    ConfidenceThreshold, DonorMatchReport, InsertedRoleSpec, MergeOverride, OverrideFile,
    SpeakerAction,
};

use super::modes::ReferenceModeOutcome;
use super::support::{
    derive_session_id, exit_with_adjudication_error, exit_with_override_file_error,
};

/// Append (or update) an entry for the current session in the
/// override file. The session ID defaults to the input CHAT file's
/// basename stem; the operator defaults to `$USER` (`"unknown"` if
/// unset). `decided_at` is the current UTC time.
pub(crate) fn write_override_entry(path: &Path, input: &Path, outcome: &ReferenceModeOutcome) {
    let session_id = derive_session_id(input);
    let operator = std::env::var("USER").unwrap_or_else(|_| {
        warn!(
            "$USER environment variable is unset; override-file entry will record operator as \
             \"unknown\", set USER (or run via the deploy harness that sets it) to preserve \
             audit-trail attribution"
        );
        "unknown".to_string()
    });
    let entry = MergeOverride::auto_decision(
        &outcome.mapping,
        &outcome.report,
        InsertedRoleSpec::new(&outcome.inserted_code, &outcome.inserted_role_tag),
        operator,
        Utc::now(),
    );
    let mut file = match OverrideFile::read_or_default(path) {
        Ok(f) => f,
        Err(e) => exit_with_override_file_error(path, e),
    };
    file.upsert(session_id, entry);
    if let Err(e) = file.write(path) {
        exit_with_override_file_error(path, e);
    }
    info!("appended override entry to: {}", path.display());
}

/// Append a pending-adjudication entry for the current session to
/// the pending file. The entry's `suggested` field carries the
/// would-have-been mapping (winner → drop, others → rename to
/// `inserted_code`/`inserted_role`) so `chatter adjudicate` can
/// surface an "accept suggested?" prompt without re-running the
/// Jaccard pass.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_pending_entry(
    pending_path: &Path,
    input: &Path,
    report: &DonorMatchReport,
    threshold: ConfidenceThreshold,
    donor_chat: &ChatFile,
    inserted_code: &SpeakerCode,
    inserted_role_tag: &ParticipantRole,
) {
    let session_id = derive_session_id(input);
    let mut suggested_mapping: std::collections::BTreeMap<String, SpeakerAction> =
        std::collections::BTreeMap::new();
    suggested_mapping.insert(report.winner.as_str().to_string(), SpeakerAction::Drop);
    for spk in donor_chat.unique_utterance_speakers() {
        if spk != report.winner {
            suggested_mapping.insert(spk.as_str().to_string(), SpeakerAction::Rename);
        }
    }
    let entry = PendingEntry {
        session_id: session_id.clone(),
        created_at: Utc::now(),
        data: PendingKindData::SpeakerIdLowConfidence {
            suggested: SuggestedSpeakerIdMapping {
                mapping: suggested_mapping,
                inserted_role: InsertedRoleSpec::new(inserted_code, inserted_role_tag),
            },
        },
        scores: report.scores_to_serializable(),
        margin: report.margin_to_serializable(),
        threshold_used: Some(threshold.0),
        engine: talkbank_transform::speaker_id::DecisionEngine::Deterministic,
        judgment: None,
    };
    append_pending_entry(pending_path, entry);
}

/// Append an already-built `PendingEntry` to the pending file: read (or
/// default), push, and write back. Both the deterministic low-confidence
/// path ([`write_pending_entry`]) and the holistic-LLM path
/// (`modes::run_holistic_mode`) funnel through here so the
/// read/push/write append semantics live in exactly one place. On any
/// pending-file I/O / TOML failure this exits via
/// [`exit_with_adjudication_error`] (exit code 1) rather than panicking.
pub(super) fn append_pending_entry(pending_path: &Path, entry: PendingEntry) {
    let mut file = match PendingAdjudications::read_or_default(pending_path) {
        Ok(f) => f,
        Err(e) => exit_with_adjudication_error(pending_path, e),
    };
    file.entries.push(entry);
    if let Err(e) = file.write(pending_path) {
        exit_with_adjudication_error(pending_path, e);
    }
    info!("appended pending entry to: {}", pending_path.display());
}
