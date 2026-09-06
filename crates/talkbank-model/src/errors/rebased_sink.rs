//! Translate document locations while retaining self-contained source snippets.

use super::{ErrorSink, ParseError, SpanShift};

/// Rebase streamed diagnostic locations by a signed byte displacement.
///
/// Raw parser location and label spans use document coordinates. Context
/// spans index their own stored source text and must remain unchanged. This
/// differs from removing a synthetic parser wrapper, which also replaces and
/// clips source context through [`super::OffsetAdjustingErrorSink`].
/// Apply this before display enhancement, which converts secondary labels to
/// snippet-relative coordinates.
pub struct RebasedErrorSink<'a, S> {
    inner: &'a S,
    delta: i32,
}

impl<'a, S: ErrorSink> RebasedErrorSink<'a, S> {
    /// Construct one translation shared by every diagnostic on this path.
    pub fn new(inner: &'a S, delta: i32) -> Self {
        Self { inner, delta }
    }
}

impl<S: ErrorSink> ErrorSink for RebasedErrorSink<'_, S> {
    fn report(&self, mut error: ParseError) {
        error.location.span.shift_spans_after(0, self.delta);
        for label in &mut error.labels {
            label.shift_spans_after(0, self.delta);
        }
        self.inner.report(error);
    }
}
