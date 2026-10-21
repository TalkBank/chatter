//! Translate document locations while retaining self-contained source snippets.

use super::{ErrorSink, ParseError, SpanShift};

/// Rebase streamed diagnostic locations by a signed byte displacement.
///
/// Raw parser location and label spans use document coordinates. Context
/// spans index their own stored source text and must remain unchanged. This
/// differs from removing a synthetic parser wrapper, which clips document
/// spans to fragment bounds through [`super::OffsetAdjustingErrorSink`]. Both
/// adapters preserve context snippets and their independent coordinates.
/// Cached primary line/column values are invalidated: byte displacement alone
/// cannot establish their values in the destination document.
/// Apply this before display enhancement, which converts secondary labels to
/// snippet-relative coordinates.
pub struct RebasedErrorSink<'a, S> {
    inner: &'a S,
    delta: isize,
}

impl<'a, S: ErrorSink> RebasedErrorSink<'a, S> {
    /// Construct one translation shared by every diagnostic on this path.
    pub fn new(inner: &'a S, delta: isize) -> Self {
        Self { inner, delta }
    }
}

impl<S: ErrorSink> ErrorSink for RebasedErrorSink<'_, S> {
    fn report(&self, mut error: ParseError) {
        // A translation into another coordinate space: the cached position
        // described the old one, so it is dropped even if the span is a point
        // that does not move.
        error.location = error.location.rebased(|mut span| {
            span.shift_spans_after(0, self.delta);
            span
        });
        for label in &mut error.labels {
            label.shift_spans_after(0, self.delta);
        }
        self.inner.report(error);
    }
}

#[cfg(test)]
mod tests {
    use super::RebasedErrorSink;
    use crate::{
        DiagnosticLocation, ErrorCode, ErrorCollector, ErrorSink, LineColumn, ParseError, Span,
    };

    /// A translation changes coordinate space, so even a point that does not
    /// move (the start of the file, until spans can be unlocated) loses the
    /// position it carried in the old space.
    #[test]
    fn translation_drops_the_old_position_even_when_the_span_does_not_move() {
        let position = LineColumn::one_based(1, 1).expect("one-based position");
        let received = ErrorCollector::new();
        RebasedErrorSink::new(&received, 15).report(
            ParseError::build(ErrorCode::TestError)
                .location(DiagnosticLocation::known_at(Span::new(0, 0), position))
                .message("at the start")
                .finish(),
        );
        let reported = received.into_vec();
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].location.position(), None);
    }
}
