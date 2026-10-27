//! Error types for CHAT parsing and validation
//!
//! # Design Principles
//!
//! - Keep `ErrorSink` streaming; avoid collecting errors inside parsers
//! - Prefer `Result` for recoverable errors; avoid silent defaults
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Examples
//!
//! ```
//! use talkbank_model::{ErrorCollector, ErrorCode, ErrorSink, ParseError, Severity, Span};
//!
//! // Create a span and push an error into a streaming sink
//! let span = Span::new(0, 5);
//! let error = ParseError::at_span(ErrorCode::ParseFailed, Severity::Error, span, "bad token");
//! let sink = ErrorCollector::new();
//! sink.report(error);
//! assert_eq!(sink.len(), 1);
//! ```

/// Async-compatible channel-backed sink implementations.
#[cfg(feature = "async")]
pub mod async_channel_sink;
/// Builder pattern for constructing `ParseError` instances.
pub mod builder;
/// CHAT format text processing for display output.
pub mod chat_formatting;
/// CLAN-adjusted error location resolution (hidden header handling).
pub mod clan_location;
/// Error code definitions for CHAT parsing and validation.
pub mod codes;
/// In-memory error collectors and counters.
pub mod collectors;
mod completed_diagnostics;
/// Which validation rules run: the input the cache key is derived from.
pub mod config;
/// Rich error context for helpful error messages.
pub mod context;
/// Diagnostic KIND (Axis 1) and validation PROFILE (Axis 2): the exhaustive
/// per-code registry that severity is derived from, never stored.
pub mod diagnostic_kind;
/// Error enhancement utilities for adding line/column and source context.
pub mod enhance;
/// Core error sink trait plus lightweight forwarding implementations.
pub mod error_sink;
mod fragment_source;
/// Generated `DiagnosticKind` match, produced from `spec/codes/error-codes.toml`
/// by `just spec-gen`. DO NOT EDIT BY HAND; see
/// the file's own header for the regeneration command.
mod generated_diagnostic_kind;
/// Byte-offset line index for O(log n) line/column lookups.
pub mod line_map;
/// Offset-adjusting error sink for wrapper technique offset adjustment.
pub mod offset_adjusting_sink;
/// Core diagnostic type: `ParseError` with source-backed context.
pub mod parse_error;
/// Collection type: `ParseErrors` and `ParseResult` type alias.
pub mod parse_errors;
mod physical_lines;
mod rebased_sink;
pub use fragment_source::{
    FragmentRangeError, FragmentRebaseError, FragmentSource, MAX_PARSER_INPUT_BYTES,
};
/// Source location types: `DiagnosticLocation`, `ErrorLabel`, `Severity`, `ErrorVec`.
pub mod location;
pub mod source_location;
/// Span shifting trait for adjusting byte offsets in errors and AST nodes.
pub mod span_shift;
/// Error sink adapter that duplicates diagnostics to two downstream sinks.
pub mod tee_sink;

#[cfg(test)]
mod tests;

#[cfg(feature = "async")]
pub use async_channel_sink::AsyncChannelErrorSink;
pub use builder::ParseErrorBuilder;
pub use clan_location::{ClanLocation, ClanLocationError, resolve_clan_location};
pub use codes::{CheckStatus, ErrorCode, XPHON_ERROR_CODES, validation_rules_fingerprint};
pub use collectors::{ErrorCollector, ParseTracker};
pub use completed_diagnostics::{CompletedDiagnostics, InternalFailure};
pub use config::{LinkerChecks, RuleSelection};
pub use context::{ErrorContext, SourceExcerptError};
pub use diagnostic_kind::{DiagnosticKind, ValidationProfile, kind_of, severity};
pub use enhance::{enhance_errors_with_index, enhance_errors_with_source};
mod source_diagnostic;
#[cfg(feature = "channels")]
pub use error_sink::ChannelErrorSink;
pub use error_sink::{ErrorSink, NullErrorSink};
pub use line_map::{LineMap, SourceIndex};
pub use location::Location;
pub use offset_adjusting_sink::OffsetAdjustingErrorSink;
pub use parse_error::ParseError;
pub use parse_errors::{ParseErrors, ParseResult};
pub use rebased_sink::RebasedErrorSink;
pub use source_diagnostic::{
    DiagnosticContext, DiagnosticLabel, DiagnosticPresentation, DiagnosticSourceError, DisplaySpan,
    SourceDiagnostic,
};
pub use source_location::{
    DiagnosticLocation, DiagnosticLocationWireError, ErrorLabel, ErrorVec, LineColumn, Severity,
    SourceLocationError,
};
pub use span_shift::SpanShift;
pub use tee_sink::TeeErrorSink;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Range;

// =============================================================================
// Span - a byte range in source text
// =============================================================================

/// A half-open byte range `start..end` in source text.
///
/// Offsets are `usize`, the width every producer (tree-sitter, the re2c lexer,
/// `str` slicing) already uses, so constructing a span never truncates. The
/// fields are private: a span is built through [`Span::new`] (trusted, ordered
/// producer offsets), [`Span::at`] (a zero-width point) or the checked
/// [`Span::try_new`] for untrusted input. JSON
/// deserialization goes through the checked path, so a decoded span always has
/// `start <= end`.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "SpanWire")]
#[schemars(description = "A half-open byte range `start..end` in the source text; `start <= end`.")]
pub struct Span {
    /// Start byte offset (0-indexed, inclusive)
    start: usize,
    /// End byte offset (0-indexed, exclusive)
    end: usize,
}

/// The unchecked wire shape of a [`Span`]; admitted through [`Span::try_new`].
/// schemars follows `try_from`, so the published schema describes this shape.
#[derive(Deserialize, JsonSchema)]
struct SpanWire {
    /// Start byte offset (0-indexed, inclusive)
    start: usize,
    /// End byte offset (0-indexed, exclusive)
    end: usize,
}

impl TryFrom<SpanWire> for Span {
    type Error = InvertedSpan;

    fn try_from(wire: SpanWire) -> Result<Self, Self::Error> {
        Span::try_new(wire.start, wire.end)
    }
}

/// A byte range whose start lies after its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("byte range {start}..{end} ends before it starts")]
pub struct InvertedSpan {
    /// The offered start offset.
    pub start: usize,
    /// The offered end offset.
    pub end: usize,
}

impl Span {
    /// A span from a trusted producer's ordered offsets.
    ///
    /// Callers are parsers and lexers whose ranges are ordered by construction
    /// (a tree-sitter node's `start_byte() <= end_byte()`, a lexer token's
    /// bounds) and arithmetic on spans already admitted. Untrusted offsets
    /// (wire input, editor positions) must use [`Span::try_new`].
    ///
    /// # Panics
    ///
    /// If `start > end`, in every build profile. An inverted range from a
    /// trusted producer is a bug in that producer (or in tree-sitter), not a
    /// property of the input, so it fails at the call site (reported through
    /// `track_caller`) rather than travelling on as a span that slices or
    /// measures wrongly. In a `const` context the check runs at compile time.
    #[inline]
    #[track_caller]
    pub const fn new(start: usize, end: usize) -> Self {
        assert!(start <= end, "Span::new: start lies after end");
        Self { start, end }
    }

    /// A span from untrusted offsets, refusing `start > end`.
    #[inline]
    pub fn try_new(start: usize, end: usize) -> Result<Self, InvertedSpan> {
        if start <= end {
            Ok(Self { start, end })
        } else {
            Err(InvertedSpan { start, end })
        }
    }

    /// A zero-width span at a single byte offset.
    #[inline]
    pub fn at(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    /// Start byte offset (inclusive).
    #[inline]
    pub fn start(&self) -> usize {
        self.start
    }

    /// End byte offset (exclusive).
    #[inline]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Check if span is empty (zero-width)
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Convert to `Range<usize>` for slicing.
    #[inline]
    pub fn to_range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Merge two spans into the smallest span that covers both.
    ///
    /// The result starts at `min(self.start, other.start)` and ends at
    /// `max(self.end, other.end)`. Useful for computing the span of a
    /// composite AST node from its children.
    #[inline]
    pub fn merge(&self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Check if this span fully contains another span
    #[inline]
    pub fn contains_span(&self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Check if this span contains a byte offset
    #[inline]
    pub fn contains_offset(&self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    /// Whether this span intersects `other` (half-open byte ranges).
    ///
    /// Two nonempty half-open ranges `[a.start, a.end)` and `[b.start, b.end)` overlap
    /// iff `a.start < b.end && b.start < a.end`. Unlike [`contains_span`], this
    /// is symmetric and detects partial overlap, not just containment. A
    /// zero-width span never overlaps anything (it covers no bytes); widen it to
    /// at least one byte at the call site if a point should match a containing
    /// span.
    ///
    /// [`contains_span`]: Span::contains_span
    #[inline]
    pub fn overlaps(&self, other: Span) -> bool {
        !self.is_empty() && !other.is_empty() && self.start < other.end && other.start < self.end
    }
}

impl TryFrom<Range<usize>> for Span {
    type Error = InvertedSpan;

    /// A span from a range whose order is not known, refusing `start > end`.
    fn try_from(range: Range<usize>) -> Result<Self, Self::Error> {
        Span::try_new(range.start, range.end)
    }
}

impl From<Span> for Range<usize> {
    /// Convert a `Span` into a range suitable for slicing.
    fn from(span: Span) -> Self {
        span.to_range()
    }
}
