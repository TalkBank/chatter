//! Nonempty regions admitted by the lexer storage that owns their coordinates.

use std::ops::Range;

use super::LexedSource;
use crate::{
    lexer::LexerSpan,
    source_text::{InvalidSourceRange, SourceSlice, SourceText},
    token::Token,
};
use talkbank_model::{ErrorSink, Span};

/// A selected region cannot be described by the current lexer run.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RegionFault {
    #[error(transparent)]
    Source(#[from] InvalidSourceRange),
    #[error("native lexer region at token {at} is empty")]
    Empty { at: usize },
    #[error("native lexer region {start}..{end} is outside its {tokens} tokens")]
    Outside {
        start: usize,
        end: usize,
        tokens: usize,
    },
}

/// Refusal whose internal diagnostic has already reached the caller's sink.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub(crate) struct ReportedRegionFailure(RegionFault);

impl RegionFault {
    pub(super) fn report(self, errors: &impl ErrorSink) -> ReportedRegionFailure {
        errors.report(talkbank_model::ParseError::new(
            talkbank_model::ErrorCode::InternalError,
            talkbank_model::Severity::Error,
            talkbank_model::DiagnosticLocation::Unlocated,
            None,
            self.to_string(),
        ));
        ReportedRegionFailure(self)
    }
}

/// Immutable tokens, spans and original source admitted as one nonempty region.
///
/// Only the owning `LexedSource` can construct this value. Consumers never
/// receive an independent source or a raw range to pair with its endpoints.
pub(crate) struct NonEmptyRegion<'tokens, 'source> {
    source: SourceSlice<'source>,
    tokens: &'tokens [Token<'source>],
    spans: &'tokens [LexerSpan],
    first: &'tokens LexerSpan,
    last: &'tokens LexerSpan,
    ending: RegionEnding,
}

enum RegionEnding {
    LogicalNewline,
    BufferEnd,
    UnprovenStop,
}

impl<'tokens, 'source> NonEmptyRegion<'tokens, 'source> {
    pub(super) fn from_range(
        owner: &'tokens LexedSource<'source>,
        range: Range<usize>,
    ) -> Result<Self, RegionFault> {
        let at = range.start;
        let selected = owner.select_range(range)?;
        Self::from_slices(selected.source, selected.tokens, selected.spans, at)
    }

    pub(super) fn whole(owner: &'tokens LexedSource<'source>) -> Result<Option<Self>, RegionFault> {
        if owner.tokens.is_empty() {
            return Ok(None);
        }
        Self::from_slices(owner.source, &owner.tokens, &owner.spans, 0).map(Some)
    }

    fn from_slices(
        source: &'source str,
        tokens: &'tokens [Token<'source>],
        spans: &'tokens [LexerSpan],
        at: usize,
    ) -> Result<Self, RegionFault> {
        // Both slices come from the same range of the owner's one unzip.
        // Their storage is private and cannot change during this borrow.
        let first = spans.first().ok_or(RegionFault::Empty { at })?;
        let last = spans.last().ok_or(RegionFault::Empty { at })?;
        let last_token = tokens.last().ok_or(RegionFault::Empty { at })?;
        let ending = if matches!(last_token, Token::Newline(_)) {
            RegionEnding::LogicalNewline
        } else if last.end == source.len() {
            RegionEnding::BufferEnd
        } else {
            RegionEnding::UnprovenStop
        };
        let source = SourceText::new(source).slice(first.start..last.end)?;
        Ok(Self {
            source,
            tokens,
            spans,
            first,
            last,
            ending,
        })
    }

    /// Borrow the admitted tokens without another index-range selection.
    pub(crate) fn tokens(&self) -> &'tokens [Token<'source>] {
        self.tokens
    }

    /// The original source borrowed by every token in this region.
    pub(crate) fn source_text(&self) -> crate::source_text::SourceText<'source> {
        self.source.source()
    }

    /// Its first token through its last token, including a selected ending.
    pub(crate) fn extent(&self) -> Span {
        self.source.span()
    }

    /// Retain the admitted bytes and their original source together.
    pub(crate) fn source_slice(&self) -> SourceSlice<'source> {
        self.source
    }

    /// Retain token-to-source association during native recovery filtering.
    pub(crate) fn located(
        &self,
    ) -> impl Iterator<Item = (&'tokens Token<'source>, LexerSpan)> + '_ {
        self.tokens.iter().zip(self.spans.iter().cloned())
    }

    /// Inspect original whitespace items only after their region is admitted.
    pub(super) fn whitespace_items(self) -> impl Iterator<Item = super::LocatedItem<'source>> {
        let extent = self.extent();
        self.source
            .text()
            .split_inclusive(char::is_whitespace)
            .scan(extent.start(), |cursor, chunk| {
                let start = *cursor;
                *cursor += chunk.len();
                let text = chunk.trim_end_matches(char::is_whitespace);
                Some(super::LocatedItem {
                    text,
                    span: Span::new(start, start + text.len()),
                })
            })
            .filter(|item| !item.text.is_empty())
    }

    /// Diagnose this region using only its owning original source.
    pub(crate) fn report_error(
        &self,
        code: talkbank_model::ErrorCode,
        message: &str,
        errors: &impl ErrorSink,
    ) {
        let extent = self.extent();
        errors.report(talkbank_model::ParseError::new(
            code,
            talkbank_model::Severity::Error,
            talkbank_model::DiagnosticLocation::new(extent),
            talkbank_model::ErrorContext::new(
                self.source.source().whole_slice().text(),
                extent,
                "native_region",
            ),
            message,
        ));
    }

    /// Admit the native main-tier head and place its whole original line.
    pub(crate) fn main_tier_locations(&self) -> Option<talkbank_model::MainTierLocations> {
        use talkbank_model::{Location, MainTierLocations};
        match (self.tokens, self.spans) {
            (
                [Token::Star(_), Token::Speaker(_), Token::TierSep(_), ..],
                [_, speaker, separator, ..],
            ) => {
                let extent = self.extent();
                Some(MainTierLocations {
                    line: Location::Located(extent),
                    speaker: Location::Located(Span::new(speaker.start, speaker.end)),
                    // Every TierSep starts with its colon; the tab is content.
                    content: Location::Located(Span::new(
                        separator.start + ":".len(),
                        extent.end(),
                    )),
                })
            }
            _ => None,
        }
    }

    /// A dependent prefix's end through its logical ending or proven EOF.
    pub(crate) fn dependent_content_span(
        &self,
    ) -> Result<
        Option<talkbank_model::model::dependent_tier::DependentTierContentSpan>,
        talkbank_model::model::dependent_tier::InvalidDependentTierContentSpan,
    > {
        let body_end = match self.ending {
            RegionEnding::LogicalNewline => self.last.start,
            // A selected region that stops before the buffer end cannot
            // certify EOF or a complete original body.
            RegionEnding::BufferEnd => self.last.end,
            RegionEnding::UnprovenStop => return Ok(None),
        };
        talkbank_model::model::dependent_tier::DependentTierContentSpan::try_new(
            self.extent(),
            self.first.end,
            body_end,
        )
        .map(Some)
    }

    /// Header provenance from this admitted logical line, not caller indices.
    pub(crate) fn locate_header(
        &self,
        header: crate::ast::HeaderParsed<'source>,
        separator: talkbank_model::model::TierSeparator,
    ) -> crate::ast::LocatedHeader<'source> {
        crate::ast::LocatedHeader::from_lexed(header, self.source_slice(), separator)
    }
}
