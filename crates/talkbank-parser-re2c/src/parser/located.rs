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
