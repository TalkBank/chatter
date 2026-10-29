//! Admitted non-empty text and explicit empty-text recovery.

use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCode, ErrorSink, ParseError, Severity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use talkbank_derive::{SemanticEq, SpanShift};

/// Text proven non-empty at construction, including deserialization.
/// Whitespace is non-empty; CHAT lexical rules are separate.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "String")]
pub struct NonEmptyString(smol_str::SmolStr);

impl JsonSchema for NonEmptyString {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "NonEmptyString".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string", "minLength": 1 })
    }
}

/// A lexical field retained for contextual validation, including observed emptiness.
///
/// JSON remains a string. An empty payload becomes `Empty`; it never constructs
/// a `NonEmptyString`. Rendering preserves the observed text and does not admit it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(from = "String")]
pub enum RecoveryText {
    /// Text with established non-emptiness, not necessarily valid CHAT syntax.
    NonEmpty(NonEmptyString),
    /// The input explicitly contained no text.
    Empty,
}

impl Serialize for RecoveryText {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl From<String> for RecoveryText {
    fn from(text: String) -> Self {
        match NonEmptyString::new(text) {
            Ok(text) => Self::NonEmpty(text),
            Err(EmptyText) => Self::Empty,
        }
    }
}

impl From<NonEmptyString> for RecoveryText {
    fn from(text: NonEmptyString) -> Self {
        Self::NonEmpty(text)
    }
}

impl TryFrom<RecoveryText> for NonEmptyString {
    type Error = EmptyText;
    fn try_from(text: RecoveryText) -> Result<Self, Self::Error> {
        match text {
            RecoveryText::NonEmpty(text) => Ok(text),
            RecoveryText::Empty => Err(EmptyText),
        }
    }
}

impl TryFrom<String> for NonEmptyString {
    type Error = EmptyText;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl From<NonEmptyString> for String {
    fn from(text: NonEmptyString) -> Self {
        text.into_inner()
    }
}

impl From<RecoveryText> for String {
    fn from(text: RecoveryText) -> Self {
        text.into_inner()
    }
}

impl TryFrom<&str> for RecoveryText {
    type Error = EmptyText;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl RecoveryText {
    /// Admit non-empty input; use the explicit `Empty` variant for recovery.
    pub fn new(text: impl AsRef<str>) -> Result<Self, EmptyText> {
        NonEmptyString::new(text).map(Self::NonEmpty)
    }

    /// Compose text with an observed non-empty prefix.
    pub fn from_head_and_tail(head: char, tail: &str) -> Self {
        Self::NonEmpty(NonEmptyString::from_head_and_tail(head, tail))
    }

    /// Borrow the exact observed text. This does not establish non-emptiness.
    pub fn as_str(&self) -> &str {
        match self {
            Self::NonEmpty(text) => text.as_str(),
            Self::Empty => "",
        }
    }

    /// Consume the state and return its exact observed text.
    pub fn into_inner(self) -> String {
        match self {
            Self::NonEmpty(text) => text.into_inner(),
            Self::Empty => String::new(),
        }
    }
}

impl Deref for RecoveryText {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<str> for RecoveryText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Display for RecoveryText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why constructing a non-empty text newtype failed.
///
/// One shared type rather than one per newtype: the fact is the same in every
/// case (the input was empty), and the caller already knows which type it
/// asked for. A per-type error would carry no information the call site does
/// not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("text cannot be empty")]
pub struct EmptyText;

/// Borrowed text admitted as non-empty, without recovery or deserialization paths.
///
/// ```compile_fail,E0451
/// let forged = talkbank_model::model::NonEmptyStr("");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonEmptyStr<'a>(&'a str);

impl<'a> NonEmptyStr<'a> {
    /// Admit text without allocating. Whitespace is non-empty lexical text.
    pub fn new(text: &'a str) -> Result<Self, EmptyText> {
        if text.is_empty() {
            Err(EmptyText)
        } else {
            Ok(Self(text))
        }
    }

    /// Borrow the original text for its original lifetime.
    pub fn as_str(self) -> &'a str {
        self.0
    }

    /// Copy admitted text into owned storage without repeating admission.
    pub fn into_owned(self) -> NonEmptyString {
        NonEmptyString(smol_str::SmolStr::from(self.0))
    }
}

/// Validated non-empty static text for literal construction.
///
/// Unlike the recovery-capable [`RecoveryText`], this evidence cannot be
/// deserialized or constructed without checking non-emptiness.
///
/// ```compile_fail,E0451
/// let forged = talkbank_model::model::NonEmptyLiteral("");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonEmptyLiteral(&'static str);

impl NonEmptyLiteral {
    /// Admit static text, at compile time when used by `non_empty_literal!`.
    pub const fn new(text: &'static str) -> Result<Self, EmptyText> {
        if text.is_empty() {
            Err(EmptyText)
        } else {
            Ok(Self(text))
        }
    }
}

impl TryFrom<&str> for NonEmptyString {
    type Error = EmptyText;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl NonEmptyString {
    /// Create a new `NonEmptyString` if the input is non-empty.
    ///
    /// Whitespace-only strings are accepted because this type enforces lexical
    /// non-emptiness, while higher-level semantic rules decide whether blanks are meaningful.
    ///
    /// # Errors
    ///
    /// [`EmptyText`] for truly empty input (`""`). A `Result` rather than an
    /// `Option` because `None` discards the reason, and this constructor now
    /// has exactly one reason to fail that a caller may want to report.
    ///
    /// # Examples
    ///
    /// ```
    /// use talkbank_model::model::NonEmptyString;
    ///
    /// assert!(NonEmptyString::new("hello").is_ok());
    /// assert!(NonEmptyString::new("   ").is_ok());  // whitespace is valid
    /// assert!(NonEmptyString::new("").is_err());
    /// ```
    pub fn new(s: impl AsRef<str>) -> Result<Self, EmptyText> {
        NonEmptyStr::new(s.as_ref()).map(NonEmptyStr::into_owned)
    }

    /// A string that begins with `head`: non-empty by construction, so
    /// nothing is checked and nothing can fail. The one route to a
    /// `NonEmptyString` built from a possibly-empty tail, for a value the
    /// program composes from a known first character (a `%x` tier label,
    /// stored with its `x`) rather than reads from input.
    ///
    /// # Examples
    ///
    /// ```
    /// use talkbank_model::model::NonEmptyString;
    ///
    /// assert_eq!(NonEmptyString::from_head_and_tail('x', "mor").as_str(), "xmor");
    /// assert_eq!(NonEmptyString::from_head_and_tail('x', "").as_str(), "x");
    /// ```
    pub fn from_head_and_tail(head: char, tail: &str) -> Self {
        let mut text = String::with_capacity(head.len_utf8() + tail.len());
        text.push(head);
        text.push_str(tail);
        Self(smol_str::SmolStr::from(text))
    }

    /// This text followed by `tail`: non-empty because this is, so nothing is
    /// checked and nothing can fail. The append that keeps the proof.
    ///
    /// # Examples
    ///
    /// ```
    /// use talkbank_model::non_empty_literal;
    ///
    /// assert_eq!(non_empty_literal!("w").with_suffix("12").as_str(), "w12");
    /// ```
    #[must_use]
    pub fn with_suffix(&self, tail: &str) -> Self {
        let mut text = String::with_capacity(self.0.len() + tail.len());
        text.push_str(&self.0);
        text.push_str(tail);
        Self(smol_str::SmolStr::from(text))
    }

    /// The route the [`non_empty_literal!`](crate::non_empty_literal) macro
    /// takes after proving its literal non-empty at compile time. Direct
    /// callers must supply the same validated evidence; raw strings cannot
    /// bypass the check, including in release builds.
    ///
    /// ```compile_fail,E0308
    /// let _ = talkbank_model::model::NonEmptyString::from_checked_literal("");
    /// ```
    #[doc(hidden)]
    pub fn from_checked_literal(literal: NonEmptyLiteral) -> Self {
        Self(smol_str::SmolStr::new_static(literal.0))
    }

    /// Borrow the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return an owned `String`.
    pub fn into_inner(self) -> String {
        self.0.to_string()
    }
}

impl Deref for NonEmptyString {
    type Target = str;

    /// Borrows the stored text as `&str`.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for NonEmptyString {
    /// Borrows the stored text as `&str`.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NonEmptyString {
    /// Displays the stored string value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Validate for RecoveryText {
    /// Reports an error when deserialized content violates the non-empty invariant.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        match self {
            Self::NonEmpty(_) => return,
            Self::Empty => {}
        }

        let location = context.field_diagnostic_location();
        // DEFAULT: When no label is provided, describe the missing value as a generic string.
        let label = context.field_label.unwrap_or("string");
        // DEFAULT: Empty strings map to the canonical EmptyString error code.
        let code = context.field_error_code.unwrap_or(ErrorCode::EmptyString);

        errors.report(
            ParseError::new(
                code,
                Severity::Error,
                location,
                None,
                format!("{} cannot be empty", label),
            )
            .with_suggestion("Provide a non-empty value"),
        );
    }
}

/// A [`NonEmptyString`] from a string literal, proven non-empty by the
/// compiler: an empty literal is a compile error, so the value needs no
/// runtime check and no `Result`. A literal of another kind (a number, a
/// byte string) is refused where the typed constant is declared.
///
/// ```
/// use talkbank_model::non_empty_literal;
///
/// assert_eq!(non_empty_literal!("lemma").as_str(), "lemma");
/// ```
///
/// ```compile_fail,E0080
/// # use talkbank_model::non_empty_literal;
/// let _ = non_empty_literal!("");
/// ```
///
/// Caller-defined names cannot change admission or substitute recovery text.
///
/// ```
/// use talkbank_model::non_empty_literal;
/// enum CallerNames { Ok, Err }
/// use CallerNames::{Ok, Err};
/// let _ = (Ok, Err);
/// assert_eq!(non_empty_literal!("word").as_str(), "word");
/// ```
///
/// ```compile_fail,E0080
/// use talkbank_model::non_empty_literal;
/// macro_rules! panic {
///     ($($ignored:tt)*) => {
///         match talkbank_model::model::NonEmptyLiteral::new("substitute") {
///             ::core::result::Result::Ok(text) => text,
///             ::core::result::Result::Err(_) => ::core::panic!("invalid control"),
///         }
///     };
/// }
/// let _ = non_empty_literal!("");
/// ```
#[macro_export]
macro_rules! non_empty_literal {
    ($literal:literal) => {{
        const LITERAL: $crate::model::NonEmptyLiteral =
            match $crate::model::NonEmptyLiteral::new($literal) {
                ::core::result::Result::Ok(literal) => literal,
                ::core::result::Result::Err(_) => {
                    ::core::panic!("a non-empty literal is empty")
                }
            };
        $crate::model::NonEmptyString::from_checked_literal(LITERAL)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_is_explicit_and_cannot_be_admitted_as_nonempty() {
        let recovered: RecoveryText = serde_json::from_str("\"\"").unwrap();
        assert_eq!(recovered, RecoveryText::Empty);
        assert_eq!(NonEmptyString::try_from(recovered), Err(EmptyText));
        assert!(serde_json::from_str::<NonEmptyString>("\"\"").is_err());
        for raw in ["word", "話", " ", "😀"] {
            let wire = serde_json::to_string(raw).unwrap();
            let recovered: RecoveryText = serde_json::from_str(&wire).unwrap();
            let RecoveryText::NonEmpty(admitted) = recovered else {
                panic!("nonempty input")
            };
            assert_eq!(admitted.as_str(), raw);
            assert_eq!(serde_json::to_string(&admitted).unwrap(), wire);
            assert_eq!(
                serde_json::from_str::<NonEmptyString>(&wire).unwrap(),
                admitted
            );
        }
    }

    #[test]
    fn borrowed_admission_preserves_text_when_owned() {
        assert_eq!(NonEmptyStr::new(""), Err(EmptyText));
        let recovered: RecoveryText = serde_json::from_str("\"\"").unwrap();
        assert_eq!(NonEmptyStr::new(recovered.as_str()), Err(EmptyText));
        for text in ["word", "話", " ", "😀"] {
            let admitted = NonEmptyStr::new(text).unwrap();
            assert_eq!(admitted.as_str(), text);
            assert_eq!(admitted.into_owned().as_str(), text);
        }
    }

    #[test]
    fn literal_admission() {
        assert_eq!(NonEmptyLiteral::new(""), Err(EmptyText));
        for text in ["word", "\u{8a71}", " "] {
            assert!(NonEmptyLiteral::new(text).is_ok());
        }
        assert_eq!(crate::non_empty_literal!("lemma").as_str(), "lemma");
        assert_eq!(crate::non_empty_literal!("\u{8a71}").as_str(), "\u{8a71}");
        assert_eq!(crate::non_empty_literal!(" ").as_str(), " ");
    }

    /// Non-empty inputs construct successfully.
    #[test]
    fn test_construction() {
        assert!(RecoveryText::new("hello").is_ok());
        assert!(RecoveryText::new("a").is_ok());
        assert!(RecoveryText::new(" x ").is_ok());
    }

    /// Empty input is rejected by `new()`.
    #[test]
    fn test_empty_rejected() {
        assert!(RecoveryText::new("").is_err());
    }

    /// Whitespace-only input is still non-empty and therefore valid.
    #[test]
    fn test_whitespace_allowed() {
        // Whitespace-only strings are valid (not empty)
        assert!(RecoveryText::new("   ").is_ok());
        assert!(RecoveryText::new("\t\n").is_ok());
    }

    /// `Deref<str>` behavior is available.
    #[test]
    fn test_deref() -> Result<(), String> {
        let s = RecoveryText::new("hello").map_err(|error| error.to_string())?;
        assert_eq!(s.as_str().get(0..2), Some("he"));
        assert_eq!(s.len(), 5);
        Ok(())
    }

    /// `Display` prints inner contents.
    #[test]
    fn test_display() -> Result<(), String> {
        let s = RecoveryText::new("hello").map_err(|error| error.to_string())?;
        assert_eq!(format!("{}", s), "hello");
        Ok(())
    }

    /// Serde roundtrip preserves values.
    #[test]
    fn test_serde_roundtrip() -> Result<(), String> {
        let s = RecoveryText::new("hello").map_err(|error| error.to_string())?;
        let json = serde_json::to_string(&s)
            .map_err(|err| format!("Failed to serialize RecoveryText: {err}"))?;
        assert_eq!(json, "\"hello\"");

        let deserialized: RecoveryText = serde_json::from_str(&json)
            .map_err(|err| format!("Failed to deserialize RecoveryText: {err}"))?;
        assert_eq!(deserialized, s);
        Ok(())
    }

    /// Empty payload may deserialize and is caught by validation later.
    #[test]
    fn test_deserialize_empty_allowed_but_invalid() -> Result<(), String> {
        let s: RecoveryText = serde_json::from_str(r#""""#)
            .map_err(|err| format!("Failed to deserialize empty RecoveryText: {err}"))?;
        assert!(s.as_str().is_empty());
        Ok(())
    }

    /// Whitespace payload deserializes as valid non-empty content.
    #[test]
    fn test_deserialize_whitespace_valid() -> Result<(), String> {
        let s: RecoveryText = serde_json::from_str(r#""   ""#)
            .map_err(|err| format!("Failed to deserialize whitespace RecoveryText: {err}"))?;
        assert!(!s.as_str().is_empty());
        assert_eq!(s.as_str(), "   ");
        Ok(())
    }

    /// Validation reports `EmptyString` for empty payloads.
    #[test]
    fn test_validate_empty_reports_error() -> Result<(), String> {
        use crate::ErrorCollector;
        use crate::validation::ValidationContext;

        let s: RecoveryText = serde_json::from_str(r#""""#)
            .map_err(|err| format!("Failed to deserialize empty RecoveryText: {err}"))?;
        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        s.validate(&ctx, &errors);

        let error_vec = errors.into_vec();
        assert!(
            error_vec
                .iter()
                .any(|e| e.code == crate::ErrorCode::EmptyString),
            "Expected E003 for empty RecoveryText, got: {:#?}",
            error_vec
        );
        Ok(())
    }
}
