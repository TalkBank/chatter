//! Postcode payload admission retains the lexer's complete lexeme.

use serde::Serialize;
use talkbank_model::{ErrorCode, ErrorSink, ParseError, Postcode, Severity};

/// A postcode matched by the lexer, including recoverable missing content.
/// Its private state is classified once, before model conversion; the whole
/// `[+ code]` is kept for placement.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PostcodeToken<'a> {
    content: Content<'a>,
    #[serde(skip)]
    lexeme: &'a str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
enum Content<'a> {
    Present(&'a str),
    Missing,
}

impl<'a> PostcodeToken<'a> {
    /// The lexer supplies tag-extracted content and the full match.
    pub(crate) fn from_lexed(payload: &'a str, lexeme: &'a str) -> Self {
        let text = payload.trim_end();
        let content = if text.is_empty() {
            Content::Missing
        } else {
            Content::Present(text)
        };
        Self { content, lexeme }
    }

    /// The postcode as written, `[+ code]`, for placement.
    pub fn lexeme(&self) -> &'a str {
        self.lexeme
    }

    /// The canonical payload preserves leading whitespace.
    pub fn text(&self) -> &'a str {
        match self.content {
            Content::Present(text) => text,
            Content::Missing => "",
        }
    }

    /// Lower admitted content at the postcode's own bytes in `source`, or
    /// report the recoverable lexical match there.
    pub(crate) fn to_model(
        &self,
        source: crate::source_text::SourceText<'_>,
        errors: &(impl ErrorSink + ?Sized),
    ) -> Option<Postcode> {
        match self.content {
            Content::Present(text) => Some(Postcode::new(
                crate::convert::locate(source, self.lexeme),
                text,
            )),
            Content::Missing => {
                errors.report(ParseError::new(
                    ErrorCode::InvalidPostcode,
                    Severity::Error,
                    crate::convert::diagnostic_location(source, self.lexeme),
                    None,
                    "Postcode is missing required content",
                ));
                None
            }
        }
    }
}
