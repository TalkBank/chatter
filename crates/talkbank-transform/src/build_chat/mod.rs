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
    /// A downstream step (language-code parsing, per-utterance parse) failed.
    #[error("failed to build CHAT: {0}")]
    Build(String),
}

/// Build a validated CHAT file from a typed transcript description.
///
/// Returns [`BuildChatError`] if the description has no participants or an
/// utterance/header value cannot be parsed into the model.
pub fn build_chat(desc: &TranscriptDescription) -> Result<ChatFile, BuildChatError> {
    if desc.participants.is_empty() {
        return Err(BuildChatError::NoParticipants);
    }

    let context = BuildChatContext::new(desc).map_err(BuildChatError::Build)?;
    let mut lines = build_header_lines(desc, context.langs());
    lines.extend(
        build_utterance_lines(
            &desc.utterances,
            context.parser(),
            context.langs(),
            context.primary_lang(),
        )
        .map_err(BuildChatError::Build)?,
    );
    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}
