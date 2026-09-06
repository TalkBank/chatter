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

    pub(crate) fn tokens(&self) -> &[Token<'source>] {
        &self.tokens
    }

    /// A token selected by the file cursor and its original lexer location.
    pub(crate) fn token_at(&self, index: usize) -> (&Token<'source>, LexerSpan) {
        (&self.tokens[index], self.spans[index].clone())
    }

    /// The file cursor has consumed a nonempty logical header, including its
    /// newline when present. Derive the extent from the same lexer storage.
    pub(crate) fn header_provenance(
        &self,
        range: Range<usize>,
        separator: talkbank_model::model::TierSeparator,
    ) -> crate::ast::HeaderProvenance {
        crate::ast::HeaderProvenance::from_lexed(
            talkbank_model::Span::from_usize(
                self.spans[range.start].start,
                self.spans[range.end - 1].end,
            ),
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
                    span: talkbank_model::Span::from_usize(start, start + text.len()),
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
                talkbank_model::SourceLocation::from_offsets(span.start, span.end),
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
