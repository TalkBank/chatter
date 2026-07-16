//! `LanguageCode` model and validation helpers.
//!
//! This type is the canonical language token used across headers, utterance
//! language metadata, and word-level language-switch annotations.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>

use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use talkbank_derive::{SemanticEq, SpanShift};

/// Interned ISO 639-3 language token used across header/main-tier language fields.
///
/// Three-letter language code following the ISO 639-3 standard. Used throughout
/// CHAT files to specify the language of utterances, participants, and transcripts.
///
/// ## Memory Optimization
///
/// This type uses `Arc<str>` with interning for memory efficiency:
/// - All codes are interned through a global interner
/// - Common codes (eng, spa, deu, etc.) are pre-populated on first use
/// - Cloning is O(1) (atomic reference count increment)
/// - Multiple occurrences of the same code share a single Arc
///
/// This reduces memory usage by 5-20MB for large corpora.
///
/// # CHAT Usage
///
/// **In headers:**
/// - `@Languages:` - Declares all languages used in the transcript
/// - `@Language of SPK:` - Specifies a participant's language
/// - `@ID` header field 1 - Primary language of transcript
///
/// **In main tiers:**
/// - `[- code]` - Language switching annotation for individual words
/// - `[+ code]` - Extended language annotation
///
/// # CHAT Format Examples
///
/// ```text
/// @Languages: eng, spa
/// @Language of CHI: eng
/// @ID: eng|corpus|CHI|...
/// *CHI: I want agua [- spa].
/// *MOT: say [+ eng] water.
/// ```
///
/// # Common Language Codes
///
/// - `eng` - English
/// - `spa` - Spanish
/// - `deu` - German (Deutsch)
/// - `fra` - French
/// - `zho` - Chinese
/// - `jpn` - Japanese
/// - `ita` - Italian
/// - `por` - Portuguese
/// - `rus` - Russian
/// - `ara` - Arabic
/// - `hin` - Hindi
/// - `kor` - Korean
///
/// # Validation
///
/// Parser acceptance is permissive; validation enforces:
/// - Three-letter lowercase format
/// - obvious placeholder rejection (`xyz`, `xxx`, `yyy`, `zzz`)
///
/// # References
///
/// - [CHAT Manual: Language Codes](https://talkbank.org/0info/manuals/CHAT.html#Language_Codes)
/// - [ISO 639-3 Standard](https://iso639-3.sil.org/)
/// - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
#[derive(
    Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift,
)]
#[serde(transparent)]
pub struct LanguageCode(pub Arc<str>);

/// Why constructing a [`LanguageCode`] failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LanguageCodeError {
    /// The input string was empty.
    ///
    /// Use [`LanguageCode::empty`] for parser recovery when a language field
    /// is missing; that sentinel is a real, valid code (`"und"`), never a
    /// silently-accepted empty value.
    #[error("language code cannot be empty; use LanguageCode::empty() for parser recovery")]
    Empty,
}

impl LanguageCode {
    /// Construct and intern a language token.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageCodeError::Empty`] if `value` is empty. Use
    /// [`empty`](Self::empty) for parser recovery when a language field is
    /// missing.
    pub fn new(value: impl AsRef<str>) -> Result<Self, LanguageCodeError> {
        let s = value.as_ref();
        if s.is_empty() {
            Err(LanguageCodeError::Empty)
        } else {
            Ok(Self(crate::model::language_interner().intern(s)))
        }
    }

    /// Sentinel for parser recovery when a language field is missing.
    ///
    /// This produces a `LanguageCode` with the placeholder value `"und"`
    /// (ISO 639-3 "undetermined"), which is a valid 3-letter code that
    /// signals "language not specified." It passes format validation but
    /// can be detected by downstream code.
    pub fn empty() -> Self {
        Self(crate::model::language_interner().intern("und"))
    }

    /// Whether this is the "undetermined" sentinel from parser recovery.
    pub fn is_undetermined(&self) -> bool {
        self.0.as_ref() == "und"
    }

    /// Borrow as `&str`.
    ///
    /// Prefer this accessor instead of depending on the internal `Arc<str>`
    /// representation in callers.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl crate::model::WriteChat for LanguageCode {
    /// Writes the raw code token with no additional normalization.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(&self.0)
    }
}

impl std::fmt::Display for LanguageCode {
    /// Displays the interned language code text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Deref for LanguageCode {
    type Target = str;

    /// Exposes the code as `&str` for generic string APIs.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for LanguageCode {
    /// Borrows this code as `&str`.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for LanguageCode {
    type Error = LanguageCodeError;

    /// Interns an owned string as a `LanguageCode`.
    ///
    /// A plain `From` is deliberately not implemented: construction can fail
    /// on empty input, and a silent `From` that panicked or silently
    /// substituted a placeholder would hide that failure from the caller.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for LanguageCode {
    type Error = LanguageCodeError;

    /// Interns a borrowed string as a `LanguageCode`.
    ///
    /// See [`TryFrom<String>`](#impl-TryFrom<String>-for-LanguageCode) for why
    /// this is fallible rather than a plain `From`.
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl std::borrow::Borrow<str> for LanguageCode {
    /// Supports hashmap/set lookups keyed by `str`.
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Validate for LanguageCode {
    /// Enforce basic CHAT-facing language-code constraints.
    fn validate(&self, _context: &ValidationContext, errors: &impl crate::ErrorSink) {
        self.report_code_issues(errors);
    }
}

impl LanguageCode {
    /// Report every CHAT-facing constraint violation on this code:
    /// 3-lowercase-letter shape, no disallowed placeholders, and ISO 639-3
    /// registry membership. Context-free so BOTH validation layers share
    /// one rule: header validation (`Validate for LanguageCode`, covering
    /// `@Languages` / `@ID`) and word-level explicit-switch validation
    /// (`resolve_word_language`, which re-anchors the reported spans to
    /// the word). Diagnostics anchor at offset 0; callers with a real
    /// span remap `location.span` after collection.
    pub(crate) fn report_code_issues(&self, errors: &impl crate::ErrorSink) {
        let is_three_lowercase =
            self.0.len() == 3 && self.0.chars().all(|c| c.is_ascii_lowercase());

        // Language codes should be 3 lowercase letters (ISO 639-3 format)
        if self.0.len() != 3 {
            errors.report(
                ParseError::new(
                    ErrorCode::InvalidLanguageCode,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new(self.as_str(), 0..self.0.len(), "language_code"),
                    format!(
                        "Language code '{}' should be 3 characters (got {})",
                        self.0,
                        self.0.len()
                    ),
                )
                .with_suggestion("Use ISO 639-3 three-letter language codes (e.g., eng, spa, deu)"),
            );
        }

        // Check if all characters are lowercase letters
        if !self.0.chars().all(|c| c.is_ascii_lowercase()) {
            errors.report(
                ParseError::new(
                    ErrorCode::InvalidLanguageCode,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new(self.as_str(), 0..self.0.len(), "language_code"),
                    format!(
                        "Language code '{}' should be lowercase letters only",
                        self.0
                    ),
                )
                .with_suggestion("Use lowercase ISO 639-3 codes (e.g., eng not ENG)"),
            );
        }

        if is_three_lowercase && is_disallowed_placeholder_language_code(self.as_str()) {
            errors.report(
                ParseError::new(
                    ErrorCode::InvalidLanguageCode,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new(self.as_str(), 0..self.0.len(), "language_code"),
                    format!("Language code '{}' is a disallowed placeholder", self.0),
                )
                .with_suggestion(
                    "Use a valid ISO 639-3 code (e.g., eng, spa, deu) in @Languages and @ID",
                ),
            );
        }

        // Check ISO 639-3 membership for codes that pass format checks.
        if is_three_lowercase
            && !is_disallowed_placeholder_language_code(self.as_str())
            && !super::iso639::is_valid_iso639_3(self.as_str())
        {
            errors.report(
                ParseError::new(
                    ErrorCode::InvalidLanguageCode,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new(self.as_str(), 0..self.0.len(), "language_code"),
                    format!(
                        "Language code '{}' is not in the ISO 639-3 registry",
                        self.0
                    ),
                )
                .with_suggestion(
                    "Use a valid ISO 639-3 code. See https://iso639-3.sil.org/ for the full list",
                ),
            );
        }
    }
}

/// Rejects obvious placeholder values often used in synthetic examples.
///
/// This helper is intentionally conservative and only blocks values that are
/// almost certainly placeholders rather than real language codes.
fn is_disallowed_placeholder_language_code(code: &str) -> bool {
    matches!(code, "xyz" | "xxx" | "yyy" | "zzz")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accepts_valid_code() {
        let code = LanguageCode::new("eng").expect("non-empty literal");
        assert_eq!(code.as_str(), "eng");
    }

    #[test]
    fn new_rejects_empty_string() {
        assert_eq!(LanguageCode::new(""), Err(LanguageCodeError::Empty));
    }

    #[test]
    fn try_from_str_rejects_empty_string() {
        let result: Result<LanguageCode, LanguageCodeError> = "".try_into();
        assert_eq!(result, Err(LanguageCodeError::Empty));
    }

    #[test]
    fn try_from_str_accepts_valid_code() {
        let code: LanguageCode = "spa".try_into().expect("non-empty literal");
        assert_eq!(code.as_str(), "spa");
    }

    #[test]
    fn empty_produces_undetermined() {
        let code = LanguageCode::empty();
        assert_eq!(code.as_str(), "und");
        assert!(code.is_undetermined());
    }

    #[test]
    fn regular_code_is_not_undetermined() {
        let code = LanguageCode::new("eng").expect("non-empty literal");
        assert!(!code.is_undetermined());
    }

    /// Helper to validate a language code and collect errors.
    fn validate_code(code: &str) -> Vec<crate::ParseError> {
        let lc = LanguageCode::new(code).expect("test call sites pass non-empty literals");
        let ctx = ValidationContext::new();
        let errors = crate::ErrorCollector::new();
        lc.validate(&ctx, &errors);
        errors.into_vec()
    }

    #[test]
    fn validate_rejects_non_iso639_3_code() {
        // "cye" is 3 lowercase letters but NOT in ISO 639-3.
        let errors = validate_code("cye");
        assert!(
            errors.iter().any(|e| e.code.as_str() == "E519"),
            "Expected E519 for non-ISO 639-3 code 'cye', got: {:?}",
            errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn validate_accepts_valid_iso639_3_code() {
        let errors = validate_code("eng");
        assert!(
            errors.is_empty(),
            "Expected no errors for 'eng', got: {:?}",
            errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn validate_accepts_valid_uncommon_iso639_3_code() {
        // "nle" (East Nyala) is a valid ISO 639-3 code, even if suspicious.
        let errors = validate_code("nle");
        assert!(
            errors.is_empty(),
            "Expected no errors for valid ISO 639-3 code 'nle', got: {:?}",
            errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>()
        );
    }

    /// Deserialization is deliberately LENIENT (parse-don't-validate): an empty
    /// code deserializes successfully via the `#[serde(transparent)]` derive, even
    /// though `new("")` rejects it for programmatic construction. This mirrors the
    /// sibling `NonEmptyString` (see its `test_deserialize_empty_allowed_but_invalid`):
    /// the `from-json` ingress reconstructs the model faithfully and the separate
    /// `Validate` pass flags domain violations (see the companion test below).
    ///
    /// DO NOT "fix" this by giving `LanguageCode` a strict custom `Deserialize`
    /// that rejects empty: that was tried and reverted (2026-07-04, Franklin's
    /// call) as inconsistent with the codebase's serde-boundary convention.
    #[test]
    fn deserialize_empty_is_lenient() {
        let code: LanguageCode =
            serde_json::from_str("\"\"").expect("deserialize is lenient: empty is accepted");
        assert_eq!(code.as_str(), "");
    }

    /// The lenient-deserialize convention pairs with a strict `Validate`: an empty
    /// (or otherwise malformed) code that entered via `from-json` is caught by
    /// validation, not by the deserializer. This is the parse-don't-validate split
    /// that makes the leniency above safe.
    #[test]
    fn deserialize_empty_is_flagged_by_validate() {
        let code: LanguageCode =
            serde_json::from_str("\"\"").expect("lenient deserialize accepts empty");
        let ctx = ValidationContext::new();
        let errors = crate::ErrorCollector::new();
        code.validate(&ctx, &errors);
        assert!(
            !errors.into_vec().is_empty(),
            "an empty language code must be flagged by the Validate pass (length check)"
        );
    }
}
