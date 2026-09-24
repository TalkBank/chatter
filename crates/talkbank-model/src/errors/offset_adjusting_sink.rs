//! OffsetAdjustingErrorSink for wrapper technique offset adjustment
//!
//! When parsing headers/tiers in isolation by wrapping them in a minimal CHAT document,
//! error offsets are relative to the wrapped document, not the original input.
//!
//! This error sink wrapper adjusts all error locations by subtracting the wrapper offset,
//! ensuring errors are reported relative to the original input.
//!
//! ## Typical flow
//!
//! 1. Build a synthetic wrapper document around a fragment (`@Begin ... @End`).
//! 2. Parse with an implementation that only understands full-file context.
//! 3. Use `OffsetAdjustingErrorSink` to translate diagnostics back to fragment-local offsets.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::{ErrorSink, ParseError, SourceLocation, Span};

/// The fragment's coordinate window. Bounds are derived from the borrowed
/// source, never supplied separately or narrowed from the origin to `u32`.
struct FragmentCoordinates<'a> {
    origin: usize,
    source: &'a str,
}

impl FragmentCoordinates<'_> {
    fn project(&self, span: Span) -> Span {
        let project = |offset: u32| {
            (offset as usize)
                .saturating_sub(self.origin)
                .min(self.source.len())
        };
        Span::from_usize(project(span.start), project(span.end))
    }
}

/// Error sink that rewrites wrapped-document offsets back to fragment offsets.
///
/// Used when parsing content that has been wrapped in a larger document.
/// Primary and secondary document spans are translated and clipped to the
/// fragment. Cached line/column coordinates are cleared. Context snippets keep
/// their own source text and relative spans, independent of text length.
/// Apply this to raw diagnostics, before display enhancement makes labels
/// snippet-relative. The caller supplies the fragment's actual wrapper origin;
/// this adapter does not certify membership in a separate wrapper string.
///
/// # Example
///
/// ```
/// use talkbank_model::{ErrorCollector, OffsetAdjustingErrorSink, ParseError};
///
/// let inner_sink = ErrorCollector::new();
/// let offset = 101; // Original input starts at byte 101 in wrapped document
/// let adjusting_sink = OffsetAdjustingErrorSink::new(&inner_sink, offset, "original input");
///
/// // Errors reported to adjusting_sink will have offsets adjusted
/// // adjusting_sink.report(error at 110) -> inner_sink receives error at 9
/// ```
pub struct OffsetAdjustingErrorSink<'a, S: ErrorSink> {
    /// The underlying error sink
    inner: &'a S,
    coordinates: FragmentCoordinates<'a>,
}

impl<'a, S: ErrorSink> OffsetAdjustingErrorSink<'a, S> {
    /// Create a new offset-adjusting sink.
    ///
    /// # Parameters
    ///
    /// * `inner` - The underlying error sink that will receive adjusted errors
    /// * `offset` - Byte offset where the original input starts in the wrapped document
    /// * `original_input` - The fragment source defining the output span bounds
    pub fn new(inner: &'a S, offset: usize, original_input: &'a str) -> Self {
        Self {
            inner,
            coordinates: FragmentCoordinates {
                origin: offset,
                source: original_input,
            },
        }
    }

    /// Rebase one diagnostic from wrapper coordinates to original-fragment coordinates.
    ///
    /// Document spans share one projection. Context spans index independently
    /// owned source text and must not be shifted or replaced by a size heuristic.
    fn adjust_error(&self, mut error: ParseError) -> ParseError {
        error.location = SourceLocation::new(self.coordinates.project(error.location.span));
        for label in &mut error.labels {
            label.span = self.coordinates.project(label.span);
        }

        error
    }
}

impl<'a, S: ErrorSink> ErrorSink for OffsetAdjustingErrorSink<'a, S> {
    /// Adjusts one diagnostic to original-input offsets and forwards it.
    fn report(&self, error: ParseError) {
        let adjusted = self.adjust_error(error);
        self.inner.report(adjusted);
    }

    /// Adjusts and forwards a `Vec` of diagnostics.
    fn report_all(&self, errors: Vec<ParseError>) {
        let adjusted: Vec<_> = errors.into_iter().map(|e| self.adjust_error(e)).collect();
        self.inner.report_all(adjusted);
    }

    /// Adjusts and forwards an inline `ErrorVec` batch.
    fn report_vec(&self, errors: super::ErrorVec) {
        let adjusted: Vec<_> = errors.into_iter().map(|e| self.adjust_error(e)).collect();
        self.inner.report_all(adjusted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorCode, ErrorCollector, ErrorContext, ParseError, Severity, SourceLocation};

    /// Tests offset adjustment.
    #[test]
    fn test_offset_adjustment() {
        let inner = ErrorCollector::new();
        let original_input = "@Date:\t29-FEB-1996";
        let wrapper_offset = 101;

        let adjusting_sink = OffsetAdjustingErrorSink::new(&inner, wrapper_offset, original_input);

        // Create an error at position 110..119 in wrapped document
        // Should be adjusted to 9..18 in original input (clamped to input length)
        let error = ParseError::new(
            ErrorCode::InternalError,
            Severity::Error,
            SourceLocation::from_offsets_with_position(110, 119, 7, 2),
            ErrorContext::new("wrapped content", 110..119, "test"),
            "Test error",
        );

        adjusting_sink.report(error);

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].location.span.start, 9);
        assert_eq!(errors[0].location.span.end, 18); // Clamped to input length
        assert_eq!(errors[0].location.line, None);
        assert_eq!(errors[0].location.column, None);
    }

    /// Origins outside the span coordinate domain must not wrap through a cast.
    #[test]
    fn large_origin_does_not_alias_zero() {
        let Some(origin) = (u32::MAX as usize).checked_add(1) else {
            return;
        };
        let inner = ErrorCollector::new();
        let sink = OffsetAdjustingErrorSink::new(&inner, origin, "fragment");
        sink.report(ParseError::new(
            ErrorCode::InternalError,
            Severity::Error,
            SourceLocation::from_offsets(1, 3),
            ErrorContext::new("fragment", 1..3, "ra"),
            "coordinate boundary",
        ));
        assert_eq!(inner.into_vec()[0].location.span, Span::new(0, 0));
    }

    /// Tests offset at boundary.
    #[test]
    fn test_offset_at_boundary() {
        let inner = ErrorCollector::new();
        let original_input = "@Date:\t29-FEB-1996";
        let wrapper_offset = 101;

        let adjusting_sink = OffsetAdjustingErrorSink::new(&inner, wrapper_offset, original_input);

        // Error exactly at the start of original input in wrapper
        let error = ParseError::new(
            ErrorCode::InternalError,
            Severity::Error,
            SourceLocation::from_offsets(101, 110),
            ErrorContext::new("wrapped content", 101..110, "test"),
            "Test error at boundary",
        );

        adjusting_sink.report(error);

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].location.span.start, 0);
        assert_eq!(errors[0].location.span.end, 9);
    }

    /// Tests offset clamping.
    #[test]
    fn test_offset_clamping() {
        let inner = ErrorCollector::new();
        let original_input = "@Date:\t29-FEB-1996";
        let wrapper_offset = 101;

        let adjusting_sink = OffsetAdjustingErrorSink::new(&inner, wrapper_offset, original_input);

        // Error that would extend beyond original input
        let error = ParseError::new(
            ErrorCode::InternalError,
            Severity::Error,
            SourceLocation::from_offsets(110, 200),
            ErrorContext::new("wrapped content", 110..200, "test"),
            "Test error beyond bounds",
        );

        adjusting_sink.report(error);

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].location.span.start, 9);
        assert_eq!(errors[0].location.span.end, original_input.len() as u32);
    }
}
