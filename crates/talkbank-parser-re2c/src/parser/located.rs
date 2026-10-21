//! Own the source/token/span relationship for source-aware file parsing.

use std::ops::Range;

use crate::lexer::{Lexer, LexerSpan};
use crate::token::Token;

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

/// Only lexing constructs this immutable owner of parallel token/span storage.
pub(crate) struct LexedSource<'source> {
    source: &'source str,
    tokens: Vec<Token<'source>>,
    spans: Vec<LexerSpan>,
}

impl<'source> LexedSource<'source> {
    pub(crate) fn new(source: &'source str, condition: usize) -> Self {
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

    /// Diagnose a nonempty region selected from this lexer run. Never derive
    /// locations or previews by concatenating delimiter-stripped payloads.
    pub(crate) fn report_error(
        &self,
        range: Range<usize>,
        code: talkbank_model::ErrorCode,
        message: &str,
        errors: &impl talkbank_model::ErrorSink,
    ) {
        let start = self.spans[range.start].start;
        let end = self.spans[range.end - 1].end;
        errors.report(talkbank_model::ParseError::new(
            code,
            talkbank_model::Severity::Error,
            talkbank_model::DiagnosticLocation::from_offsets(start, end),
            talkbank_model::ErrorContext::new(
                self.source,
                talkbank_model::Span::new(start, end),
                "native_region",
            ),
            message,
        ));
    }

    /// The source extent of a nonempty token range selected by the file
    /// cursor: from the first token's start to the last token's end.
    pub(crate) fn extent(&self, range: Range<usize>) -> talkbank_model::Span {
        talkbank_model::Span::new(self.spans[range.start].start, self.spans[range.end - 1].end)
    }

    /// The locations of a main tier whose line the cursor selected, including
    /// its line ending: the line, the speaker token and the content region,
    /// which runs from the end of the separator's colon (so the tab is
    /// content, as in the tree-sitter grammar) to the end of the line.
    ///
    /// `None` when the line does not begin `*`, speaker, separator. The
    /// main-tier grammar refuses such a line before this is asked, so a
    /// caller that still gets `None` has paired a parse with the wrong range
    /// and must report that rather than lower an unlocated tier.
    pub(crate) fn main_tier_locations(
        &self,
        line: Range<usize>,
    ) -> Option<talkbank_model::MainTierLocations> {
        use talkbank_model::{Location, MainTierLocations, Span};
        /// Every `TierSep` lexeme begins with the colon; content starts after it.
        const COLON: &str = ":";
        let extent = self.extent(line.clone());
        match &self.tokens[line.clone()] {
            [Token::Star(_), Token::Speaker(_), Token::TierSep(_), ..] => {
                let speaker = &self.spans[line.start + 1];
                let separator = &self.spans[line.start + 2];
                Some(MainTierLocations {
                    line: Location::Located(extent),
                    speaker: Location::Located(Span::new(speaker.start, speaker.end)),
                    content: Location::Located(Span::new(
                        separator.start + COLON.len(),
                        extent.end(),
                    )),
                })
            }
            _ => None,
        }
    }

    /// A token selected by the file cursor and its original lexer location.
    pub(crate) fn token_at(&self, index: usize) -> (&Token<'source>, LexerSpan) {
        (&self.tokens[index], self.spans[index].clone())
    }

    /// The cursor selected one dependent prefix, its body and its optional
    /// ending from this lexer run. Rich payload slices are not range proxies.
    pub(crate) fn dependent_content_span(
        &self,
        line: Range<usize>,
        ending: usize,
    ) -> Result<
        Option<talkbank_model::model::dependent_tier::DependentTierContentSpan>,
        talkbank_model::model::dependent_tier::InvalidDependentTierContentSpan,
    > {
        use talkbank_model::Span;
        use talkbank_model::model::dependent_tier::DependentTierContentSpan;
        let line_end = self.spans[line.end - 1].end;
        let body_end = match self.spans.get(ending) {
            Some(newline) => newline.start,
            None if line_end == self.source.len() => line_end,
            // Token exhaustion can mean an embedded NUL stopped the lexer.
            // Neither that stop nor an unconsumed buffer tail proves EOF.
            None => return Ok(None),
        };
        DependentTierContentSpan::try_new(
            Span::new(self.spans[line.start].start, line_end),
            self.spans[line.start].end,
            body_end,
        )
        .map(Some)
    }

    /// The file cursor has consumed a nonempty logical header, including its
    /// newline when present. Derive the extent from the same lexer storage.
    pub(crate) fn header_provenance(
        &self,
        range: Range<usize>,
        separator: talkbank_model::model::TierSeparator,
    ) -> crate::ast::HeaderProvenance {
        crate::ast::HeaderProvenance::from_lexed(
            talkbank_model::Span::new(self.spans[range.start].start, self.spans[range.end - 1].end),
            separator,
        )
    }

    /// Preserve exact source bytes and locations while inspecting a token range.
    /// Rich tokens can expose only a payload through `text()`; concatenating
    /// those payloads would lose spelling and positions in malformed content.
    pub(crate) fn whitespace_items(
        &self,
        range: Range<usize>,
    ) -> impl Iterator<Item = LocatedItem<'source>> {
        let (text, start) = if range.is_empty() {
            ("", 0)
        } else {
            let start = self.spans[range.start].start;
            let end = self.spans[range.end - 1].end;
            (&self.source[start..end], start)
        };
        text.split_inclusive(char::is_whitespace)
            .scan(start, |cursor, chunk| {
                let start = *cursor;
                *cursor += chunk.len();
                let text = chunk.trim_end_matches(char::is_whitespace);
                Some(LocatedItem {
                    text,
                    span: talkbank_model::Span::new(start, start + text.len()),
                })
            })
            .filter(|item| !item.text.is_empty())
    }

    /// Diagnose a standalone newline selected by the file dispatcher. A
    /// newline left behind after recovery on a nonempty line is not blank.
    pub(crate) fn report_blank_line(&self, index: usize, errors: &impl talkbank_model::ErrorSink) {
        let span = &self.spans[index];
        if span.start == 0
            || matches!(
                self.source.as_bytes().get(span.start - 1),
                Some(b'\r' | b'\n')
            )
        {
            errors.report(talkbank_model::ParseError::new(
                talkbank_model::ErrorCode::BlankLineNotAllowed,
                talkbank_model::Severity::Error,
                talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
                None,
                "Blank lines are not allowed",
            ));
        }
    }

    /// A range selected by the file parser's token cursor retains its locations.
    pub(crate) fn located(
        &self,
        range: Range<usize>,
    ) -> impl Iterator<Item = (&Token<'source>, LexerSpan)> {
        self.tokens[range.clone()]
            .iter()
            .zip(self.spans[range].iter().cloned())
    }
}
