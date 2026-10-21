//! Admitted caller source and its document coordinate range.

use super::{ErrorCode, ErrorSink, ParseError, Severity, Span, SpanShift};

/// The largest input a parser accepts: tree-sitter stores byte offsets as
/// `uint32_t`, so a longer input cannot be parsed faithfully.
pub const MAX_PARSER_INPUT_BYTES: usize = u32::MAX as usize;

/// Why a fragment cannot be admitted. Two distinct facts, never conflated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FragmentRangeError {
    /// The input is longer than a parser can represent.
    #[error("input of {len} bytes exceeds the {MAX_PARSER_INPUT_BYTES}-byte parser input limit")]
    InputTooLarge {
        /// Length of the offered input in bytes.
        len: usize,
    },
    /// `origin + len` does not exist as a byte offset.
    #[error("document range starting at {origin} with length {len} overflows")]
    RangeOverflows {
        /// Requested document origin.
        origin: usize,
        /// Length of the offered input in bytes.
        len: usize,
    },
}

impl FragmentRangeError {
    /// Can `len` bytes placed at `origin` be parsed and located?
    ///
    /// # Why the arithmetic is separate from the value it guards
    ///
    /// So it can be TESTED. The failing input-size case through
    /// [`FragmentSource::new`] needs four gibibytes of text; split out, it is
    /// `check(0, MAX_PARSER_INPUT_BYTES + 1)` and costs nothing.
    ///
    /// # Errors
    ///
    /// [`InputTooLarge`](Self::InputTooLarge) when the input exceeds the parser
    /// limit; [`RangeOverflows`](Self::RangeOverflows) when its end offset does
    /// not exist.
    pub const fn check(origin: usize, len: usize) -> Result<(), Self> {
        if len > MAX_PARSER_INPUT_BYTES {
            return Err(Self::InputTooLarge { len });
        }
        match origin.checked_add(len) {
            Some(_) => Ok(()),
            None => Err(Self::RangeOverflows { origin, len }),
        }
    }

    /// No document location can be fabricated for an unrepresentable range.
    pub fn into_diagnostic(self) -> ParseError {
        ParseError::at_location(
            ErrorCode::ParseFailed,
            Severity::Error,
            crate::DiagnosticLocation::Unlocated,
            self.to_string(),
        )
    }
}

/// A fragment that a parser can read and whose document range exists.
#[derive(Clone, Copy)]
pub struct FragmentSource<'a> {
    input: &'a str,
    origin: usize,
}

impl<'a> FragmentSource<'a> {
    /// Reject an unparseable or unlocatable range before invoking a parser.
    pub fn new(input: &'a str, origin: usize) -> Result<Self, FragmentRangeError> {
        FragmentRangeError::check(origin, input.len())?;
        Ok(Self { input, origin })
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

    /// The complete admitted document range, including a known empty range.
    /// Unlike generic model-span shifting, this comes from source admission
    /// rather than interpreting the legacy unknown-span sentinel.
    pub fn whole_span(self) -> Span {
        // Admission proved `origin + len` exists.
        Span::new(self.origin, self.origin + self.input.len())
    }

    /// Source admission, not a generic span, knows an empty caller's point.
    /// Primary locations and related labels consume this same source fact.
    fn diagnostic_span(self, span: Span) -> Span {
        if self.input.is_empty() {
            self.whole_span()
        } else {
            self.rebase(span)
        }
    }

    /// Move input-relative model spans into the admitted document range.
    pub fn rebase<T: SpanShift>(self, value: T) -> T {
        self.rebase_from(value, 0)
    }

    /// Project spans from an admitted synthetic source whose input starts here.
    /// Combining wrapper removal with the origin avoids an intermediate zero
    /// position being mistaken for the legacy unknown-span sentinel.
    pub fn rebase_from<T: SpanShift>(self, mut value: T, input_start: usize) -> T {
        // Move by the signed distance `origin - input_start` in steps that fit
        // `isize`; at most a few steps even for the largest offsets.
        let (mut remaining, forward) = match self.origin.checked_sub(input_start) {
            Some(distance) => (distance, true),
            None => (input_start - self.origin, false),
        };
        while remaining != 0 {
            let step = remaining.min(isize::MAX as usize);
            let signed = if forward {
                step as isize
            } else {
                -(step as isize)
            };
            value.shift_spans_after(0, signed);
            remaining -= step;
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
        error.location = error
            .location
            .rebased(|span| self.source.diagnostic_span(span));
        for label in &mut error.labels {
            label.span = self.source.diagnostic_span(label.span);
        }
        self.inner.report(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An input longer than the parser limit is refused, at origin zero.
    ///
    /// SURVIVES the type change to `usize` spans: this is the tree-sitter
    /// input limit, which no span type expresses. Origin zero is the
    /// whole-file case that `parse_chat_file_streaming` admits.
    #[test]
    fn an_input_past_the_parser_limit_is_refused() {
        assert!(FragmentRangeError::check(0, MAX_PARSER_INPUT_BYTES).is_ok());
        assert_eq!(
            FragmentRangeError::check(0, MAX_PARSER_INPUT_BYTES + 1),
            Err(FragmentRangeError::InputTooLarge {
                len: MAX_PARSER_INPUT_BYTES + 1
            })
        );
    }

    /// A large ORIGIN no longer matters: document offsets are `usize`.
    #[test]
    fn a_fragment_placed_far_into_a_document_is_admitted() {
        assert!(FragmentRangeError::check(MAX_PARSER_INPUT_BYTES * 2, 5).is_ok());
    }

    /// A range whose end cannot be computed at all is refused, not wrapped.
    #[test]
    fn a_range_whose_end_overflows_is_refused() {
        assert_eq!(
            FragmentRangeError::check(usize::MAX, 1),
            Err(FragmentRangeError::RangeOverflows {
                origin: usize::MAX,
                len: 1
            })
        );
    }

    /// Rebasing moves spans by the full signed distance, in both directions.
    #[test]
    fn rebasing_moves_spans_by_the_signed_distance() {
        let source = FragmentSource::new("abc", 10).expect("admitted");
        assert_eq!(source.rebase(Span::new(1, 2)), Span::new(11, 12));
        assert_eq!(source.rebase_from(Span::new(15, 16), 12), Span::new(13, 14));
    }
}
