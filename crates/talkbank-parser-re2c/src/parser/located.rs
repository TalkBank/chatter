//! Own the source/token/span relationship for source-aware file parsing.

use std::ops::Range;

use crate::lexer::{Lexer, LexerSpan};
use crate::token::Token;

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
