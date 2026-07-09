//! `chatter rediarize`: re-attribute utterance speakers from an
//! external diarization turns file.
//!
//! Thin CLI shim over `talkbank_transform::rediarize`: reads the CHAT
//! input and the turns JSON, applies the transform through the shared
//! `rediarize_content` seam, writes the output (or stdout), and reports
//! the outcome summary on stderr (stderr so a stdout CHAT stream stays
//! clean). User contract: `book/src/chatter/user-guide/rediarize.md`.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{Level, info, span, warn};

use crate::exit_codes::{EXIT_INPUT_ERROR, EXIT_PRECONDITION};
use talkbank_model::ParseValidateOptions;
use talkbank_transform::rediarize::{TurnsJsonError, parse_turns_json, rediarize_content};

/// Cap on individually-listed flagged utterances in the summary; a
/// pathological input (e.g. an empty turns file) flags every utterance,
/// and hundreds of detail lines help nobody. The full count is always
/// reported.
const MAX_FLAGGED_DETAILS: usize = 20;

/// Top-level entry for `chatter rediarize INPUT --turns TURNS.json [-o OUT]`.
///
/// Exit codes per the user-guide contract: 0 success (flagged
/// utterances do not fail the command), 1 invalid input (unreadable
/// file, CHAT parse failure, malformed turns JSON), 2 precondition
/// violation (turns JSON parsed but is semantically defective, e.g. an
/// inverted span). No output file is written on any non-zero exit.
pub fn run_rediarize(input: &Path, turns_path: &Path, output: Option<&PathBuf>) {
    let _span = span!(
        Level::INFO,
        "chatter_rediarize",
        input = %input.display(),
        turns = %turns_path.display(),
    )
    .entered();

    let content = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to read {}: {}", input.display(), e);
            eprintln!("Error reading {}: {}", input.display(), e);
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };
    let turns_text = match fs::read_to_string(turns_path) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to read {}: {}", turns_path.display(), e);
            eprintln!("Error reading {}: {}", turns_path.display(), e);
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    let turns_file = match parse_turns_json(&turns_text) {
        Ok(t) => t,
        Err(e) => {
            warn!("turns JSON rejected: {}", e);
            eprintln!("Error in {}: {}", turns_path.display(), e);
            // Malformed JSON is unusable input (1); a well-formed file
            // carrying an inverted span is a semantic precondition
            // violation (2), per the exit-code contract.
            let code = match e {
                TurnsJsonError::Json(_) => EXIT_INPUT_ERROR,
                TurnsJsonError::InvertedTurn { .. } => EXIT_PRECONDITION,
            };
            std::process::exit(code);
        }
    };
    if let Some(source) = &turns_file.source {
        info!("diarization source: {source}");
    }

    let (rewritten, outcome) =
        match rediarize_content(&content, &turns_file.turns, ParseValidateOptions::default()) {
            Ok(result) => result,
            Err(e) => {
                warn!("rediarize failed: {}", e);
                eprintln!("Error: {}", e);
                std::process::exit(EXIT_INPUT_ERROR);
            }
        };

    match output {
        Some(path) => {
            if let Err(e) = fs::write(path, &rewritten) {
                warn!("failed to write {}: {}", path.display(), e);
                eprintln!("Error writing {}: {}", path.display(), e);
                std::process::exit(EXIT_INPUT_ERROR);
            }
            info!("wrote rediarized file: {}", path.display());
        }
        None => {
            print!("{rewritten}");
        }
    }

    // Outcome summary AFTER the write so it describes a completed
    // operation. `unchanged` includes flagged utterances (they kept
    // their speaker); the flagged count calls them out separately.
    eprintln!(
        "rediarize: {} reassigned, {} unchanged, {} flagged",
        outcome.reassigned,
        outcome.unchanged,
        outcome.flagged.len()
    );
    for flagged in outcome.flagged.iter().take(MAX_FLAGGED_DETAILS) {
        eprintln!(
            "  flagged: utterance {} kept *{}: ({})",
            flagged.utterance_index,
            flagged.kept_speaker.as_str(),
            flagged.reason
        );
    }
    if outcome.flagged.len() > MAX_FLAGGED_DETAILS {
        eprintln!(
            "  ... and {} more flagged utterances",
            outcome.flagged.len() - MAX_FLAGGED_DETAILS
        );
    }
}
