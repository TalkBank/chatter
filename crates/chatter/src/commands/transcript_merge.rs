//! `chatter merge`, structural merge of two CHAT transcripts.
//!
//! Parses both inputs, calls `merge_chat_files`, reports what the merge
//! dropped, and writes the result (or stdout). It works on the typed
//! `Merged` because the report is part of the required typestate transition,
//! not an optional side channel.
//!
//! [`report_merge_notices`] is shared with `chatter pipeline` so both
//! merge entry points say the same thing; a warning written at one call site
//! is a warning the other command silently lacks.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{Level, info, span, warn};

use crate::exit_codes::{EXIT_INPUT_ERROR, EXIT_PRECONDITION};
use talkbank_model::SpeakerCode;
use talkbank_transform::parse_and_validate;
use talkbank_transform::serialize::to_chat_string;
use talkbank_transform::transcript_merge::{
    MergeError, Merged, Reported, default_strip_tiers, merge_chat_files,
};

/// Top-level entry for `chatter merge file1 file2 --retain <SPK[,SPK...]>`.
///
/// Exit codes follow the user-guide contract: 1 for unusable input (I/O or
/// parse), 2 for a precondition the merge refuses on. [`merge_exit_code`] owns
/// the mapping.
pub fn run_merge(file1: &Path, file2: &Path, retain: &[String], output: Option<&PathBuf>) {
    let _span = span!(
        Level::INFO,
        "chatter_merge",
        file1 = %file1.display(),
        file2 = %file2.display(),
    )
    .entered();

    let options = talkbank_model::ParseValidateOptions::default();
    let strip = default_strip_tiers();
    // Parse the clap-provided raw strings into domain newtypes at the CLI
    // boundary. Interior code works on `&[SpeakerCode]` only.
    let retain: Vec<SpeakerCode> = retain.iter().map(SpeakerCode::new).collect();

    // Read AND parse in one place. Both halves had the same
    // match-warn-eprintln-exit shape, written twice for reading and about to
    // be written twice more for parsing.
    //
    // Parsed here at the CLI boundary. The typed merge accepts the model and
    // returns the reportable state this command needs.
    let load = |path: &Path| {
        let content = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                warn!("failed to read {}: {}", path.display(), e);
                eprintln!("Error reading {}: {}", path.display(), e);
                std::process::exit(EXIT_INPUT_ERROR);
            }
        };
        match parse_and_validate(&content, options.clone()) {
            Ok(file) => file,
            Err(e) => {
                warn!("failed to parse {}: {}", path.display(), e);
                eprintln!("Error parsing {}: {}", path.display(), e);
                std::process::exit(EXIT_INPUT_ERROR);
            }
        }
    };
    let f1 = load(file1);
    let f2 = load(file2);

    let merged = match merge_chat_files(&f1, &f2, &retain, &strip) {
        Ok(m) => m,
        Err(e) => {
            warn!("merge failed: {}", e);
            eprintln!("Error: {}", e);
            // Exit-code mapping per the user-guide contract:
            // - precondition violations → 2
            // - invalid input (parse errors) → 1
            // Future MergeError variants from later precondition
            // cycles get explicit arms here.
            std::process::exit(merge_exit_code(&e));
        }
    };

    let merged = to_chat_string(&report_merge_notices(merged, file1).into_file());

    match output {
        Some(path) => {
            if let Err(e) = fs::write(path, merged) {
                warn!("failed to write {}: {}", path.display(), e);
                eprintln!("Error writing {}: {}", path.display(), e);
                std::process::exit(EXIT_INPUT_ERROR);
            }
            info!("wrote merged file: {}", path.display());
        }
        None => {
            print!("{merged}");
        }
    }
}

/// The exit code for a merge failure, per the user-guide contract.
///
/// ONE owner, exhaustively matched. `chatter merge` enumerated every variant
/// while `chatter pipeline` wrote `_ => EXIT_PRECONDITION`, so a new variant
/// that should exit 1 would have been a compile error at one command and a
/// silent 2 at the other. `chatter batch` buckets sessions by these codes, so
/// a mis-mapped variant mis-buckets a whole corpus run with no local symptom.
///
/// Every variant is a precondition today, so every one maps to 2. The match
/// stays exhaustive rather than collapsing to a constant: the next variant
/// must be classified here, which is the whole reason this function exists.
pub(crate) fn merge_exit_code(error: &MergeError) -> i32 {
    match error {
        MergeError::RetainSpeakersMissing { .. }
        | MergeError::NoTimelineInFile1
        | MergeError::LanguageMismatch { .. }
        | MergeError::AmbiguousSpeaker { .. }
        | MergeError::ParticipantAlreadyDeclared { .. } => EXIT_PRECONDITION,
    }
}

/// Something the merge did that the operator should know about, but which is
/// not a failure.
///
/// A value rather than an `eprintln!` at the call site, following
/// `validate::cache::CacheEvent` in this same crate: it makes the message
/// testable without matching on stderr, and it gives the two merge commands
/// one thing to render rather than two format strings to keep in step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MergeNotice {
    /// File 1 speakers lost every utterance because they are not in `retain`.
    ///
    /// Carries each speaker with how many utterances it lost. The speakers
    /// come from `Merged` itself, so rendering this never touches File 1 and
    /// cannot describe the wrong file.
    SpeakersDropped {
        /// Speaker code and the number of its utterances that were dropped.
        speakers: Vec<(SpeakerCode, usize)>,
        /// The File 1 path, for a message the operator can act on.
        file1: PathBuf,
    },
}

impl MergeNotice {
    /// Every notice this merge owes the operator.
    ///
    /// Returns them rather than printing, so a caller decides where they go
    /// and a test can assert on the value.
    pub(crate) fn all(merged: &Merged, file1: &Path) -> Vec<Self> {
        let speakers = merged.dropped_speakers();
        if speakers.is_empty() {
            return Vec::new();
        }
        vec![Self::SpeakersDropped {
            speakers,
            file1: file1.to_path_buf(),
        }]
    }

    /// One sentence for a terminal.
    pub(crate) fn sentence(&self) -> String {
        match self {
            Self::SpeakersDropped { speakers, file1 } => {
                let total: usize = speakers.iter().map(|(_, count)| count).sum();
                let codes: Vec<String> = speakers
                    .iter()
                    .map(|(speaker, count)| format!("{speaker} ({count})"))
                    .collect();
                format!(
                    "Warning: dropped {total} utterance(s) from {} because their speaker \
                     is not in --retain: {}. Those speakers are still declared in the output.",
                    file1.display(),
                    codes.join(", ")
                )
            }
        }
    }
}

/// Print every notice the merge owes the operator, to stderr.
///
/// Shared by `chatter merge` and `chatter pipeline`. A warning written at one
/// call site is a warning the other command silently lacks: before this, the
/// pipeline path, which `chatter batch` drives over whole corpora, reported
/// nothing.
pub(crate) fn report_merge_notices(merged: Merged, file1: &Path) -> Reported {
    for notice in MergeNotice::all(&merged, file1) {
        // Bound once: `sentence()` builds the whole message, and two calls can
        // drift if either line grows a modifier.
        let sentence = notice.sentence();
        warn!("{sentence}");
        eprintln!("{sentence}");
    }
    // Travels `report`, which is the only route to a serializable file. The
    // sink is empty because the notices were rendered above; what the type
    // enforces is that SOMETHING looked, not what it did.
    merged.report(|_, _| {})
}
