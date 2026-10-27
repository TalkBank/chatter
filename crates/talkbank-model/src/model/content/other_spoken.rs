//! Interposed-speech events (`&*SPK:text`).
//!
//! This token captures brief background/interjection speech from a different
//! speaker without creating a full turn line.
//!
//! # CHAT Format References
//!
//! - [Interposed Words](https://talkbank.org/0info/manuals/CHAT.html#InterposedWords)

use super::{SpeakerCode, Word, WriteChat};
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
#[serde(deny_unknown_fields)]
pub struct OtherSpokenEvent {
    /// Speaker code for the interposed source.
    pub speaker: SpeakerCode,

    /// Ordinary word syntax spoken by the other speaker.
    /// This word is validated but does not enter the main speaker's alignment.
    word: Box<Word>,

    /// Source location metadata for diagnostics.
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub location: crate::Location,
}

impl OtherSpokenEvent {
    /// Build an interposed event from an ordinary structured word.
    /// The payload must have word structure; model validation remains a separate boundary.
    ///
    /// ```compile_fail,E0308
    /// let event = talkbank_model::OtherSpokenEvent::new(
    ///     talkbank_model::Location::Unlocated, "CHI", "");
    /// ```
    pub fn new(
        location: impl Into<crate::Location>,
        speaker: impl Into<SpeakerCode>,
        word: Word,
    ) -> Self {
        Self {
            speaker: speaker.into(),
            word: Box::new(word),
            location: location.into(),
        }
    }

    /// The other speaker's word, separate from the main speaker's word sequence.
    pub fn word(&self) -> &Word {
        &self.word
    }

    /// Edit structured word content without introducing an unchecked text payload.
    pub fn word_mut(&mut self) -> &mut Word {
        &mut self.word
    }
}

impl WriteChat for OtherSpokenEvent {
    /// Writes interposed speech in CHAT form (`&*SPEAKER:text`).
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        {
            w.write_str("&*")?;
            crate::model::WriteChat::write_chat(&self.speaker, w)?;
            w.write_str(":")?;
            self.word.write_chat(w)
        }
    }
}

impl std::fmt::Display for OtherSpokenEvent {
    /// Formats the canonical CHAT serialization for this interposed event.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interposed_wire_requires_structured_word_and_refuses_legacy_text() {
        let event = OtherSpokenEvent::new(
            crate::Location::Unlocated,
            "CHI",
            Word::new(
                crate::Location::Unlocated,
                crate::non_empty_literal!("hello").into(),
            ),
        );
        let wire = serde_json::to_value(&event).unwrap();
        assert!(wire.get("word").is_some());
        assert!(wire.get("text").is_none());
        let decoded: OtherSpokenEvent = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(decoded.to_chat_string(), "&*CHI:hello");
        for text in ["", "hello", "hello world"] {
            assert!(
                serde_json::from_value::<OtherSpokenEvent>(
                    serde_json::json!({ "speaker": "CHI", "text": text })
                )
                .is_err()
            );
        }
        let mut ambiguous = wire;
        ambiguous["text"] = serde_json::json!("goodbye");
        assert!(serde_json::from_value::<OtherSpokenEvent>(ambiguous).is_err());
    }
}
