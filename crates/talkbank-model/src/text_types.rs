//! Distinct raw and cleaned text projections for CHAT pipelines.
//!
//! `ChatCleanedText` and `ChatRawText` distinguish two text projections. Their
//! named constructors project a typed CHAT-AST [`Word`] or [`Separator`].
//! Deserialization also accepts wire strings, so these types do not certify
//! source provenance or validation.
//!
//! # Why distinct projection types
//!
//! Cleaned text is, by definition, a computed view of a CHAT model, see
//! [`Word::compute_cleaned_text`] for the canonical projection (concatenated
//! `WordContent::Text` graphemes plus `WordContent::Shortening` restored
//! material; all prosodic/analytical markers excluded). "Cleaned" is meaningful
//! only relative to a structured source state. Named construction therefore
//! projects model values, while imported strings carry no proof of that origin.
//! Provider transport substitutions belong in a separate downstream request
//! type; they must never be manufactured as cleaned source text.
//!
//! Per-consumer projections downstream of `ChatCleanedText` (lowercasing for
//! frequency keys, apostrophe stripping for `wdlen`, etc.) are downstream
//! transforms, not different flavors of cleaned text. See
//! `<workspace>/docs/cleaned-text-consumer-policies.md` for the per-consumer
//! audit.
//!
//! See also `<workspace>/.claude/plans/migration-item-1-nutype-foundation-from-talkbank-model.md`
//! for the migration plan and `<workspace>/docs/architecture-review-2026-05-01.md`
//! Debt 11 for the architectural debt this change addresses.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::model::WriteChat;
use crate::model::content::Separator;
use crate::model::content::Word;

/// Generates the six `PartialEq` impls that let a projection
/// string newtype be compared against `str`, `&str`, and `String` (in
/// either argument position). Comparison is read-only and does not
/// construct a value; this is ergonomics for tests and diagnostic
/// code without constructing another projection.
///
/// Used by [`ChatRawText`] and [`ChatCleanedText`].
macro_rules! impl_string_comparison_helpers {
    ($t:ty) => {
        impl PartialEq<str> for $t {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }
        impl PartialEq<&str> for $t {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<String> for $t {
            fn eq(&self, other: &String) -> bool {
                &self.0 == other
            }
        }
        impl PartialEq<$t> for str {
            fn eq(&self, other: &$t) -> bool {
                self == other.0.as_str()
            }
        }
        impl PartialEq<$t> for &str {
            fn eq(&self, other: &$t) -> bool {
                *self == other.0.as_str()
            }
        }
        impl PartialEq<$t> for String {
            fn eq(&self, other: &$t) -> bool {
                self == &other.0
            }
        }
    };
}

/// Source-faithful raw token text from a parsed CHAT AST node.
///
/// Includes CHAT markers and punctuation exactly as parsed, suitable for
/// roundtrip serialization and diagnostics. Named constructors project a typed
/// AST source; deserialization does not certify provenance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct ChatRawText(String);

impl ChatRawText {
    /// Raw token text from a parsed [`Word`]. Mirrors [`Word::raw_text`].
    pub fn from_word_raw(word: &Word) -> Self {
        Self(word.raw_text().to_string())
    }

    /// Raw text projection of a parsed [`Separator`]. Separators have no
    /// raw/cleaned distinction, both projections render to the same CHAT
    /// surface form via [`WriteChat::to_chat_string`].
    pub fn from_separator(sep: &Separator) -> Self {
        Self(sep.to_chat_string())
    }

    /// Borrows the wrapped raw text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChatRawText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ChatRawText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Read-only comparison helpers (see the macro definition below for
// rationale). Comparison does not construct a value; these are ergonomics
// for tests and log/diagnostic code.
impl_string_comparison_helpers!(ChatRawText);

/// Lexical content extracted from CHAT after markup stripping, projected from a
/// parsed CHAT AST node.
///
/// Cleaned text is the projection defined by [`Word::compute_cleaned_text`]:
/// concatenated `WordContent::Text` (base graphemes) and `WordContent::Shortening`
/// (elided material restored, e.g. `som(e)` → `some`). All prosodic/analytical
/// markers are excluded.
///
/// Named constructors project a typed AST source. Deserialization does not
/// certify source provenance; see module-level docs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct ChatCleanedText(String);

impl ChatCleanedText {
    /// Cleaned-text projection of a parsed [`Word`]. Delegates to
    /// [`Word::cleaned_text`] (which itself caches the result of
    /// [`Word::compute_cleaned_text`]).
    pub fn from_word(word: &Word) -> Self {
        Self(word.cleaned_text().to_string())
    }

    /// Cleaned-text projection of a parsed [`Separator`]. A separator's
    /// cleaned form is its CHAT surface; there are no markers to strip.
    pub fn from_separator(sep: &Separator) -> Self {
        Self(sep.to_chat_string())
    }

    /// Borrows the wrapped cleaned text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns an iterator over the characters of the cleaned text.
    pub fn chars(&self) -> std::str::Chars<'_> {
        self.0.chars()
    }

    /// Lowercases the cleaned text.
    pub fn to_lowercase(&self) -> String {
        self.0.to_lowercase()
    }
}

impl fmt::Display for ChatCleanedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ChatCleanedText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl_string_comparison_helpers!(ChatCleanedText);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::content::Word;

    #[test]
    fn cleaned_and_raw_text_remain_distinct() {
        let word = Word::new(
            crate::Location::Unlocated,
            crate::WordText::from(crate::non_empty_literal!("hello")),
        )
        .with_form_type(crate::model::FormType::C);
        let raw = ChatRawText::from_word_raw(&word);
        let cleaned = ChatCleanedText::from_word(&word);

        assert_eq!(raw.as_str(), "hello@c");
        assert_eq!(cleaned.as_str(), "hello");
    }

    #[test]
    fn cleaned_text_serializes_transparently() {
        let word = Word::new(
            crate::Location::Unlocated,
            crate::WordText::from(crate::non_empty_literal!("test")),
        );
        let text = ChatCleanedText::from_word(&word);
        let json = serde_json::to_string(&text).unwrap();
        assert_eq!(json, "\"test\"");

        let decoded: ChatCleanedText = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, text);
    }
}
