//! Build a validated CHAT file from a structured transcript description.
//!
//! Given participants, optional media, and utterances as pre-formatted CHAT
//! main-tier text (a [`TranscriptDescription`]), assemble a [`ChatFile`] AST:
//! synthesize the header block, parse each utterance through the tree-sitter
//! parser (so the result is real, validated model structure, never hand-built
//! scaffolding), and close with `@End`.
//!
//! This is the general CHAT-generation entry point for any converter (the
//! MICASE/SBCSAE converters, external tools). It has NO ML, audio, network, or
//! fleet dependency. The batchalign ASR path (timed word tokens, retrace runs,
//! `%wor`, and the JSON/PyO3 bridge) is a downstream layer on top of this and
//! is not part of the general builder.

mod headers;
mod parser;
mod schema;
mod utterances;

#[cfg(test)]
mod tests;

use talkbank_model::model::{ChatFile, Header, Line};

pub use schema::{ParticipantDesc, TranscriptDescription, UtteranceDesc};

use headers::build_header_lines;
use parser::BuildChatContext;
use utterances::build_utterance_lines;

/// Failure assembling a CHAT file from a [`TranscriptDescription`].
#[derive(Debug, thiserror::Error)]
pub enum BuildChatError {
    /// The description carried no participants; CHAT requires at least one.
    #[error("at least one participant is required")]
    NoParticipants,
    /// The description named no languages.
    ///
    /// This used to default to `eng`, silently, which is a value the builder
    /// INVENTED appearing in `@Languages` and in every `@ID` indistinguishably
    /// from one the caller stated. A transcript's language is not something a
    /// file-assembler can know.
    #[error("at least one language is required; `@Languages` cannot be invented")]
    NoLanguages,
    /// A participant named no corpus.
    ///
    /// This used to substitute the literal `corpus_name`, which is worse than
    /// an error: it is a plausible-looking value that reaches the `@ID` header
    /// and the published file.
    #[error("participant {speaker} named no corpus; the `@ID` corpus field cannot be invented")]
    EmptyCorpus {
        /// The speaker whose `corpus` was empty.
        speaker: String,
    },
    /// A downstream step (language-code parsing, per-utterance parse) failed.
    #[error("failed to build CHAT: {0}")]
    Build(String),
    /// The supplied media name cannot be written to an `@Media` header and
    /// read back unchanged.
    #[error("invalid @Media filename: {0}")]
    MediaFilename(#[from] talkbank_model::model::MediaFilenameError),
}

/// Build a validated CHAT file from a typed transcript description.
///
/// Returns [`BuildChatError`] if the description has no participants or an
/// utterance/header value cannot be parsed into the model.
pub fn build_chat(desc: &TranscriptDescription) -> Result<ChatFile, BuildChatError> {
    if desc.participants.is_empty() {
        return Err(BuildChatError::NoParticipants);
    }
    // The two facts a file-assembler cannot know, refused here beside the one
    // that was already refused. Both used to be invented: `langs` became
    // `["eng"]` and an empty corpus became the literal `corpus_name`, and both
    // reached the published `@Languages` and `@ID` headers looking stated.
    if desc.langs.is_empty() {
        return Err(BuildChatError::NoLanguages);
    }
    if let Some(participant) = desc
        .participants
        .iter()
        .find(|participant| participant.corpus.is_empty())
    {
        return Err(BuildChatError::EmptyCorpus {
            speaker: participant.id.clone(),
        });
    }

    let context = BuildChatContext::new(desc).map_err(BuildChatError::Build)?;
    let mut lines = build_header_lines(desc, context.langs())?;
    lines.extend(
        build_utterance_lines(&desc.utterances, context.parser(), context.primary_lang())
            .map_err(BuildChatError::Build)?,
    );
    lines.push(Line::header(Header::End));

    // `with_participants`, NOT `new`. `ChatFile::new` leaves the participant
    // map EMPTY -- its doc says it is for "when participant metadata has not
    // been assembled yet", i.e. mid-parse -- and validating such a file reports
    // E522, "speaker used on main tier but not declared in @Participants",
    // against a file whose `@Participants` line is right there, because the
    // check reads the MAP. Every producer of a `ChatFile` except this one ran
    // the join.
    //
    // `report_into` is the only way to reach the map and it takes the sink, so
    // possession of the map is proof the join's diagnostics were reported. They
    // are surfaced as a build failure here: an `@ID` and an `@Participants`
    // entry that do not reconcile is a fault in the DESCRIPTION, which is this
    // function's input, not something to hand downstream.
    let join_errors = talkbank_model::errors::ErrorCollector::new();
    let participants =
        talkbank_model::model::participant::join::build_participants_from_lines(&lines)
            .report_into(&join_errors);
    if join_errors.has_errors() {
        return Err(BuildChatError::Build(
            join_errors
                .to_vec()
                .iter()
                .map(|e| format!("{} {}", e.code.as_str(), e.message))
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }

    Ok(ChatFile::with_participants(lines, participants))
}
