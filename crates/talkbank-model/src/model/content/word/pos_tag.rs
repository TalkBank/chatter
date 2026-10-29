//! Lexical admission for the payload after a main-tier word's `$` marker.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift};

/// A payload outside the main-tier POS tag alphabet.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidWordPosTag {
    /// A present marker must have a nonempty payload.
    #[error("word POS tag is empty")]
    Empty,
    /// Tags contain ASCII letters and colons only.
    #[error("invalid word POS tag character {character:?} at byte {byte_offset}")]
    InvalidCharacter {
        /// Refused Unicode scalar.
        character: char,
        /// Byte offset in the supplied payload.
        byte_offset: usize,
    },
}

/// A nonempty main-tier POS hint without its leading `$`.
///
/// This admits the grammar's `[a-zA-Z:]+` spelling, preserving case and colons.
/// It does not impose a closed vocabulary or claim morphological correctness.
///
/// ```
/// use talkbank_model::{Location, Word, WordPosTag, WordText};
/// let word = Word::new(Location::Unlocated, WordText::new("name")?)
///     .with_part_of_speech(WordPosTag::new("N:PROP")?);
/// assert_eq!(word.to_chat(), "name$N:PROP");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Raw text cannot bypass payload admission:
///
/// ```compile_fail
/// use talkbank_model::{Location, Word, WordText};
/// let word = Word::new(Location::Unlocated, WordText::new("name").unwrap());
/// let _ = word.with_part_of_speech("n");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "String")]
pub struct WordPosTag(SmolStr);

impl WordPosTag {
    /// Admit a complete payload; no trimming or marker removal is performed.
    pub fn new(text: impl AsRef<str>) -> Result<Self, InvalidWordPosTag> {
        let text = text.as_ref();
        if text.is_empty() {
            return Err(InvalidWordPosTag::Empty);
        }
        for (byte_offset, character) in text.char_indices() {
            if !character.is_ascii_alphabetic() && character != ':' {
                return Err(InvalidWordPosTag::InvalidCharacter {
                    character,
                    byte_offset,
                });
            }
        }
        Ok(Self(SmolStr::new(text)))
    }

    /// The admitted payload, retaining its exact spelling.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for WordPosTag {
    type Error = InvalidWordPosTag;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl std::ops::Deref for WordPosTag {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for WordPosTag {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl JsonSchema for WordPosTag {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WordPosTag".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string", "minLength": 1,
            "pattern": "^[a-zA-Z:]+$",
            "not": { "pattern": "[^a-zA-Z:]" }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_pos_payloads_preserve_open_vocabulary_and_refuse_syntax_bytes() {
        for text in ["n", "N:PROP", "custom", ":", "a::b"] {
            let tag = WordPosTag::new(text).unwrap();
            assert_eq!(tag.as_str(), text);
            assert_eq!(serde_json::to_value(&tag).unwrap(), text);
            assert_eq!(
                serde_json::from_value::<WordPosTag>(serde_json::json!(text)).unwrap(),
                tag
            );
        }
        for byte in 0u8..=127 {
            let text = format!("n{}v", char::from(byte));
            assert_eq!(
                WordPosTag::new(&text).is_ok(),
                byte.is_ascii_alphabetic() || byte == b':',
                "{byte}"
            );
            assert_eq!(
                serde_json::from_value::<WordPosTag>(serde_json::json!(text)).is_ok(),
                byte.is_ascii_alphabetic() || byte == b':',
                "{byte}"
            );
        }
        assert_eq!(WordPosTag::new(""), Err(InvalidWordPosTag::Empty));
        assert_eq!(
            WordPosTag::new("né"),
            Err(InvalidWordPosTag::InvalidCharacter {
                character: 'é',
                byte_offset: 1
            })
        );
    }
}
