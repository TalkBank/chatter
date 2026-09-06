//! Admit a whole logical header before converting its recovered AST.

use crate::ast::{HeaderParsed, Line};
use crate::token::Token;
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Only admission can produce a header accounting for the complete fragment.
pub(crate) struct HeaderFragment<'source> {
    header: Box<HeaderParsed<'source>>,
}

impl<'source> HeaderFragment<'source> {
    pub(crate) fn parse(input: &'source str, errors: &impl ErrorSink) -> Option<Self> {
        let lexed = super::entry_points::lex_file_source(input, errors);
        // Continuations are lexer tokens within a logical header, not line ends.
        // Inspect the original suffix after a logical newline, so even an
        // unsupported trailing line (which file recovery could drop) is refused.
        if let Some((_, span)) = lexed
            .located(0..lexed.tokens().len())
            .find(|(token, _)| matches!(token, Token::Newline(_)))
            && !input[span.end..].trim().is_empty()
        {
            return Self::reject(input, errors);
        }
        // The lexer accepts a legacy NUL sentinel. A fragment must still account
        // for every byte of its caller's input instead of silently truncating it.
        if let Some(last) = lexed.tokens().len().checked_sub(1) {
            let (_, span) = lexed.token_at(last);
            if !input[span.end..].trim().is_empty() {
                return Self::reject(input, errors);
            }
        }
        let parsed = super::file::parse_file_with_errors(&lexed, errors);
        let mut lines = parsed.lines.into_iter();
        match (lines.next(), lines.next()) {
            (Some(Line::Header { header, .. }), None) => Some(Self { header }),
            _ => Self::reject(input, errors),
        }
    }

    pub(crate) fn lower(self) -> talkbank_model::model::Header {
        crate::convert::header_parsed_to_model(&self.header)
    }

    fn reject(input: &str, errors: &impl ErrorSink) -> Option<Self> {
        errors.report(ParseError::new(
            ErrorCode::TierValidationError,
            Severity::Error,
            SourceLocation::from_offsets(0, input.len()),
            ErrorContext::new(input, 0..input.len(), "header"),
            "Expected exactly one complete header fragment",
        ));
        None
    }
}
