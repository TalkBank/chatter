//! Typed model for `%tim` (timing) dependent tier.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Timing_Tier>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

use crate::model::NonEmptyString;
use crate::model::TimeValue;
use crate::model::header::parse_time_value;

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
/// `Unsupported` and flagged by validation (E603).
/// A tier line that declares nothing is [`Self::Empty`], which is neither of
/// those: it is not a time and it is not free text, so it gets its own state
/// rather than an invented payload. E756 judges it.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Timing_Tier>
#[derive(Clone, Debug, PartialEq, SemanticEq, SpanShift, ValidationTagged)]
pub enum TimTier {
    /// Structured time-like content.
    Parsed {
        /// Structured time segments extracted from the text.
        #[span_shift(skip)]
        segments: Vec<TimSegment>,
        /// Raw text payload preserved for roundtrip.
        #[span_shift(skip)]
        content: NonEmptyString,
        /// Source span for error reporting.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        span: crate::Span,
    },
    /// Non-time content (free text like "afternoon session").
    Unsupported {
        /// Raw text payload.
        #[span_shift(skip)]
        content: NonEmptyString,
        /// Source span for error reporting.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        span: crate::Span,
    },
    /// A `%tim:` line with nothing after the separator.
    ///
    /// A third state rather than an empty payload on the other two: an empty
    /// `%tim` is not a time that failed to parse (`Unsupported`) and not a time
    /// (`Parsed`), it is a declaration that was never made. Both other variants
    /// hold a [`NonEmptyString`], so before this variant existed the re2c
    /// backend had to lower an empty `%tim:` to an `Unsupported` DEPENDENT TIER
    /// (E605, "unsupported dependent tier"), losing the tier's identity to
    /// report a code about the tier NAME on a file whose tier name is fine.
    ///
    /// Tagged CLEAN explicitly. `ValidationTagged`'s fallback reads the
    /// severity off the variant's NAME (a suffix of `Error` / `Warning` /
    /// `Unsupported`), which would give the right answer here by accident and
    /// the wrong one under a rename. E603 is about a `%tim` body that is not a
    /// time; an absent body is not a malformed time, it is E756's business, and
    /// letting E603 fire would print `Invalid %tim tier format: ''`.
    #[validation_tag(clean)]
    Empty {
        /// Source span for error reporting.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        span: crate::Span,
    },
}

impl TimTier {
    /// A `%tim` tier that declares nothing.
    ///
    /// Named rather than reached by passing an empty string to
    /// [`Self::from_text`], which cannot express it: both content-bearing
    /// variants hold a [`NonEmptyString`]. The only legitimate callers are the
    /// parsers, which met a `%tim:` line with no body and must say so.
    ///
    /// The result is INVALID CHAT, and deliberately representable anyway:
    /// recovery is not validity, and a parser that cannot express what the file
    /// says is a parser that will invent something instead.
    #[must_use]
    pub fn empty() -> Self {
        Self::Empty {
            span: crate::Span::DUMMY,
        }
    }

    /// The raw text this tier DECLARED, or `None` when it declared nothing.
    ///
    /// Distinct from [`Self::as_str`], which flattens the empty case to `""` so
    /// `Display` and the serializer have something to write. This accessor is
    /// what `DependentTier::empty_content_span` asks, so the "declared nothing"
    /// question is answered by the type rather than re-derived from a string.
    #[must_use]
    pub fn declared_content(&self) -> Option<&str> {
        match self {
            Self::Parsed { content, .. } | Self::Unsupported { content, .. } => {
                Some(content.as_str())
            }
            Self::Empty { .. } => None,
        }
    }

    /// Parse a `%tim` tier body, classifying as `Parsed` or `Unsupported`.
    ///
    /// Cannot produce [`Self::Empty`]: the argument is a [`NonEmptyString`], so
    /// a caller with nothing to hand over reaches for [`Self::empty`] instead.
    pub fn from_text(content: NonEmptyString) -> Self {
        if let Some(segments) = parse_tim_segments(content.as_str()) {
            Self::Parsed {
                segments,
                content,
                span: crate::Span::DUMMY,
            }
        } else {
            Self::Unsupported {
                content,
                span: crate::Span::DUMMY,
            }
        }
    }

    /// Sets the source span.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        match &mut self {
            Self::Parsed { span: s, .. } => *s = span,
            Self::Unsupported { span: s, .. } => *s = span,
            Self::Empty { span: s } => *s = span,
        }
        self
    }

    /// Returns the raw text content, `""` for [`Self::Empty`].
    ///
    /// The empty case flattens here because `Display` and the CHAT serializer
    /// need a string, and `%tim:` with nothing after it is what an empty tier
    /// writes back. Callers asking whether anything was DECLARED want
    /// [`Self::declared_content`], which does not flatten.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Parsed { content, .. } => content.as_str(),
            Self::Unsupported { content, .. } => content.as_str(),
            Self::Empty { .. } => "",
        }
    }

    /// Returns the source span.
    pub fn span(&self) -> crate::Span {
        match self {
            Self::Parsed { span, .. } => *span,
            Self::Unsupported { span, .. } => *span,
            Self::Empty { span } => *span,
        }
    }

    /// Returns the structured time segments (none for `Unsupported` or `Empty`).
    pub fn segments(&self) -> &[TimSegment] {
        match self {
            Self::Parsed { segments, .. } => segments,
            Self::Unsupported { .. } | Self::Empty { .. } => &[],
        }
    }
}

/// Parse whitespace-separated time tokens from `%tim` content.
///
/// Each token is either a single time or a hyphen-separated range.
fn parse_tim_segments(s: &str) -> Option<Vec<TimSegment>> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    for token in trimmed.split_whitespace() {
        // Try as a range (hyphen-separated).
        if let Some((left, right)) = token.split_once('-') {
            let start = parse_time_value(left)?;
            let end = parse_time_value(right)?;
            segments.push(TimSegment::Range { start, end });
        } else {
            let tv = parse_time_value(token)?;
            segments.push(TimSegment::Single(tv));
        }
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
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
        Ok(match NonEmptyString::new(&s) {
            Ok(content) => Self::from_text(content),
            Err(_) => Self::empty(),
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

    #[test]
    fn parsed_single_time() {
        let content = NonEmptyString::new("7:55").unwrap();
        let tim = TimTier::from_text(content);
        assert!(matches!(tim, TimTier::Parsed { .. }));
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
        let content = NonEmptyString::new("00:01:30-00:02:00").unwrap();
        let tim = TimTier::from_text(content);
        assert_eq!(tim.segments().len(), 1);
        assert!(matches!(tim.segments()[0], TimSegment::Range { .. }));
    }

    #[test]
    fn parsed_multiple_tokens() {
        let content = NonEmptyString::new("7:55 00:01:30-00:02:00").unwrap();
        let tim = TimTier::from_text(content);
        assert_eq!(tim.segments().len(), 2);
    }

    #[test]
    fn parsed_bare_seconds() {
        let content = NonEmptyString::new("45").unwrap();
        let tim = TimTier::from_text(content);
        assert!(matches!(tim, TimTier::Parsed { .. }));
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
        let content = NonEmptyString::new("afternoon session").unwrap();
        let tim = TimTier::from_text(content);
        assert!(matches!(tim, TimTier::Unsupported { .. }));
        assert_eq!(tim.segments().len(), 0);
    }

    #[test]
    fn roundtrip_text() {
        let input = "00:01:30-00:02:00";
        let content = NonEmptyString::new(input).unwrap();
        let tim = TimTier::from_text(content);
        assert_eq!(tim.as_str(), input);
    }
}
