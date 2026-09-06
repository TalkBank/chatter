//! Admitted caller source and its document coordinate range.

use super::{ErrorCode, ErrorSink, ParseError, Severity, Span, SpanShift};

/// The requested source range cannot be represented by model coordinates.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Fragment document range exceeds the 32-bit source coordinate space")]
pub struct FragmentRangeError;

impl FragmentRangeError {
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
        let end = origin.checked_add(input.len());
        if end.is_none_or(|end| end > u32::MAX as usize) {
            return Err(FragmentRangeError);
        }
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
