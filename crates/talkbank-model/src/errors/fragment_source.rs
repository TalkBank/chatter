//! Admitted caller source and its document coordinate range.

use super::{ErrorCode, ErrorSink, ParseError, Severity, Span, SpanShift};

/// The requested source range cannot be represented by model coordinates.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Fragment document range exceeds the 32-bit source coordinate space")]
pub struct FragmentRangeError;

impl FragmentRangeError {
    /// Does a document range of `len` bytes starting at `origin` fit the
    /// model's 32-bit byte coordinates?
    ///
    /// # Why the arithmetic is separate from the value it guards
    ///
    /// So it can be TESTED. The only way to reach the failing case through
    /// [`FragmentSource::new`] at origin zero, which is what a whole-file
    /// parse passes, is to hold four gibibytes of text; a test that allocates
    /// that is a test nobody runs. Split out, the case is
    /// `check(0, u32::MAX as usize + 1)` and costs nothing.
    ///
    /// # Errors
    ///
    /// When `origin + len` overflows or exceeds [`u32::MAX`].
    pub const fn check(origin: usize, len: usize) -> Result<(), Self> {
        match origin.checked_add(len) {
            Some(end) if end <= u32::MAX as usize => Ok(()),
            // Both arms are the same refusal, written out rather than joined
            // by an `is_none_or`, because they are different facts: the first
            // is a range that does not fit, the second a range whose END
            // cannot even be computed.
            Some(_) | None => Err(Self),
        }
    }

    /// No document location can be fabricated for an unrepresentable range.
    pub fn into_diagnostic(self) -> ParseError {
        ParseError::at_span(
            ErrorCode::ParseFailed,
            Severity::Error,
            Span::DUMMY,
            self.to_string(),
        )
    }
}

/// A fragment whose complete document range fits the model's byte coordinates.
#[derive(Clone, Copy)]
pub struct FragmentSource<'a> {
    input: &'a str,
    origin: u32,
}

impl<'a> FragmentSource<'a> {
    /// Reject an unrepresentable range before invoking a parser or narrowing offsets.
    pub fn new(input: &'a str, origin: usize) -> Result<Self, FragmentRangeError> {
        FragmentRangeError::check(origin, input.len())?;
        Ok(Self {
            input,
            origin: origin as u32,
        })
    }

    /// Admit at a streaming parser boundary, reporting failure exactly once.
    pub fn admit(
        input: &'a str,
        origin: usize,
        errors: &impl ErrorSink,
    ) -> crate::ParseOutcome<Self> {
        match Self::new(input, origin) {
            Ok(source) => crate::ParseOutcome::parsed(source),
            Err(error) => {
                errors.report(error.into_diagnostic());
                crate::ParseOutcome::rejected()
            }
        }
    }

    /// Original caller text admitted with this origin.
    pub fn input(self) -> &'a str {
        self.input
    }

    /// Move input-relative model spans into the admitted document range.
    pub fn rebase<T: SpanShift>(self, value: T) -> T {
        self.rebase_from(value, 0)
    }

    /// Project spans from an admitted synthetic source whose input starts here.
    /// Combining wrapper removal with the origin avoids an intermediate zero
    /// position being mistaken for the legacy unknown-span sentinel.
    pub fn rebase_from<T: SpanShift>(self, mut value: T, input_start: u32) -> T {
        let mut remaining = i64::from(self.origin) - i64::from(input_start);
        // At most three bounded edit steps; neither direction wraps an i32.
        while remaining != 0 {
            let step = remaining.clamp(-i64::from(i32::MAX), i64::from(i32::MAX)) as i32;
            value.shift_spans_after(0, step);
            remaining -= i64::from(step);
        }
        value
    }

    /// Rebase locations and labels, retaining snippet-relative context spans.
    pub fn error_sink<S: ErrorSink>(self, inner: &S) -> impl ErrorSink {
        FragmentSink {
            source: self,
            inner,
        }
    }
}

struct FragmentSink<'a, 's, S> {
    source: FragmentSource<'s>,
    inner: &'a S,
}

impl<S: ErrorSink> ErrorSink for FragmentSink<'_, '_, S> {
    fn report(&self, mut error: ParseError) {
        error.location.span = self.source.rebase(error.location.span);
        for label in &mut error.labels {
            label.span = self.source.rebase(label.span);
        }
        self.inner.report(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document larger than the coordinate space is refused, at origin zero.
    ///
    /// SURVIVES a type change, and says which category: this is ARITHMETIC.
    /// No signature relates a `usize` length to a `u32` coordinate space, and
    /// the check is the only thing that does.
    ///
    /// Origin zero is the whole-file case, and it is the one that had no
    /// caller: every FRAGMENT entry point admitted its input through
    /// `FragmentSource`, and `parse_chat_file_streaming` did not, so a
    /// four-gibibyte transcript reached tree-sitter unchecked. The reason that
    /// went unnoticed is in this test's own shape: reaching it through
    /// `FragmentSource::new` needs four gibibytes of text, so nobody wrote it.
    #[test]
    fn a_document_past_the_coordinate_space_is_refused() {
        assert!(FragmentRangeError::check(0, u32::MAX as usize).is_ok());
        assert!(FragmentRangeError::check(0, u32::MAX as usize + 1).is_err());
    }

    /// A fragment whose ORIGIN pushes it past the space is refused too.
    ///
    /// The case the fragment API always had, kept because the split that made
    /// the test above possible must not lose it.
    #[test]
    fn a_fragment_placed_past_the_coordinate_space_is_refused() {
        assert!(FragmentRangeError::check(u32::MAX as usize - 4, 4).is_ok());
        assert!(FragmentRangeError::check(u32::MAX as usize - 4, 5).is_err());
    }

    /// A range whose end cannot be computed at all is refused, not wrapped.
    ///
    /// `usize::MAX` plus anything overflows. Before the check was factored out
    /// this arm was an `is_none_or`, where the two facts (a range that does
    /// not fit, and a range whose end does not exist) shared one expression.
    #[test]
    fn a_range_whose_end_overflows_is_refused() {
        assert!(FragmentRangeError::check(usize::MAX, 1).is_err());
    }
}
