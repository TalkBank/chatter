//! Iterator-owned progress through one immutable lexer run.

use std::iter::{Cloned, Enumerate, Peekable, Zip};
use std::slice::Iter;

use super::LexedSource;
use crate::{lexer::LexerSpan, token::Token};

type LocatedIter<'tokens, 'source> =
    Enumerate<Zip<Iter<'tokens, Token<'source>>, Cloned<Iter<'tokens, LexerSpan>>>>;

/// An observed token, its position and byte span from the same lexer owner.
/// A caller cannot request an out-of-bounds token or substitute its source.
pub(crate) struct CursorToken<'tokens, 'source> {
    source: &'source str,
    index: usize,
    token: &'tokens Token<'source>,
    span: LexerSpan,
}

impl<'tokens, 'source> CursorToken<'tokens, 'source> {
    pub(crate) fn index(&self) -> usize {
        self.index
    }
    pub(crate) fn token(&self) -> &'tokens Token<'source> {
        self.token
    }
    pub(crate) fn span(&self) -> LexerSpan {
        self.span.clone()
    }

    /// A newline left behind after nonempty-line recovery is not blank.
    pub(crate) fn report_blank_line(&self, errors: &impl talkbank_model::ErrorSink) {
        let span = &self.span;
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
}

/// Progress is the remaining iterator, never a caller-writable integer.
/// End-of-input is ordinary absence; peeking never indexes a token vector.
pub(crate) struct TokenCursor<'tokens, 'source> {
    owner: &'tokens LexedSource<'source>,
    remaining: Peekable<LocatedIter<'tokens, 'source>>,
}

impl<'tokens, 'source> TokenCursor<'tokens, 'source> {
    pub(super) fn new(owner: &'tokens LexedSource<'source>) -> Self {
        Self {
            owner,
            remaining: owner
                .tokens
                .iter()
                .zip(owner.spans.iter().cloned())
                .enumerate()
                .peekable(),
        }
    }

    pub(crate) fn owner(&self) -> &'tokens LexedSource<'source> {
        self.owner
    }

    pub(crate) fn position(&self) -> usize {
        self.owner.tokens.len() - self.remaining.len()
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.remaining.len() == 0
    }

    pub(crate) fn peek(&mut self) -> Option<CursorToken<'tokens, 'source>> {
        self.remaining
            .peek()
            .map(|(index, (token, span))| CursorToken {
                source: self.owner.source,
                index: *index,
                token,
                span: span.clone(),
            })
    }

    pub(crate) fn advance(&mut self) {
        let _ = self.remaining.next();
    }

    /// Inspect the end of the current logical line without consuming it.
    pub(crate) fn line_end_position(&self) -> usize {
        self.position()
            + self
                .remaining
                .clone()
                .take_while(|(_, (token, _))| !matches!(token, Token::Newline(_)))
                .count()
    }

    pub(crate) fn skip_to_newline(&mut self) {
        while self
            .remaining
            .next_if(|(_, (token, _))| !matches!(token, Token::Newline(_)))
            .is_some()
        {}
    }

    pub(crate) fn take_newline(&mut self) -> Option<LexerSpan> {
        self.remaining
            .next_if(|(_, (token, _))| matches!(token, Token::Newline(_)))
            .map(|(_, (_, span))| span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peeking_preserves_position_and_exhaustion_cannot_advance_past_input() {
        let source = include_str!("../../../../../corpus/reference/content/words-prosody.cha");
        let owner = LexedSource::new(source, crate::lexer::COND_INITIAL);
        let mut cursor = owner.cursor();
        assert!(
            cursor.take_newline().is_none(),
            "initial header is not a newline"
        );
        for (index, (token, span)) in owner.located().enumerate() {
            for _ in 0..2 {
                let current = cursor.peek().expect("remaining reference token");
                assert_eq!(current.index(), index);
                assert_eq!(cursor.position(), index);
                assert_eq!(current.token(), token);
                assert_eq!(
                    (current.span.start, current.span.end),
                    (span.start, span.end)
                );
                assert!(std::ptr::eq(current.source, source));
            }
            cursor.advance();
        }
        let end = owner.tokens().len();
        assert!(cursor.is_finished());
        assert_eq!(cursor.position(), end);
        for _ in 0..2 {
            assert!(cursor.peek().is_none());
            assert!(cursor.take_newline().is_none());
            cursor.skip_to_newline();
            cursor.advance();
            assert_eq!(cursor.position(), end);
            assert_eq!(cursor.line_end_position(), end);
        }
    }
}
