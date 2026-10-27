//! Own the source/token/span relationship for source-aware file parsing.

use std::ops::Range;

use crate::lexer::{Lexer, LexerSpan};
use crate::token::Token;

mod cursor;
pub(crate) use cursor::TokenCursor;
mod region;
use region::RegionFault;
pub(crate) use region::{NonEmptyRegion, ReportedRegionFailure};

/// A whitespace-delimited item borrowed and located by its source owner.
pub(crate) struct LocatedItem<'source> {
    text: &'source str,
    span: talkbank_model::Span,
}

impl<'source> LocatedItem<'source> {
    pub(crate) fn text(&self) -> &'source str {
        self.text
    }
    pub(crate) fn span(&self) -> talkbank_model::Span {
        self.span
    }
}

/// A token selection admitted by its immutable lexer owner. Empty selections
/// are legitimate; tokens, spans and diagnostic source cannot be substituted
/// independently by consumers.
pub(crate) struct LocatedTokens<'tokens, 'source> {
    source: &'source str,
    tokens: &'tokens [Token<'source>],
    spans: &'tokens [LexerSpan],
}

impl<'tokens, 'source> LocatedTokens<'tokens, 'source> {
    pub(crate) fn source(&self) -> &'source str {
        self.source
    }

    pub(crate) fn tokens(&self) -> &'tokens [Token<'source>] {
        self.tokens
    }

    pub(crate) fn into_located(
        self,
    ) -> impl DoubleEndedIterator<Item = (&'tokens Token<'source>, LexerSpan)> {
        self.tokens.iter().zip(self.spans.iter().cloned())
    }
}

/// Only lexing constructs this immutable owner of parallel token/span storage.
pub(crate) struct LexedSource<'source> {
    source: &'source str,
    tokens: Vec<Token<'source>>,
    spans: Vec<LexerSpan>,
}

impl<'source> LexedSource<'source> {
    pub(crate) fn new(source: &'source str, condition: crate::lexer::StartCondition) -> Self {
        let (tokens, spans) = Lexer::new(source, condition).unzip();
        Self {
            source,
            tokens,
            spans,
        }
    }

    pub(crate) fn source(&self) -> &'source str {
        self.source
    }

    /// The text the tokens borrow from, for the parsers that slice it (the
    /// word parsers) and the converter that places against it. Minting it
    /// here keeps the tokens and the text one pairing.
    pub(crate) fn source_text(&self) -> crate::source_text::SourceText<'source> {
        crate::source_text::SourceText::new(self.source)
    }

    pub(crate) fn tokens(&self) -> &[Token<'source>] {
        &self.tokens
    }

    /// Admit one nonempty cursor region from this immutable lexer run.
    /// Invalid producer indices report an internal failure, never bad CHAT.
    pub(crate) fn admit_region(
        &self,
        range: Range<usize>,
        errors: &impl talkbank_model::ErrorSink,
    ) -> Result<NonEmptyRegion<'_, 'source>, ReportedRegionFailure> {
        NonEmptyRegion::from_range(self, range).map_err(|fault| fault.report(errors))
    }

    /// All lexer tokens when there are any. Empty fragment input is an
    /// ordinary absence, not an invalid nonempty cursor selection.
    pub(crate) fn whole_region(&self) -> Result<Option<NonEmptyRegion<'_, 'source>>, RegionFault> {
        NonEmptyRegion::whole(self)
    }

    fn optional_region(
        &self,
        range: Range<usize>,
        errors: &impl talkbank_model::ErrorSink,
    ) -> Result<Option<NonEmptyRegion<'_, 'source>>, ReportedRegionFailure> {
        match NonEmptyRegion::from_range(self, range) {
            Ok(region) => Ok(Some(region)),
            Err(RegionFault::Empty { .. }) => Ok(None),
            Err(fault @ (RegionFault::Outside { .. } | RegionFault::Source(_))) => {
                Err(fault.report(errors))
            }
        }
    }

    /// Start a cursor whose iterator and source have this single owner.
    pub(crate) fn cursor(&self) -> TokenCursor<'_, 'source> {
        TokenCursor::new(self)
    }

    /// Preserve exact source bytes and locations while inspecting a token range.
    /// Rich tokens can expose only a payload through `text()`; concatenating
    /// those payloads would lose spelling and positions in malformed content.
    pub(crate) fn whitespace_items(
        &self,
        range: Range<usize>,
        errors: &impl talkbank_model::ErrorSink,
    ) -> Result<impl Iterator<Item = LocatedItem<'source>>, ReportedRegionFailure> {
        Ok(self
            .optional_region(range, errors)?
            .into_iter()
            .flat_map(NonEmptyRegion::whitespace_items))
    }

    /// Whole-stream inspection has no caller-supplied range to validate.
    pub(crate) fn located(&self) -> impl DoubleEndedIterator<Item = (&Token<'source>, LexerSpan)> {
        self.whole_tokens().into_located()
    }

    pub(crate) fn whole_tokens(&self) -> LocatedTokens<'_, 'source> {
        LocatedTokens {
            source: self.source,
            tokens: &self.tokens,
            spans: &self.spans,
        }
    }

    /// Select tokens once at their owner, before source-aware consumers run.
    pub(crate) fn admit_tokens(
        &self,
        range: Range<usize>,
        errors: &impl talkbank_model::ErrorSink,
    ) -> Result<LocatedTokens<'_, 'source>, ReportedRegionFailure> {
        self.select_range(range)
            .map_err(|fault| fault.report(errors))
    }

    fn select_range(&self, range: Range<usize>) -> Result<LocatedTokens<'_, 'source>, RegionFault> {
        let fault = || RegionFault::Outside {
            start: range.start,
            end: range.end,
            tokens: self.tokens.len(),
        };
        let tokens = self.tokens.get(range.clone()).ok_or_else(fault)?;
        let spans = self.spans.get(range.clone()).ok_or_else(fault)?;
        Ok(LocatedTokens {
            source: self.source,
            tokens,
            spans,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{ErrorCode, ErrorCollector, Severity};

    fn reference_source() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus/reference/annotation/retrace.cha"),
        )
        .expect("existing public reference")
    }

    #[test]
    fn region_admission_refuses_empty_and_outside_producer_ranges_as_internal_faults() {
        let input = reference_source();
        let lexed = LexedSource::new(&input, crate::lexer::COND_INITIAL);
        let sink = ErrorCollector::new();
        assert!(lexed.admit_region(0..0, &sink).is_err());
        assert!(
            lexed
                .admit_region(0..lexed.tokens().len() + 1, &sink)
                .is_err()
        );
        let findings = sink.into_vec();
        assert_eq!(findings.len(), 2);
        assert!(
            findings
                .iter()
                .all(|finding| finding.code == ErrorCode::InternalError
                    && finding.severity == Severity::Error
                    && finding.location.span().is_none())
        );
    }

    #[test]
    fn admitted_region_retains_original_bytes_and_diagnostic_extent() {
        let input = reference_source();
        let lexed = LexedSource::new(&input, crate::lexer::COND_INITIAL);
        let sink = ErrorCollector::new();
        let region = lexed
            .admit_region(0..lexed.tokens().len(), &sink)
            .expect("real tokens");
        assert_eq!(region.extent(), talkbank_model::Span::new(0, input.len()));
        assert_eq!(region.tokens().len(), lexed.tokens().len());
        region.report_error(
            ErrorCode::UnparsableContent,
            "region boundary control",
            &sink,
        );
        let findings = sink.into_vec();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].location.span(), Some(region.extent()));
    }

    #[test]
    fn whitespace_selection_distinguishes_empty_input_from_invalid_indices() {
        let input = reference_source();
        let lexed = LexedSource::new(&input, crate::lexer::COND_INITIAL);
        let sink = ErrorCollector::new();
        assert_eq!(lexed.whitespace_items(0..0, &sink).unwrap().count(), 0);
        assert!(sink.into_vec().is_empty());

        let sink = ErrorCollector::new();
        let outside = lexed.tokens().len() + 1;
        assert!(lexed.whitespace_items(outside..outside, &sink).is_err());
        let findings = sink.into_vec();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, ErrorCode::InternalError);
        assert!(findings[0].location.span().is_none());
    }

    #[test]
    fn region_admission_reports_invalid_source_bounds_as_internal_faults() {
        let input = reference_source();
        let mut lexed = LexedSource::new(&input, crate::lexer::COND_INITIAL);
        let count = lexed.tokens.len();
        // Fault injection at the lexer-storage boundary, not malformed CHAT.
        lexed.spans.last_mut().unwrap().end = input.len() + 1;
        let sink = ErrorCollector::new();
        assert!(lexed.admit_region(0..count, &sink).is_err());
        assert!(matches!(lexed.whole_region(), Err(RegionFault::Source(_))));
        let findings = sink.into_vec();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, ErrorCode::InternalError);
        assert_eq!(findings[0].severity, Severity::Error);
    }

    /// An empty body is admitted; invalid producer coordinates report an
    /// internal fault rather than panicking or becoming an empty body.
    #[test]
    fn token_selection_distinguishes_empty_bodies_from_invalid_ranges() {
        let input = reference_source();
        let lexed = LexedSource::new(&input, crate::lexer::COND_INITIAL);
        let count = lexed.tokens().len();
        let sink = ErrorCollector::new();
        for at in [0, count] {
            let selected = lexed.admit_tokens(at..at, &sink).unwrap();
            assert!(std::ptr::eq(selected.source(), input.as_str()));
            assert_eq!(selected.into_located().count(), 0);
        }
        assert!(sink.is_empty());
        for (start, end) in [(count + 1, count + 1), (1, 0), (0, count + 1)] {
            let errors = ErrorCollector::new();
            assert!(lexed.admit_tokens(start..end, &errors).is_err());
            let findings = errors.into_vec();
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0].code, ErrorCode::InternalError);
            assert_eq!(findings[0].severity, Severity::Error);
        }
    }
}
