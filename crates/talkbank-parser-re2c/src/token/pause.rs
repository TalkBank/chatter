//! Pause payload and full source extent issued together by the lexer.

use serde::Serialize;
use talkbank_model::Span;

/// A pause's payload text and complete extent, including both parentheses.
/// Only lexical admission constructs this value; parsing and lowering retain it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PauseLexeme<'source> {
    text: &'source str,
    #[serde(skip)]
    span: Span,
}

impl<'source> PauseLexeme<'source> {
    pub(crate) fn from_lexed(text: &'source str, start: usize, end: usize) -> Self {
        Self {
            text,
            span: Span::from_usize(start, end),
        }
    }

    /// The original token payload (numeric content for a timed pause).
    pub fn text(&self) -> &'source str {
        self.text
    }

    /// The full match from the same lexer step that selected the pause kind.
    pub fn span(&self) -> Span {
        self.span
    }
}
