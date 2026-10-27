//! Translate document locations while retaining self-contained source snippets.

use super::{ErrorSink, ParseError, Span};

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

/// Translation is rigid: unlike a deletion, it cannot collapse an endpoint.
#[derive(Debug, thiserror::Error)]
#[error("span {span:?} cannot be translated by {delta} bytes")]
struct UnrepresentableTranslation {
    span: Span,
    delta: isize,
}

impl<S: ErrorSink> ErrorSink for RebasedErrorSink<'_, S> {
    fn report(&self, error: ParseError) {
        self.inner.report(super::span_shift::translate_diagnostic(
            error,
            &mut |span| match span
                .start()
                .checked_add_signed(self.delta)
                .zip(span.end().checked_add_signed(self.delta))
            {
                Some((start, end)) => Ok(Span::new(start, end)),
                None => Err(UnrepresentableTranslation {
                    span,
                    delta: self.delta,
                }),
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::RebasedErrorSink;
    use crate::{
        DiagnosticLocation, ErrorCode, ErrorCollector, ErrorSink, LineColumn, ParseError, Span,
    };

    /// A translation changes coordinate space, so an identity displacement
    /// still drops the position cached for the previous source.
    #[test]
    fn translation_drops_the_old_position_even_when_the_span_does_not_move() {
        let position = LineColumn::one_based(1, 1).expect("one-based position");
        let received = ErrorCollector::new();
        RebasedErrorSink::new(&received, 0).report(
            ParseError::build(ErrorCode::TestError)
                .location(DiagnosticLocation::known_at(Span::new(0, 0), position))
                .message("at the start")
                .finish(),
        );
        let reported = received.into_vec();
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].location.position(), None);
        assert_eq!(reported[0].location.span(), Some(Span::at(0)));
    }

    #[test]
    fn invalid_translation_reports_failure_without_clamped_locations() {
        use crate::{ErrorContext, ErrorLabel, Severity};
        for (span, delta) in [
            (Span::new(0, 1), -1),
            (Span::new(usize::MAX - 1, usize::MAX), 1),
        ] {
            let original = ParseError::new(
                ErrorCode::TestError,
                Severity::Error,
                DiagnosticLocation::new(Span::at(1)),
                ErrorContext::new("x", Span::new(0, 1), "x"),
                "original failure",
            )
            .with_label(ErrorLabel::new(span, "related failure"));
            let received = ErrorCollector::new();
            RebasedErrorSink::new(&received, delta).report(original.clone());
            let reported = received.into_vec();
            assert_eq!(reported.len(), 1);
            assert_eq!(reported[0].code, ErrorCode::InternalError);
            assert_eq!(reported[0].location, DiagnosticLocation::Unlocated);
            assert!(reported[0].labels.is_empty());
            assert_eq!(reported[0].context, original.context);
            assert!(reported[0].message.contains("original failure"));
            assert!(reported[0].message.contains(&format!("{span:?}")));
        }
    }
}
