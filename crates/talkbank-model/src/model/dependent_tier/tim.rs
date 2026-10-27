//! Typed model for `%tim` (timing) dependent tier.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Timing_Tier>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

use crate::model::TimeValue;
use crate::model::header::parse_time_value;
use crate::model::{NonEmptyString, RecoveryText};

/// A time segment in a `%tim` tier: single time or range.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub enum TimSegment {
    /// A single time point (e.g. `7:55`).
    Single(TimeValue),
    /// A range between two time points (e.g. `00:01:30-00:02:00`).
    Range {
        /// Start of the range.
        start: TimeValue,
        /// End of the range.
        end: TimeValue,
    },
}

/// Timing tier content from `%tim`.
///
/// Time-like content (tokens with colons and digits, e.g. `7:55` or
/// `00:01:30-00:02:00`) is parsed into structured `TimSegment`s.
/// Free-text descriptions (e.g. `afternoon session`) are stored as
/// unsupported and flagged by validation (E603).
/// A tier line that declares nothing has a separate empty state: it is neither
/// a time nor free text. E756 judges it. Construction owns this classification;
/// callers cannot supply or mutate derived segments independently of the text.
///
/// The parsed classification and its derived segments must agree with the text.
///
/// ```compile_fail
/// use talkbank_model::model::{NonEmptyString, ValidationTagged};
/// use talkbank_model::model::dependent_tier::TimTier;
/// let tier = TimTier::Parsed {
///     segments: Vec::new(),
///     content: NonEmptyString::new("afternoon session").unwrap(),
///     location: talkbank_model::Location::Unlocated,
/// };
/// assert!(tier.has_validation_issue());
/// ```
///
/// ```compile_fail
/// use talkbank_model::model::{NonEmptyString, RecoveryText, ValidationTagged};
/// use talkbank_model::model::dependent_tier::TimTier;
/// let mut tier = TimTier::from_text(
///     talkbank_model::Location::Unlocated,
///     RecoveryText::new("7:55").unwrap(),
/// );
/// if let TimTier::Parsed { content, .. } = &mut tier {
///     *content = NonEmptyString::new("afternoon session").unwrap();
/// }
/// assert!(tier.has_validation_issue());
/// ```
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Timing_Tier>
#[derive(Clone, Debug, PartialEq, SemanticEq, SpanShift)]
pub struct TimTier {
    /// Classification and derived timing have one constructor and no mutation route.
    #[span_shift(skip)]
    state: TimState,
    /// Source location does not affect the declared timing semantics.
    #[semantic_eq(skip)]
    location: crate::Location,
}

/// Only `TimTier::from_text` constructs a content-bearing state.
#[derive(Clone, Debug, PartialEq, SemanticEq, ValidationTagged)]
enum TimState {
    Parsed {
        segments: ParsedTimSegments,
        content: NonEmptyString,
    },
    Unsupported {
        content: NonEmptyString,
    },
    /// E756 owns absent content; it is not an E603 unsupported-time payload.
    #[validation_tag(clean)]
    Empty,
}

impl crate::model::ValidationTagged for TimTier {
    fn validation_tag(&self) -> crate::model::ValidationTag {
        crate::model::ValidationTagged::validation_tag(&self.state)
    }
}

impl TimTier {
    /// Retain a `%tim` tier that declares nothing, for E756 validation.
    /// Empty recovery is distinct from non-time text and does not invent a time.
    #[must_use]
    pub fn empty(location: impl Into<crate::Location>) -> Self {
        Self::from_text(location, RecoveryText::Empty)
    }

    /// The raw text this tier declared, or `None` when it declared nothing.
    /// Unlike `as_str`, this preserves the distinction between absent content
    /// and a content-bearing tier for dependent-tier validation.
    #[must_use]
    pub fn declared_content(&self) -> Option<&str> {
        match &self.state {
            TimState::Parsed { content, .. } | TimState::Unsupported { content } => {
                Some(content.as_str())
            }
            TimState::Empty => None,
        }
    }

    /// Classify the body and derive its time segments together.
    /// Explicit empty recovery remains empty; nonempty unsupported text is
    /// retained for E603 rather than promoted to parsed timing.
    pub fn from_text(location: impl Into<crate::Location>, content: RecoveryText) -> Self {
        let state = match content {
            RecoveryText::Empty => TimState::Empty,
            RecoveryText::NonEmpty(content) => match ParsedTimSegments::parse(&content) {
                Some(segments) => TimState::Parsed { segments, content },
                None => TimState::Unsupported { content },
            },
        };
        Self {
            state,
            location: location.into(),
        }
    }

    /// Raw content, or an empty string for a declaration with no body.
    /// Consumers deciding absence use `declared_content` instead.
    pub fn as_str(&self) -> &str {
        match &self.state {
            TimState::Parsed { content, .. } | TimState::Unsupported { content } => {
                content.as_str()
            }
            TimState::Empty => "",
        }
    }

    /// Re-express the tier's location without changing its admitted content.
    #[must_use]
    pub fn map_location(mut self, project: impl FnOnce(crate::Span) -> crate::Span) -> Self {
        self.location = self.location.map(project);
        self
    }

    /// Returns the source location.
    pub fn location(&self) -> crate::Location {
        self.location
    }

    /// Structured times, empty for unsupported text or an absent body.
    pub fn segments(&self) -> &[TimSegment] {
        match &self.state {
            TimState::Parsed { segments, .. } => segments.as_slice(),
            TimState::Unsupported { .. } | TimState::Empty => &[],
        }
    }
}

/// A nonempty sequence admitted atomically from the tier's complete text.
/// Private storage cannot be emptied after its first segment was admitted.
#[derive(Clone, Debug, PartialEq, SemanticEq)]
struct ParsedTimSegments(Vec<TimSegment>);

impl ParsedTimSegments {
    fn parse(content: &NonEmptyString) -> Option<Self> {
        let mut tokens = content.as_str().split_whitespace();
        // Nonempty lexical text may still contain only whitespace.
        let first = parse_tim_segment(tokens.next()?)?;
        let mut segments = vec![first];
        for token in tokens {
            segments.push(parse_tim_segment(token)?);
        }
        Some(Self(segments))
    }

    fn as_slice(&self) -> &[TimSegment] {
        &self.0
    }
}

/// Admit both endpoints before constructing a range segment.
fn parse_tim_segment(token: &str) -> Option<TimSegment> {
    match token.split_once('-') {
        Some((left, right)) => Some(TimSegment::Range {
            start: parse_time_value(left)?,
            end: parse_time_value(right)?,
        }),
        None => parse_time_value(token).map(TimSegment::Single),
    }
}

impl std::fmt::Display for TimTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- Serde: serialize/deserialize as plain string for backward compat ---

impl Serialize for TimTier {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TimTier {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // The wire form is the raw text, and `""` is not ambiguous: every
        // content-bearing variant holds a `NonEmptyString`, so an empty string
        // can only have come from `Empty`. Reading it back as `Empty` rather
        // than as an error is what makes the round trip lossless now that the
        // state exists; it used to be rejected because it was unrepresentable.
        let s = String::deserialize(deserializer)?;
        Ok(match RecoveryText::new(&s) {
            Ok(content) => Self::from_text(crate::Location::Unlocated, content),
            Err(_) => Self::empty(crate::Location::Unlocated),
        })
    }
}

impl JsonSchema for TimTier {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TimTier".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ValidationTagged;

    #[test]
    fn explicit_empty_recovery_has_no_unsupported_time_diagnostic_state() {
        let tier = TimTier::from_text(crate::Location::Unlocated, RecoveryText::Empty);
        assert_eq!(tier.declared_content(), None);
        assert_eq!(serde_json::to_string(&tier).unwrap(), "\"\"");
    }

    #[test]
    fn parsed_single_time() {
        let content = RecoveryText::new("7:55").unwrap();
        let tim = TimTier::from_text(crate::Location::Unlocated, content);
        assert!(!tim.has_validation_issue());
        assert_eq!(tim.segments().len(), 1);
        match &tim.segments()[0] {
            TimSegment::Single(tv) => {
                assert_eq!((tv.hours, tv.minutes, tv.seconds), (0, 7, 55));
            }
            other => panic!("expected Single, got {:?}", other),
        }
    }

    #[test]
    fn parsed_range() {
        let content = RecoveryText::new("00:01:30-00:02:00").unwrap();
        let tim = TimTier::from_text(crate::Location::Unlocated, content);
        assert_eq!(tim.segments().len(), 1);
        assert!(matches!(tim.segments()[0], TimSegment::Range { .. }));
    }

    #[test]
    fn parsed_multiple_tokens() {
        let content = RecoveryText::new("7:55 00:01:30-00:02:00").unwrap();
        let tim = TimTier::from_text(crate::Location::Unlocated, content);
        assert_eq!(tim.segments().len(), 2);
    }

    #[test]
    fn parsed_bare_seconds() {
        let content = RecoveryText::new("45").unwrap();
        let tim = TimTier::from_text(crate::Location::Unlocated, content);
        assert!(!tim.has_validation_issue());
        assert_eq!(tim.segments().len(), 1);
        match &tim.segments()[0] {
            TimSegment::Single(tv) => {
                assert_eq!((tv.hours, tv.minutes, tv.seconds), (0, 0, 45));
            }
            other => panic!("expected Single, got {:?}", other),
        }
    }

    #[test]
    fn unsupported_free_text() {
        let content = RecoveryText::new("afternoon session").unwrap();
        let tim = TimTier::from_text(crate::Location::Unlocated, content);
        assert!(tim.has_validation_issue());
        assert_eq!(tim.segments().len(), 0);
    }

    #[test]
    fn wire_readback_preserves_classification_and_derived_segments() {
        use crate::model::{SemanticEq, ValidationTag};
        for (text, tag, segments) in [
            ("", ValidationTag::Clean, 0),
            ("afternoon session", ValidationTag::Warning, 0),
            ("   ", ValidationTag::Warning, 0),
            ("7:55", ValidationTag::Clean, 1),
            ("00:01:30-00:02:00", ValidationTag::Clean, 1),
            ("7:55 8:00", ValidationTag::Clean, 2),
            (" 7:55 ", ValidationTag::Clean, 1),
        ] {
            let content = match RecoveryText::new(text) {
                Ok(content) => content,
                Err(_) => RecoveryText::Empty,
            };
            let tier = TimTier::from_text(crate::Span::new(5, 5 + text.len()), content);
            let encoded = serde_json::to_string(&tier).unwrap();
            assert_eq!(encoded, serde_json::to_string(text).unwrap());
            let restored: TimTier = serde_json::from_str(&encoded).unwrap();
            assert_eq!(restored.validation_tag(), tag);
            assert_eq!(restored.segments().len(), segments);
            assert_eq!(restored.location(), crate::Location::Unlocated);
            assert!(tier.semantic_eq(&restored));
        }
    }

    #[test]
    fn roundtrip_text() {
        let input = "00:01:30-00:02:00";
        let content = RecoveryText::new(input).unwrap();
        let tim = TimTier::from_text(crate::Location::Unlocated, content);
        assert_eq!(tim.as_str(), input);
    }
}
