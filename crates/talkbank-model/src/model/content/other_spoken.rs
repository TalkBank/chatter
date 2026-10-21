//! Interposed-speech events (`&*SPK:text`).
//!
//! This token captures brief background/interjection speech from a different
//! speaker without creating a full turn line.
//!
//! # CHAT Format References
//!
//! - [Interposed Words](https://talkbank.org/0info/manuals/CHAT.html#InterposedWords)

use super::{SpeakerCode, WriteChat};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Interposed speech payload.
///
/// # CHAT Format Examples
///
/// ```text
/// &*MOT:careful         Mother says "careful" in background
/// &*FAT:watch           Father says "watch" while child speaks
/// &*CHI:mine            Child interjects "mine"
/// &*BRO:stop            Brother says "stop" in background
/// ```
///
/// # Usage Context
///
/// Interposed words are used when another speaker briefly interjects or speaks
/// in the background without taking the conversational floor. If the intervention
/// is substantial enough to constitute a turn, it should be transcribed as a
/// separate utterance instead.
///
/// # References
///
/// - [Interposed Words](https://talkbank.org/0info/manuals/CHAT.html#InterposedWords)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct OtherSpokenEvent {
    /// Speaker code for the interposed source.
    pub speaker: SpeakerCode,

    /// Interposed text payload.
    pub text: smol_str::SmolStr,

    /// Source location metadata for diagnostics.
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub location: crate::Location,
}

impl OtherSpokenEvent {
    /// Build an interposed event with dummy span metadata.
    ///
    /// This is the common constructor for parser output prior to span attachment
    /// and for unit tests focused on serialization behavior.
    pub fn new(
        location: impl Into<crate::Location>,
        speaker: impl Into<SpeakerCode>,
        text: impl Into<smol_str::SmolStr>,
    ) -> Self {
        Self {
            speaker: speaker.into(),
            text: text.into(),
            location: location.into(),
        }
    }
}

impl WriteChat for OtherSpokenEvent {
    /// Writes interposed speech in CHAT form (`&*SPEAKER:text`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "&*{}:{}", self.speaker, self.text)
    }
}

impl std::fmt::Display for OtherSpokenEvent {
    /// Formats the canonical CHAT serialization for this interposed event.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
