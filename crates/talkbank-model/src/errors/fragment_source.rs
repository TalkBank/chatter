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

/// Why a value cannot move from its fragment coordinates into the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FragmentRebaseError {
    /// The requested producer coordinate range does not exist.
    #[error("fragment input starting at {input_start} with length {input_len} overflows")]
    SourceRangeOverflows {
        /// Start in the producer's coordinate space.
        input_start: usize,
        /// Admitted caller input length.
        input_len: usize,
    },
    /// An offered model span includes bytes the caller did not supply.
    #[error("span {span:?} is outside fragment input {input:?}")]
    OutsideInput {
        /// Offered coordinates, before translation.
        span: Span,
        /// The complete caller input in the producer's coordinate space.
        input: Span,
    },
}

/// Both coordinate ranges exist. Only contained spans may cross this boundary.
struct FragmentCoordinates<'a> {
    source: FragmentSource<'a>,
    input: Span,
}

impl<'a> FragmentCoordinates<'a> {
    fn new(source: FragmentSource<'a>, input_start: usize) -> Result<Self, FragmentRebaseError> {
        let input_end = input_start.checked_add(source.input.len()).ok_or(
            FragmentRebaseError::SourceRangeOverflows {
                input_start,
                input_len: source.input.len(),
            },
        )?;
        Ok(Self {
            source,
            input: Span::new(input_start, input_end),
        })
    }

    fn rebase<T: SpanShift>(&self, value: T) -> Result<T, FragmentRebaseError> {
        value.try_map_spans(&mut |span| self.rebase_span(span))
    }

    fn rebase_span(&self, span: Span) -> Result<Span, FragmentRebaseError> {
        if span.start() < self.input.start() || span.end() > self.input.end() {
            return Err(FragmentRebaseError::OutsideInput {
                span,
                input: self.input,
            });
        }
        // Containment and admission of both coordinate ranges prove these
        // subtractions and additions exist and preserve the span's width.
        Ok(Span::new(
            self.source.origin + (span.start() - self.input.start()),
            self.source.origin + (span.end() - self.input.start()),
        ))
    }
}

impl FragmentRebaseError {
    /// Report failed model admission without inventing a document location.
    pub fn into_diagnostic(self) -> ParseError {
        ParseError::at_location(
            ErrorCode::InternalError,
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
    pub fn whole_span(self) -> Span {
        // Admission proved `origin + len` exists.
        Span::new(self.origin, self.origin + self.input.len())
    }

    /// Move input-relative model spans into the admitted document range.
    /// Reject any span outside the caller input; never clamp model coordinates.
    pub fn rebase<T: SpanShift>(self, value: T) -> Result<T, FragmentRebaseError> {
        self.relative_coordinates().rebase(value)
    }

    /// Consume a value in synthetic coordinates and admit its participating
    /// spans before returning it in document coordinates. Failure exposes no
    /// partial model.
    pub fn rebase_from<T: SpanShift>(
        self,
        value: T,
        input_start: usize,
    ) -> Result<T, FragmentRebaseError> {
        FragmentCoordinates::new(self, input_start)?.rebase(value)
    }

    fn relative_coordinates(self) -> FragmentCoordinates<'a> {
        FragmentCoordinates {
            source: self,
            input: Span::new(0, self.input.len()),
        }
    }

    /// Project a wrapper-owned diagnostic extent onto the caller input.
    /// Unlike model rebasing, this deliberately clips bytes outside the input.
    pub fn project_span_from(self, span: Span, input_start: usize) -> Span {
        let start = span
            .start()
            .saturating_sub(input_start)
            .min(self.input.len());
        let end = span.end().saturating_sub(input_start).min(self.input.len());
        Span::new(self.origin + start, self.origin + end)
    }

    /// Streaming parser boundary: report failed admission once and return no model.
    pub fn rebase_reporting<T: SpanShift>(
        self,
        value: T,
        errors: &impl ErrorSink,
    ) -> crate::ParseOutcome<T> {
        match self.rebase(value) {
            Ok(value) => crate::ParseOutcome::parsed(value),
            Err(error) => {
                errors.report(error.into_diagnostic());
                crate::ParseOutcome::rejected()
            }
        }
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
    fn report(&self, error: ParseError) {
        let coordinates = self.source.relative_coordinates();
        self.inner.report(super::span_shift::translate_diagnostic(
            error,
            &mut |span| {
                if self.source.input.is_empty() {
                    // An empty caller input owns its sole insertion point.
                    Ok(self.source.whole_span())
                } else {
                    coordinates.rebase_span(span)
                }
            },
        ));
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
        assert_eq!(
            source.rebase(Span::new(1, 2)).expect("contained span"),
            Span::new(11, 12)
        );
        assert_eq!(
            source
                .rebase_from(Span::new(14, 15), 12)
                .expect("contained span"),
            Span::new(12, 13)
        );
    }

    /// Source admission cannot certify spans outside that source, even when
    /// their translated offsets would happen to fit in a machine integer.
    #[test]
    fn rebasing_rejects_spans_outside_the_admitted_source() {
        let source = FragmentSource::new("abc", 10).expect("admitted");
        assert!(source.rebase(Span::new(0, 4)).is_err());
        assert!(source.rebase_from(Span::new(11, 13), 12).is_err());
        assert!(source.rebase_from(Span::new(12, 16), 12).is_err());
    }

    /// A bad parser span must not collapse to the maximum document offset.
    #[test]
    fn rebasing_near_the_address_limit_rejects_instead_of_clamping() {
        let source = FragmentSource::new("a", usize::MAX - 1).expect("admitted");
        assert_eq!(
            source.rebase(Span::new(0, 1)).expect("contained span"),
            Span::new(usize::MAX - 1, usize::MAX)
        );
        assert!(source.rebase(Span::new(1, 2)).is_err());
    }

    #[test]
    fn fragment_diagnostic_rejection_preserves_the_message_and_snippet() {
        use crate::{DiagnosticLocation, ErrorCollector, ErrorContext, ErrorLabel};
        let source = FragmentSource::new("abc", 10).expect("source");
        let original = ParseError::new(
            ErrorCode::TestError,
            Severity::Error,
            DiagnosticLocation::new(Span::new(0, 1)),
            ErrorContext::new("abc", Span::new(0, 1), "a"),
            "original failure",
        )
        .with_label(ErrorLabel::new(Span::new(2, 4), "invalid related extent"));
        let errors = ErrorCollector::new();
        source.error_sink(&errors).report(original.clone());
        let received = errors.into_vec();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].code, ErrorCode::InternalError);
        assert_eq!(received[0].location, DiagnosticLocation::Unlocated);
        assert!(received[0].labels.is_empty());
        assert_eq!(received[0].context, original.context);
        assert!(crate::CompletedDiagnostics::admit(received.clone()).is_err());
        assert!(received[0].message.contains("original failure"));
        assert!(received[0].message.contains("end: 3"));
    }

    #[test]
    fn empty_model_points_are_checked_even_when_no_translation_is_needed() {
        let source = FragmentSource::new("", 0).expect("source");
        assert_eq!(source.rebase(Span::at(0)), Ok(Span::at(0)));
        assert!(source.rebase(Span::at(1)).is_err());
        assert_eq!(
            source.rebase_from(Span::at(usize::MAX), usize::MAX),
            Ok(Span::at(0))
        );
    }

    #[test]
    fn an_unrepresentable_producer_range_is_rejected_even_without_model_spans() {
        let source = FragmentSource::new("abc", 10).expect("source");
        assert_eq!(
            source.rebase_from(crate::Location::Unlocated, usize::MAX),
            Err(FragmentRebaseError::SourceRangeOverflows {
                input_start: usize::MAX,
                input_len: 3
            })
        );
    }
}
