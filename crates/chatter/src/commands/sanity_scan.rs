//! `chatter sanity-scan`, post-merge misclassification detector.
//!
//! Thin CLI shim over [`talkbank_transform::sanity_scan`]. For each
//! auto-decided session in the override file, look up the merged
//! CHAT file by basename, run the heuristic, and (if flagged)
//! append a `sanity-scan-misclassification` pending entry to the
//! pending file. Exit 4 if any session was flagged so operator
//! tooling can short-circuit.

use std::fs;
use std::io::Write;
use std::path::Path;
use tracing::{Level, info, span, warn};

use talkbank_model::{ParseValidateOptions, SpeakerCode};
use talkbank_transform::adjudication::{
    PendingAdjudications, PendingEntry, PendingKindData, SuggestedSpeakerIdMapping,
};
use talkbank_transform::parse_and_validate;
use talkbank_transform::recorded_time::RecordedTime;
use talkbank_transform::report_destination::{ReportInputs, StagedReportDestination};
use talkbank_transform::sanity_scan::{SanityScanThreshold, scan_session};
use talkbank_transform::speaker_id::OverrideFile;

/// A complete source population and a pending destination distinct from it.
/// Missing decision sources or selected transcripts cannot become empty scans.
struct ScanInputs {
    overrides: OverrideFile,
    pending: PendingAdjudications,
    destination: StagedReportDestination,
}

fn report_input_error(error: impl std::fmt::Display) -> crate::exit_codes::CommandStop {
    eprintln!("Error preparing scan: {error}");
    crate::exit_codes::ReportedFailure::Input.into()
}

impl ScanInputs {
    fn admit(
        directory: &Path,
        overrides: &Path,
        pending: &Path,
    ) -> crate::exit_codes::CommandResult<Self> {
        let metadata = fs::metadata(directory).map_err(report_input_error)?;
        if !metadata.is_dir() {
            return Err(report_input_error(format!(
                "{} is not a CHAT directory",
                directory.display()
            )));
        }
        let decisions = OverrideFile::read(overrides).map_err(report_input_error)?;
        let transcripts: Vec<_> = decisions
            .auto_entries()
            .map(|(session, _)| directory.join(format!("{session}.cha")))
            .collect();
        let sources = ReportInputs::discover(
            std::iter::once(overrides).chain(transcripts.iter().map(|path| path.as_path())),
        )
        .map_err(report_input_error)?;
        let destination = sources
            .staged_destination(pending)
            .map_err(report_input_error)?;
        let pending = PendingAdjudications::read_or_default(pending).map_err(report_input_error)?;
        Ok(Self {
            overrides: decisions,
            pending,
            destination,
        })
    }
}

/// Top-level entry for `chatter sanity-scan`.
///
/// Exit-code contract:
/// - 0: scan completed; no sessions flagged.
/// - 1: I/O or parse error on a merged file or the override file.
/// - 4: scan completed; at least one session was flagged and a
///   pending entry written. Mirrors the speaker-id-low-confidence
///   exit code so operator-driven re-runs can short-circuit on
///   non-zero.
pub fn run_sanity_scan(
    merged_dir: &Path,
    override_path: &Path,
    anchor: &str,
    threshold: SanityScanThreshold,
    write_pending_path: &Path,
) -> crate::exit_codes::CommandResult {
    let _span = span!(
        Level::INFO,
        "chatter_sanity_scan",
        merged_dir = %merged_dir.display(),
        override_file = %override_path.display(),
    )
    .entered();

    let ScanInputs {
        overrides: override_file,
        mut pending,
        destination,
    } = ScanInputs::admit(merged_dir, override_path, write_pending_path)?;

    let anchor_code = SpeakerCode::new(anchor);
    let options = ParseValidateOptions::default();

    let mut flagged: u32 = 0;
    let mut scanned: u32 = 0;

    for (session_id, entry) in override_file.auto_entries() {
        let merged_path = merged_dir.join(format!("{session_id}.cha"));
        scanned += 1;
        let merged_content = match fs::read_to_string(&merged_path) {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "failed to read merged file {}: {}",
                    merged_path.display(),
                    e
                );
                eprintln!("Error reading {}: {}", merged_path.display(), e);
                return Err(crate::exit_codes::ReportedFailure::Input.into());
            }
        };
        let merged_chat = match parse_and_validate(&merged_content, options.clone()) {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "merged file parse failed for session {:?}: {}",
                    session_id, e
                );
                eprintln!("Error parsing {}: {}", merged_path.display(), e);
                return Err(crate::exit_codes::ReportedFailure::Input.into());
            }
        };
        if let Some(flag) = scan_session(&merged_chat, entry, &anchor_code, threshold) {
            info!("⚠ flagged: {} ({})", session_id, flag.reason);
            pending.entries.push(PendingEntry {
                session_id: session_id.to_string(),
                created_at: RecordedTime::now(),
                data: PendingKindData::SanityScanMisclassification {
                    suggested: SuggestedSpeakerIdMapping {
                        mapping: flag.suggested_mapping,
                        adult_roles: flag.suggested_adult_roles,
                    },
                    reason: flag.reason,
                },
                // Sanity-scan has no Jaccard inputs, scores+margin
                // intentionally empty/None.
                scores: std::collections::BTreeMap::new(),
                margin: None,
                threshold_used: Some(threshold.value()),
                engine: talkbank_transform::speaker_id::DecisionEngine::Deterministic,
                judgment: None,
            });
            flagged += 1;
        } else {
            info!("✓ ok: {}", session_id);
        }
    }

    if flagged > 0 {
        let text = toml::to_string_pretty(&pending).map_err(report_input_error)?;
        let mut writer = destination.into_writer().map_err(report_input_error)?;
        writer
            .write_all(text.as_bytes())
            .map_err(report_input_error)?;
        writer
            .finish()
            .and_then(|publication| publication.publish())
            .map_err(report_input_error)?;
        info!(
            "appended {} pending entries to: {}",
            flagged,
            write_pending_path.display()
        );
    }

    outln!(
        "sanity-scan summary: {scanned} auto-decided session(s) scanned; \
         {flagged} flagged for adjudication"
    );
    if flagged > 0 {
        return Err(crate::exit_codes::ReportedFailure::NeedsReview.into());
    }
    Ok(())
}
