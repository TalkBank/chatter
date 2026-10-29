//! Lexical admission for a declared special-form marker's custom label.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift};

/// A custom label that cannot occupy one CHAT form suffix.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidFormLabel {
    /// A label-taking marker requires a nonempty label.
    #[error("custom form label is empty")]
    Empty,
    /// Labels contain ASCII letters, digits and underscores only.
    #[error("invalid custom form label character {character:?} at byte {byte_offset}")]
    InvalidCharacter {
        /// Refused Unicode scalar.
        character: char,
        /// Byte offset in the supplied label.
        byte_offset: usize,
    },
}

/// A nonempty custom form label without the leading `@z:`.
///
/// Both grammars admit `[a-zA-Z0-9_]+`. No vocabulary or case normalization
/// is imposed. Undeclared form-marker recovery remains a separate state.
///
/// ```
/// use talkbank_model::{FormLabel, FormType, Location, Word, WordSuffix, WordText};
/// let word = Word::new(Location::Unlocated, WordText::new("word")?)
///     .with_suffix(WordSuffix::Form(FormType::UserDefined(FormLabel::new("Code_9")?)));
/// assert_eq!(word.to_chat(), "word@z:Code_9");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Unchecked text cannot construct a declared custom form:
///
/// ```compile_fail
/// use talkbank_model::FormType;
/// let _ = FormType::UserDefined(String::from("two words"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "String")]
pub struct FormLabel(SmolStr);

impl FormLabel {
    /// Admit the complete label without trimming or removing marker syntax.
    pub fn new(text: impl AsRef<str>) -> Result<Self, InvalidFormLabel> {
        let text = text.as_ref();
        if text.is_empty() {
            return Err(InvalidFormLabel::Empty);
        }
        for (byte_offset, character) in text.char_indices() {
            if !character.is_ascii_alphanumeric() && character != '_' {
                return Err(InvalidFormLabel::InvalidCharacter {
                    character,
                    byte_offset,
                });
            }
        }
        Ok(Self(SmolStr::new(text)))
    }

    /// Exact admitted spelling.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for FormLabel {
    type Error = InvalidFormLabel;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl std::fmt::Display for FormLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for FormLabel {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl JsonSchema for FormLabel {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FormLabel".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string", "minLength": 1,
            "pattern": "^[a-zA-Z0-9_]+$",
            "not": { "pattern": "[^a-zA-Z0-9_]" }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_labels_preserve_spelling_and_refuse_syntax_characters() {
        for text in ["a", "9", "_", "Code_9"] {
            let label = FormLabel::new(text).unwrap();
            assert_eq!(label.as_str(), text);
            assert_eq!(label.to_string(), text);
            assert_eq!(serde_json::to_value(&label).unwrap(), text);
            assert_eq!(
                serde_json::from_value::<FormLabel>(serde_json::json!(text)).unwrap(),
                label
            );
        }
        for byte in 0u8..=127 {
            let text = format!("a{}z", char::from(byte));
            let allowed = byte.is_ascii_alphanumeric() || byte == b'_';
            assert_eq!(FormLabel::new(&text).is_ok(), allowed, "{byte}");
            assert_eq!(
                serde_json::from_value::<FormLabel>(serde_json::json!(text)).is_ok(),
                allowed,
                "{byte}"
            );
        }
        assert_eq!(FormLabel::new(""), Err(InvalidFormLabel::Empty));
        assert_eq!(
            FormLabel::new("aé"),
            Err(InvalidFormLabel::InvalidCharacter {
                character: 'é',
                byte_offset: 1,
            })
        );
    }
}
