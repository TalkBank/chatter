//! Model for inline free-form codes (`[^ ...]`).
//!
//! Freecodes are intentionally open text payloads used for corpus- or project-
//! specific annotations that do not map to a dedicated CHAT token type.
//!
//! # CHAT Format References
//!
//! - [Complex Local Events](https://talkbank.org/0info/manuals/CHAT.html#ComplexLocalEvents)

use super::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Inline free-form annotation payload.
///
/// The model stores raw text and leaves interpretation to downstream tooling.
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Freecode {
    /// Payload text written after `[^ ` and before `]`.
    pub text: smol_str::SmolStr,

    /// Source location metadata for diagnostics.
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub location: crate::Location,
}

impl Freecode {
    /// Build a freecode with dummy span metadata.
    ///
    /// Use this when constructing values in tests or transformations where
    /// source offsets are unavailable.
    pub fn new(location: impl Into<crate::Location>, text: impl Into<smol_str::SmolStr>) -> Self {
        Self {
            text: text.into(),
            location: location.into(),
        }
    }
}

impl WriteChat for Freecode {
    /// Serializes canonical CHAT freecode syntax (`[^ ...]`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str("[^ ")?;
        w.write_str(&self.text)?;
        w.write_char(']')
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
