//! Postcode tokens (`[+ ...]`) used for utterance-level analysis tags.
//!
//! # CHAT Format References
//!
//! - [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)

use super::WriteChat;
use super::code_payload::CodePayload;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Utterance-level postcode payload.
///
/// The model stores raw postcode text and leaves interpretation to downstream
/// tooling/pipelines.
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want cookie [+ exc] .      Exclude from analysis
/// *MOT: very good [+ imp] !          Imitation prompt
/// ```
///
/// # Common Postcodes
///
/// - `[+ exc]` - Exclude utterance
/// - `[+ trn]` - Translation
/// - `[+ gram]` - Grammatical
/// - `[+ jar]` - Jargon
///
/// # References
///
/// - [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)
/// - [Excluded Utterance Postcode](https://talkbank.org/0info/manuals/CHAT.html#ExcludedUtterancePostcode)
/// - [Included Utterance Postcode](https://talkbank.org/0info/manuals/CHAT.html#IncludedUtterancePostcode)
#[derive(Clone, Debug, PartialEq, JsonSchema, SemanticEq, SpanShift)]
#[schemars(with = "CodePayload")]
pub struct Postcode {
    content: CodePayload,
    /// Source location metadata for diagnostics (not serialized).
    #[semantic_eq(skip)]
    pub location: crate::Location,
}

impl Serialize for Postcode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.content.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Postcode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            content: CodePayload::deserialize(deserializer)?,
            location: crate::Location::Unlocated,
        })
    }
}

impl Postcode {
    /// Classify a structured payload once; blank content remains invalid recovery.
    pub fn new(location: impl Into<crate::Location>, text: impl Into<smol_str::SmolStr>) -> Self {
        Self {
            content: CodePayload::new(text),
            location: location.into(),
        }
    }

    /// Preserve the complete spelling of a recognized but invalid postcode.
    pub fn unparsed(
        location: impl Into<crate::Location>,
        source: impl Into<smol_str::SmolStr>,
    ) -> Self {
        Self {
            content: CodePayload::unparsed(source),
            location: location.into(),
        }
    }

    /// Admit an extracted payload, retaining its full lexeme when content is missing.
    pub fn from_lexed(location: impl Into<crate::Location>, payload: &str, lexeme: &str) -> Self {
        Self {
            content: CodePayload::from_lexed(payload, lexeme),
            location: location.into(),
        }
    }

    /// Admitted payload; recovery has no valid code text.
    pub fn text(&self) -> Option<&str> {
        self.content.text()
    }

    /// Complete retained source, available only for unparsed recovery.
    pub fn unparsed_source(&self) -> Option<&str> {
        self.content.unparsed_source()
    }

    /// Report recovery invalidity without rechecking text or requiring source coordinates.
    pub(crate) fn validate_recovery(&self, errors: &(impl crate::ErrorSink + ?Sized)) {
        if self.text().is_none() {
            errors.report(crate::ParseError::at_location(
                crate::ErrorCode::InvalidPostcode,
                crate::Severity::Error,
                self.location,
                "Postcode is missing required content or retains unparsed source",
            ));
        }
    }
}

impl WriteChat for Postcode {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        self.content.write_chat("[+ ", w)
    }
}

impl std::fmt::Display for Postcode {
    /// Formats the postcode as CHAT text (`[+ <text>]`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postcode_wire_admission_preserves_recovery_and_valid_compatibility() {
        for payload in ["", "  ", "\t", "bch", " bch"] {
            let wire = serde_json::json!({"text": payload});
            let value: Postcode = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(value.text().is_some(), !payload.trim_end().is_empty());
            let errors = crate::ErrorCollector::new();
            value.validate_recovery(&errors);
            assert_eq!(errors.has_errors(), payload.trim_end().is_empty());
            assert_eq!(serde_json::to_value(&value).unwrap(), wire);
            assert_eq!(value.location, crate::Location::Unlocated);
        }
        let value = Postcode::unparsed(crate::Location::Unlocated, "[+  ]");
        let wire = serde_json::to_value(&value).unwrap();
        assert_eq!(wire, serde_json::json!({"unparsed": "[+  ]"}));
        let decoded: Postcode = serde_json::from_value(wire).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.to_string(), "[+  ]");
        assert!(
            serde_json::from_value::<Postcode>(
                serde_json::json!({"text":"bch", "unparsed":"[+  ]"})
            )
            .is_err()
        );
    }

    /// Regression: simple postcode text roundtrips through `Display`.
    #[test]
    fn postcode_simple_roundtrip() {
        let postcode = Postcode::new(crate::Location::Unlocated, "bch");
        let output = postcode.to_string();
        assert_eq!(output, "[+ bch]", "Postcode roundtrip failed");
    }

    /// Regression: postcode text with spaces is preserved during serialization.
    #[test]
    fn postcode_with_spaces_roundtrip() {
        let postcode = Postcode::new(crate::Location::Unlocated, "long text with spaces");
        let output = postcode.to_string();
        assert_eq!(
            output, "[+ long text with spaces]",
            "Postcode with spaces roundtrip failed"
        );
    }
}
