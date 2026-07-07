//! `chatter sanity-scan`, post-merge misclassification detector.
//!
//! Thin CLI shim over [`talkbank_transform::sanity_scan`]. For each
//! auto-decided session in the override file, look up the merged
//! CHAT file by basename, run the heuristic, and (if flagged)
//! append a `sanity-scan-misclassification` pending entry to the
//! pending file. Exit 4 if any session was flagged so operator
//! tooling can short-circuit.

use std::fs;
use std::path::Path;
use tracing::{Level, info, span, warn};

use crate::exit_codes::{EXIT_INPUT_ERROR, EXIT_LOW_CONFIDENCE, EXIT_SUCCESS};
use chrono::Utc;
use talkbank_model::{ParseValidateOptions, SpeakerCode};
use talkbank_transform::adjudication::{
    PendingAdjudications, PendingEntry, PendingKindData, SuggestedSpeakerIdMapping,
};
use talkbank_transform::parse_and_validate;
use talkbank_transform::sanity_scan::{SanityScanThreshold, scan_session};
use talkbank_transform::speaker_id::OverrideFile;

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
    threshold: f64,
    write_pending_path: &Path,
) {
    let _span = span!(
        Level::INFO,
        "chatter_sanity_scan",
        merged_dir = %merged_dir.display(),
        override_file = %override_path.display(),
    )
    .entered();

    let override_file = match OverrideFile::read_or_default(override_path) {
        Ok(f) => f,
        Err(e) => {
            warn!(
                "failed to read override file {}: {}",
                override_path.display(),
                e
            );
            eprintln!(
                "Error reading override file {}: {}",
                override_path.display(),
                e
            );
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    let anchor_code = SpeakerCode::new(anchor);
    let scan_threshold = SanityScanThreshold(threshold);
    let options = ParseValidateOptions::default();

    let mut flagged: u32 = 0;
    let mut scanned: u32 = 0;
    let mut pending = match PendingAdjudications::read_or_default(write_pending_path) {
        Ok(f) => f,
        Err(e) => {
            warn!(
                "failed to read pending file {}: {}",
                write_pending_path.display(),
                e
            );
            eprintln!(
                "Error reading pending file {}: {}",
                write_pending_path.display(),
                e
            );
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    for (session_id, entry) in override_file.auto_entries() {
        let merged_path = merged_dir.join(format!("{session_id}.cha"));
        if !merged_path.exists() {
            warn!(
                "no merged file for session {:?}; expected {}",
                session_id,
                merged_path.display()
            );
            continue;
        }
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
                std::process::exit(EXIT_INPUT_ERROR);
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
                std::process::exit(EXIT_INPUT_ERROR);
            }
        };
        if let Some(flag) = scan_session(&merged_chat, entry, &anchor_code, scan_threshold) {
            info!("⚠ flagged: {} ({})", session_id, flag.reason);
            pending.entries.push(PendingEntry {
                session_id: session_id.to_string(),
                created_at: Utc::now(),
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
                threshold_used: Some(threshold),
                engine: talkbank_transform::speaker_id::DecisionEngine::Deterministic,
                judgment: None,
            });
            flagged += 1;
        } else {
            info!("✓ ok: {}", session_id);
        }
    }

    if flagged > 0 {
        if let Err(e) = pending.write(write_pending_path) {
            warn!(
                "failed to write pending file {}: {}",
                write_pending_path.display(),
                e
            );
            eprintln!(
                "Error writing pending file {}: {}",
                write_pending_path.display(),
                e
            );
            std::process::exit(EXIT_INPUT_ERROR);
        }
        info!(
            "appended {} pending entries to: {}",
            flagged,
            write_pending_path.display()
        );
    }

    eprintln!(
        "sanity-scan summary: {scanned} auto-decided session(s) scanned; \
         {flagged} flagged for adjudication"
    );
    if flagged > 0 {
        std::process::exit(EXIT_LOW_CONFIDENCE);
    }
    std::process::exit(EXIT_SUCCESS);
}
