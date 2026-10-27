//! `chatter rediarize`: re-attribute utterance speakers from an
//! external diarization turns file.
//!
//! Thin CLI shim over `talkbank_transform::rediarize`: reads the CHAT
//! input and the turns JSON, applies the transform through the shared
//! `rediarize_content` seam, writes the output (or stdout), and reports
//! the outcome summary on stderr (stderr so a stdout CHAT stream stays
//! clean). User contract: `book/src/chatter/user-guide/rediarize.md`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tracing::{Level, info, span, warn};

use talkbank_model::ParseValidateOptions;
use talkbank_transform::rediarize::{
    ContestedThreshold, RediarizeSummary, TurnsJsonError, parse_turns_json, rediarize_content,
};
use talkbank_transform::report_destination::{
    PendingPublication, ReportDestinationError, StagedReportDestination,
};

/// Cap on individually-listed flagged utterances in the summary; a
/// pathological input (e.g. an empty turns file) flags every utterance,
/// and hundreds of detail lines help nobody. The full count is always
/// reported.
const MAX_FLAGGED_DETAILS: usize = 20;

enum ChatDestination {
    Stdout,
    File(StagedReportDestination),
}

enum ChatPublication {
    Stdout(String),
    File(PendingPublication),
}

/// Both destinations are admitted and regular-file bytes staged before publication.
/// This is a publication preparation phase, not an atomic two-file transaction.
struct PreparedRediarizePublication {
    chat: ChatPublication,
    summary: Option<PendingPublication>,
}

impl PreparedRediarizePublication {
    fn prepare(
        chat: String,
        input: &Path,
        turns: &Path,
        output: Option<&Path>,
        summary: Option<(&Path, String)>,
    ) -> Result<Self, ReportDestinationError> {
        let destination = match output {
            Some(path) => {
                ChatDestination::File(StagedReportDestination::admit(path, [input, turns])?)
            }
            None => ChatDestination::Stdout,
        };
        let summary = summary
            .map(|(path, json)| {
                let summary = StagedReportDestination::admit(path, [input, turns])?;
                if let ChatDestination::File(chat) = &destination {
                    chat.ensure_distinct(&summary)?;
                }
                Ok::<_, ReportDestinationError>((summary, json))
            })
            .transpose()?;
        fn stage(
            destination: StagedReportDestination,
            bytes: &[u8],
        ) -> io::Result<PendingPublication> {
            let mut writer = destination.into_writer()?;
            writer.write_all(bytes)?;
            writer.finish()
        }
        let chat = match destination {
            ChatDestination::Stdout => ChatPublication::Stdout(chat),
            ChatDestination::File(destination) => {
                ChatPublication::File(stage(destination, chat.as_bytes())?)
            }
        };
        let summary = summary
            .map(|(destination, json)| stage(destination, json.as_bytes()))
            .transpose()?;
        Ok(Self { chat, summary })
    }

    fn publish(self) -> crate::exit_codes::CommandResult {
        fn publish(destination: PendingPublication) -> crate::exit_codes::CommandResult {
            destination.publish().map_err(|error| {
                eprintln!("Error writing rediarization output: {error}");
                crate::exit_codes::ReportedFailure::Input.into()
            })
        }
        match self.chat {
            ChatPublication::Stdout(chat) => {
                out!("{chat}");
            }
            ChatPublication::File(destination) => publish(destination)?,
        }
        if let Some(destination) = self.summary {
            publish(destination)?;
        }
        Ok(())
    }
}

/// Top-level entry for `chatter rediarize INPUT --turns TURNS.json [-o OUT]
/// [--summary-json SUMMARY.json]`.
///
/// Exit codes per the user-guide contract: 0 success (flagged
/// utterances do not fail the command), 1 invalid input (unreadable
/// file, CHAT parse failure, malformed turns JSON), 2 precondition
/// violation (turns JSON parsed but is semantically defective, e.g. an
/// inverted span). Destination admission preserves existing bytes on refusal;
/// later write failures can leave partial output.
pub fn run_rediarize(
    input: &Path,
    turns_path: &Path,
    output: Option<&PathBuf>,
    summary_json: Option<&Path>,
    contested_at: Option<f64>,
) -> crate::exit_codes::CommandResult {
    // Validated HERE, before anything is read, so a bad share fails the
    // command rather than silently meaning "nothing is contested". A `NaN`
    // would satisfy no comparison and is exactly that silent case.
    let contested_at = match contested_at.map(ContestedThreshold::new).transpose() {
        Ok(threshold) => threshold,
        Err(e) => {
            eprintln!("Error: {e}");
            return Err(crate::exit_codes::ReportedFailure::Input.into());
        }
    };
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
            return Err(crate::exit_codes::ReportedFailure::Input.into());
        }
    };
    let turns_text = match fs::read_to_string(turns_path) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to read {}: {}", turns_path.display(), e);
            eprintln!("Error reading {}: {}", turns_path.display(), e);
            return Err(crate::exit_codes::ReportedFailure::Input.into());
        }
    };

    let turns_file = match parse_turns_json(&turns_text) {
        Ok(t) => t,
        Err(e) => {
            warn!("turns JSON rejected: {}", e);
            eprintln!("Error in {}: {}", turns_path.display(), e);
            // Malformed JSON is unusable input (1); a well-formed file
            // carrying an inverted span or invalid track is a semantic precondition
            // violation (2), per the exit-code contract.
            let code = match e {
                TurnsJsonError::Json(_) => crate::exit_codes::ReportedFailure::Input,
                TurnsJsonError::InvertedTurn { .. } | TurnsJsonError::InvalidTrack { .. } => {
                    crate::exit_codes::ReportedFailure::Precondition
                }
            };
            return Err(code.into());
        }
    };
    if let Some(source) = turns_file.source() {
        info!("diarization source: {source}");
    }

    let (rewritten, outcome) = match rediarize_content(
        &content,
        turns_file.timeline(),
        ParseValidateOptions::default(),
        contested_at,
    ) {
        Ok(result) => result,
        Err(e) => {
            warn!("rediarize failed: {}", e);
            eprintln!("Error: {}", e);
            return Err(crate::exit_codes::ReportedFailure::Input.into());
        }
    };

    // Serialize and admit the whole output set before any destination truncates.
    let summary = summary_json
        .map(|path| {
            let summary = RediarizeSummary::new(turns_file.source(), &outcome);
            serde_json::to_string_pretty(&summary).map(|json| (path, json + "\n"))
        })
        .transpose();
    let summary = match summary {
        Ok(summary) => summary,
        Err(error) => {
            eprintln!("Error serializing summary: {error}");
            return Err(crate::exit_codes::ReportedFailure::Input.into());
        }
    };
    let publication = match PreparedRediarizePublication::prepare(
        rewritten,
        input,
        turns_path,
        output.map(PathBuf::as_path),
        summary,
    ) {
        Ok(publication) => publication,
        Err(error) => {
            eprintln!("Error preparing output: {error}");
            return Err(crate::exit_codes::ReportedFailure::Input.into());
        }
    };
    publication.publish()?;

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
    Ok(())
}
