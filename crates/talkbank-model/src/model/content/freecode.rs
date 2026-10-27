//! Model for inline free-form codes (`[^ ...]`).
//!
//! Freecodes are intentionally open text payloads used for corpus- or project-
//! specific annotations that do not map to a dedicated CHAT token type.
//!
//! # CHAT Format References
//!
//! - [Complex Local Events](https://talkbank.org/0info/manuals/CHAT.html#ComplexLocalEvents)

use super::WriteChat;
use super::code_payload::CodePayload;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Inline free-form annotation payload.
///
/// The model leaves code interpretation to downstream tooling. Blank structured
/// payloads and explicitly unparsed source remain invalid recovery; JSON cannot
/// promote them to admitted text.
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want [^ gesture] cookie .
/// *MOT: look [^ emphasis] there !
/// ```
///
/// # References
///
/// - [Complex Local Events](https://talkbank.org/0info/manuals/CHAT.html#ComplexLocalEvents)
#[derive(Clone, Debug, PartialEq, JsonSchema, SemanticEq, SpanShift)]
#[schemars(with = "CodePayload")]
pub struct Freecode {
    content: CodePayload,
    /// Source location metadata for diagnostics (not serialized).
    #[semantic_eq(skip)]
    pub location: crate::Location,
}

impl Serialize for Freecode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.content.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Freecode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            content: CodePayload::deserialize(deserializer)?,
            location: crate::Location::Unlocated,
        })
    }
}

impl Freecode {
    /// Classify a structured payload once; blank content remains invalid recovery.
    pub fn new(location: impl Into<crate::Location>, text: impl Into<smol_str::SmolStr>) -> Self {
        Self {
            content: CodePayload::new(text),
            location: location.into(),
        }
    }

    /// Preserve the complete spelling of a recognized but invalid freecode.
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
    pub fn validate_recovery(&self, errors: &(impl crate::ErrorSink + ?Sized)) {
        if self.text().is_none() {
            errors.report(crate::ParseError::at_location(
                crate::ErrorCode::UnparsableContent,
                crate::Severity::Error,
                self.location,
                "Freecode is missing required content or retains unparsed source",
            ));
        }
    }
}

impl WriteChat for Freecode {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        self.content.write_chat("[^ ", w)
    }
}

impl std::fmt::Display for Freecode {
    /// Formats this freecode as a CHAT inline annotation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freecode_wire_admission_preserves_recovery_and_valid_compatibility() {
        for payload in ["", "  ", "\t", "note", " note"] {
            let wire = serde_json::json!({"text": payload});
            let value: Freecode = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(value.text().is_some(), !payload.trim_end().is_empty());
            let errors = crate::ErrorCollector::new();
            value.validate_recovery(&errors);
            assert_eq!(errors.has_errors(), payload.trim_end().is_empty());
            assert_eq!(serde_json::to_value(&value).unwrap(), wire);
            assert_eq!(value.location, crate::Location::Unlocated);
        }
        let value = Freecode::unparsed(crate::Location::Unlocated, "[^  ]");
        let wire = serde_json::to_value(&value).unwrap();
        assert_eq!(wire, serde_json::json!({"unparsed": "[^  ]"}));
        let decoded: Freecode = serde_json::from_value(wire).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.to_string(), "[^  ]");
        assert!(
            serde_json::from_value::<Freecode>(
                serde_json::json!({"text":"note", "unparsed":"[^  ]"})
            )
            .is_err()
        );
    }

    /// Round-trips a single-token freecode through `Display`.
    #[test]
    fn freecode_simple_roundtrip() {
        let freecode = Freecode::new(crate::Location::Unlocated, "comment");
        let output = freecode.to_string();
        assert_eq!(output, "[^ comment]", "Freecode roundtrip failed");
    }

    /// Round-trips a multi-token freecode with internal whitespace.
    #[test]
    fn freecode_with_spaces_roundtrip() {
        let freecode = Freecode::new(crate::Location::Unlocated, "this is a longer comment");
        let output = freecode.to_string();
        assert_eq!(
            output, "[^ this is a longer comment]",
            "Freecode with spaces roundtrip failed"
        );
    }
}
