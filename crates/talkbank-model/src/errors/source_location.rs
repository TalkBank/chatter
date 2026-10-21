//! Source location types and span-to-location conversions.
//!
//! Provides [`DiagnosticLocation`] for mapping byte offsets to line/column positions,
//! [`ErrorLabel`] for secondary diagnostic spans, [`Severity`] for error classification,
//! and the [`ErrorVec`] type alias for small-vec-optimized error collections.

use super::{InvertedSpan, Location, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::fmt;
use std::num::NonZeroUsize;

use super::parse_error::ParseError;

// =============================================================================
// Error Vector Optimization
// =============================================================================

/// A secondary label attached to a diagnostic, pointing to a related source span.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorLabel {
    /// Byte range of the labeled span.
    #[serde(flatten)]
    pub span: Span,
    /// Descriptive message for this label.
    pub message: String,
}

impl ErrorLabel {
    /// Create a new error label pointing to a related source span.
    ///
    /// # Parameters
    ///
    /// - `span`: Byte range of the related source location.
    /// - `message`: Descriptive text rendered alongside the underlined span
    ///   (e.g., "first declared here", "expected this type").
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

/// Optimized error vector that stores up to 2 errors inline before heap allocation.
pub type ErrorVec = SmallVec<[ParseError; 2]>;

/// Severity class for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Invalid CHAT syntax - must be fixed
    Error,
    /// Valid but deprecated/stylistic issue - should be fixed
    Warning,
}

impl fmt::Display for Severity {
    /// Render severity as the lowercase label expected by CLI/UI reporters.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

/// A one-based line and column, always known together.
///
/// Columns count bytes from the start of the line, not characters or UTF-16
/// units; LSP positions use their own conversion. Both numbers are nonzero by
/// type, so a zero line or a line without a column cannot be represented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct LineColumn {
    /// Line number in the source file (1-indexed).
    pub line: NonZeroUsize,
    /// Byte column within the line (1-indexed).
    pub column: NonZeroUsize,
}

impl LineColumn {
    /// A position from one-based numbers, or `None` if either is zero.
    pub const fn one_based(line: usize, column: usize) -> Option<Self> {
        match (NonZeroUsize::new(line), NonZeroUsize::new(column)) {
            (Some(line), Some(column)) => Some(Self { line, column }),
            _ => None,
        }
    }

    /// The one-based position of a zero-based line and byte column, as
    /// [`crate::LineMap`] reports them.
    pub const fn from_zero_based(line: usize, column: usize) -> Self {
        Self {
            line: NonZeroUsize::MIN.saturating_add(line),
            column: NonZeroUsize::MIN.saturating_add(column),
        }
    }

    /// The line and column of byte `offset` in `source`.
    ///
    /// Performs an allocation-free scan up to the requested byte offset. There
    /// is no address-keyed cache: a buffer can be edited or reused between calls.
    /// For repeated lookups, build one [`crate::SourceIndex`] and use its
    /// `line_col_of` method (zero-indexed), or enrich a batch of diagnostics.
    /// An offset past the end of `source` keeps counting columns on the last
    /// line, so callers with untrusted offsets bound them first.
    pub fn of_offset(offset: usize, source: &str) -> Self {
        let mut line = NonZeroUsize::MIN;
        let mut line_start = 0;
        for (index, byte) in source.bytes().take(offset).enumerate() {
            if byte == b'\n' {
                line = line.saturating_add(1);
                line_start = index + 1;
            }
        }
        // `offset >= line_start` here, and adding one makes it nonzero.
        let column = NonZeroUsize::MIN.saturating_add(offset.saturating_sub(line_start));
        Self { line, column }
    }
}

/// Where a diagnostic points: a byte span in the source (with its line and
/// column when they have been computed), or nowhere.
///
/// `Unlocated` is the honest answer for a diagnostic about a value that has
/// no source position, such as a node built programmatically or read back
/// from AST JSON. It is a variant, never a `0..0` span, because `0..0` is also
/// the real zero-width position at the start of a file.
///
/// The wire form is tagged: `{"kind":"known","start":..,"end":..}` (plus
/// `line` and `column` together when known) or `{"kind":"unlocated"}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "DiagnosticLocationWire", into = "DiagnosticLocationWire")]
pub enum DiagnosticLocation {
    /// The diagnostic points at a byte span of the source.
    Known {
        /// The byte span.
        span: Span,
        /// The span start's line and column, when computed.
        position: Option<LineColumn>,
    },
    /// The diagnostic concerns a value with no source position.
    Unlocated,
}

/// The flat, tagged wire form of a [`DiagnosticLocation`].
///
/// Decoding goes through `TryFrom`, so a reversed span, a zero line or
/// column, or a line without its column (or the reverse) is refused rather
/// than read as "no position". schemars follows `try_from`/`into`, so the
/// published schema describes this shape.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DiagnosticLocationWire {
    /// The diagnostic points at a byte span of the source.
    Known {
        /// Start byte offset (0-indexed, inclusive).
        start: usize,
        /// End byte offset (0-indexed, exclusive); `start <= end`.
        end: usize,
        /// Line of `start` (1-indexed); present exactly when `column` is.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<NonZeroUsize>,
        /// Byte column of `start` within its line (1-indexed); present
        /// exactly when `line` is.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        column: Option<NonZeroUsize>,
    },
    /// The diagnostic concerns a value with no source position.
    Unlocated,
}

/// Why a wire diagnostic location was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DiagnosticLocationWireError {
    /// The span ends before it starts.
    #[error(transparent)]
    Inverted(#[from] InvertedSpan),
    /// Only one of `line` and `column` was present.
    #[error("a diagnostic location carries both line and column, or neither")]
    HalfPosition,
}

impl TryFrom<DiagnosticLocationWire> for DiagnosticLocation {
    type Error = DiagnosticLocationWireError;

    fn try_from(wire: DiagnosticLocationWire) -> Result<Self, Self::Error> {
        match wire {
            DiagnosticLocationWire::Known {
                start,
                end,
                line,
                column,
            } => {
                let span = Span::try_new(start, end)?;
                let position = match (line, column) {
                    (Some(line), Some(column)) => Some(LineColumn { line, column }),
                    (None, None) => None,
                    (Some(_), None) | (None, Some(_)) => {
                        return Err(DiagnosticLocationWireError::HalfPosition);
                    }
                };
                Ok(Self::Known { span, position })
            }
            DiagnosticLocationWire::Unlocated => Ok(Self::Unlocated),
        }
    }
}

impl From<DiagnosticLocation> for DiagnosticLocationWire {
    fn from(location: DiagnosticLocation) -> Self {
        match location {
            DiagnosticLocation::Known { span, position } => Self::Known {
                start: span.start(),
                end: span.end(),
                line: position.map(|p| p.line),
                column: position.map(|p| p.column),
            },
            DiagnosticLocation::Unlocated => Self::Unlocated,
        }
    }
}

/// Errors for source-based location construction.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error,
)]
pub enum SourceLocationError {
    /// A byte offset exceeds the source text length.
    #[error("offset {offset} is out of bounds for source length {source_len}")]
    OffsetOutOfBounds {
        /// The out-of-bounds byte offset.
        offset: usize,
        /// Length of the source text.
        source_len: usize,
    },
    /// A byte span is invalid for the given source text.
    #[error("span {start}..{end} is invalid for source length {source_len}")]
    SpanOutOfBounds {
        /// Start byte offset of the span.
        start: usize,
        /// End byte offset of the span.
        end: usize,
        /// Length of the source text.
        source_len: usize,
    },
}

impl DiagnosticLocation {
    /// A location at a byte span, without line/column information.
    pub const fn new(span: Span) -> Self {
        Self::Known {
            span,
            position: None,
        }
    }

    /// A location at a byte span whose line and column are known.
    pub const fn known_at(span: Span, position: LineColumn) -> Self {
        Self::Known {
            span,
            position: Some(position),
        }
    }

    /// A location from start and end byte offsets, without line/column.
    ///
    /// Equivalent to `DiagnosticLocation::new(Span::new(start, end))`.
    #[track_caller]
    pub fn from_offsets(start: usize, end: usize) -> Self {
        Self::new(Span::new(start, end))
    }

    /// A location at a byte offset of `source`, with its line and column.
    ///
    /// Returns an error when `offset` is out of bounds for `source`.
    pub fn from_offset_in_source(offset: usize, source: &str) -> Result<Self, SourceLocationError> {
        if offset > source.len() {
            return Err(SourceLocationError::OffsetOutOfBounds {
                offset,
                source_len: source.len(),
            });
        }
        Ok(Self::known_at(
            Span::at(offset),
            LineColumn::of_offset(offset, source),
        ))
    }

    /// A location at a byte range of `source`, with its start's line and column.
    ///
    /// Returns an error when the range is inverted or out of bounds for `source`.
    pub fn from_offsets_in_source(
        start: usize,
        end: usize,
        source: &str,
    ) -> Result<Self, SourceLocationError> {
        match Span::try_new(start, end) {
            Ok(span) if end <= source.len() => {
                Ok(Self::known_at(span, LineColumn::of_offset(start, source)))
            }
            Ok(_) | Err(_) => Err(SourceLocationError::SpanOutOfBounds {
                start,
                end,
                source_len: source.len(),
            }),
        }
    }

    /// A zero-width location at a single byte offset (no line/column).
    ///
    /// Useful for insertion diagnostics that point *between* characters rather
    /// than *at* a range.
    pub fn at_offset(offset: usize) -> Self {
        Self::new(Span::at(offset))
    }

    /// A location from a `Range<usize>` produced by a trusted producer.
    #[track_caller]
    pub fn from_range(range: std::ops::Range<usize>) -> Self {
        Self::new(Span::new(range.start, range.end))
    }

    /// The same diagnostic in another coordinate space.
    ///
    /// `project` maps the span; the line and column are dropped because they
    /// described the old coordinates. An unlocated diagnostic stays unlocated.
    pub fn rebased(self, project: impl FnOnce(Span) -> Span) -> Self {
        match self {
            Self::Known { span, .. } => Self::new(project(span)),
            Self::Unlocated => Self::Unlocated,
        }
    }

    /// The byte span, if the diagnostic has one.
    pub const fn span(&self) -> Option<Span> {
        match self {
            Self::Known { span, .. } => Some(*span),
            Self::Unlocated => None,
        }
    }

    /// The line and column, if the diagnostic has a span and they were computed.
    pub const fn position(&self) -> Option<LineColumn> {
        match self {
            Self::Known { position, .. } => *position,
            Self::Unlocated => None,
        }
    }
}

impl fmt::Display for DiagnosticLocation {
    /// `line L, column C, bytes S..E`, `bytes S..E`, or `location unknown`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known {
                span,
                position: Some(LineColumn { line, column }),
            } => write!(
                f,
                "line {line}, column {column}, bytes {}..{}",
                span.start(),
                span.end()
            ),
            Self::Known {
                span,
                position: None,
            } => write!(f, "bytes {}..{}", span.start(), span.end()),
            Self::Unlocated => write!(f, "location unknown"),
        }
    }
}

impl From<Span> for DiagnosticLocation {
    /// A span is always a real location, so this is total.
    fn from(span: Span) -> Self {
        Self::new(span)
    }
}

impl From<Location> for DiagnosticLocation {
    /// A located value's span, or an unlocated diagnostic.
    fn from(location: Location) -> Self {
        match location {
            Location::Located(span) => Self::new(span),
            Location::Unlocated => Self::Unlocated,
        }
    }
}

impl From<Span> for miette::SourceSpan {
    /// Convert absolute byte-span data into `miette` source-span format.
    fn from(span: Span) -> Self {
        (span.start, span.len()).into()
    }
}

impl From<Severity> for miette::Severity {
    /// Map parser severity to `miette` severity.
    fn from(s: Severity) -> Self {
        match s {
            Severity::Error => miette::Severity::Error,
            Severity::Warning => miette::Severity::Warning,
        }
    }
}

#[cfg(test)]
mod source_identity_tests {
    use super::LineColumn;
    use std::num::NonZeroUsize;

    fn at(line: usize, column: usize) -> LineColumn {
        LineColumn {
            line: NonZeroUsize::new(line).expect("test line is nonzero"),
            column: NonZeroUsize::new(column).expect("test column is nonzero"),
        }
    }

    #[test]
    fn edited_source_at_same_address_does_not_reuse_stale_line_positions() {
        let mut source = String::from("a\nbc");
        let address = source.as_ptr();
        assert_eq!(LineColumn::of_offset(3, &source), at(2, 2));
        source.replace_range(.., "ab\nc");
        assert_eq!(source.as_ptr(), address);
        assert_eq!(LineColumn::of_offset(3, &source), at(2, 1));
    }
}

#[cfg(test)]
mod wire_tests {
    use super::{DiagnosticLocation, LineColumn};
    use crate::Span;
    use serde_json::json;

    fn position(line: usize, column: usize) -> LineColumn {
        LineColumn::one_based(line, column).expect("test position is one-based")
    }

    #[test]
    fn known_and_unlocated_have_tagged_wire_forms() {
        let cases = [
            (
                DiagnosticLocation::known_at(Span::new(1, 3), position(2, 4)),
                json!({"kind": "known", "start": 1, "end": 3, "line": 2, "column": 4}),
            ),
            (
                DiagnosticLocation::new(Span::new(1, 3)),
                json!({"kind": "known", "start": 1, "end": 3}),
            ),
            (DiagnosticLocation::Unlocated, json!({"kind": "unlocated"})),
        ];
        for (location, wire) in cases {
            assert_eq!(serde_json::to_value(location).expect("serialize"), wire);
            let decoded: DiagnosticLocation =
                serde_json::from_value(wire).expect("decode tagged location");
            assert_eq!(decoded, location);
        }
    }

    #[test]
    fn wire_refuses_inverted_spans_zero_positions_and_half_positions() {
        for wire in [
            json!({"kind": "known", "start": 3, "end": 1}),
            json!({"kind": "known", "start": 1, "end": 3, "line": 0, "column": 1}),
            json!({"kind": "known", "start": 1, "end": 3, "line": 2}),
            json!({"kind": "known", "start": 1, "end": 3, "column": 2}),
            json!({"start": 1, "end": 3}),
        ] {
            assert!(
                serde_json::from_value::<DiagnosticLocation>(wire.clone()).is_err(),
                "{wire} must be refused"
            );
        }
    }

    #[test]
    fn display_names_each_variant() {
        assert_eq!(
            DiagnosticLocation::known_at(Span::new(1, 3), position(2, 4)).to_string(),
            "line 2, column 4, bytes 1..3"
        );
        assert_eq!(
            DiagnosticLocation::new(Span::new(1, 3)).to_string(),
            "bytes 1..3"
        );
        assert_eq!(
            DiagnosticLocation::Unlocated.to_string(),
            "location unknown"
        );
    }
}
