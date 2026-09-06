//! Postcode payload admission retains the lexer's complete source location.

use serde::Serialize;
use talkbank_model::{ErrorCode, ErrorSink, ParseError, Postcode, Severity, SourceLocation, Span};

/// A postcode matched by the lexer, including recoverable missing content.
/// Its private state is classified once, before model conversion.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PostcodeToken<'a> {
    content: Content<'a>,
    #[serde(skip)]
    span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
enum Content<'a> {
    Present(&'a str),
    Missing,
}

impl<'a> PostcodeToken<'a> {
    /// The lexer supplies tag-extracted content and its full matched span.
    pub(crate) fn from_lexed(payload: &'a str, span: Span) -> Self {
        let text = payload.trim_end();
        let content = if text.is_empty() {
            Content::Missing
        } else {
            Content::Present(text)
        };
        Self { content, span }
    }

    /// The canonical payload preserves leading whitespace.
    pub fn text(&self) -> &'a str {
        match self.content {
            Content::Present(text) => text,
            Content::Missing => "",
        }
    }

    /// Lower admitted content, or report the recoverable lexical match.
    pub(crate) fn to_model(&self, errors: &(impl ErrorSink + ?Sized)) -> Option<Postcode> {
        match self.content {
            Content::Present(text) => Some(Postcode::new(text).with_span(self.span)),
            Content::Missing => {
                errors.report(ParseError::new(
                    ErrorCode::InvalidPostcode,
                    Severity::Error,
                    SourceLocation::new(self.span),
                    None,
                    "Postcode is missing required content",
                ));
                None
            }
        }
    }
}
