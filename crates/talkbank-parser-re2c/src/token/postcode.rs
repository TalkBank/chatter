//! Postcode payload admission retains the lexer's complete lexeme.

use serde::Serialize;
use talkbank_model::{ErrorCode, ErrorSink, ParseError, Postcode, Severity};

/// A postcode matched by the lexer, including recoverable missing content.
/// The shared model classifies content during conversion; the whole
/// `[+ code]` is kept for placement.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PostcodeToken<'a> {
    payload: &'a str,
    #[serde(skip)]
    lexeme: &'a str,
}

impl<'a> PostcodeToken<'a> {
    /// The lexer supplies tag-extracted content and the full match.
    pub(crate) fn from_lexed(payload: &'a str, lexeme: &'a str) -> Self {
        Self { payload, lexeme }
    }

    /// The postcode as written, `[+ code]`, for placement.
    pub fn lexeme(&self) -> &'a str {
        self.lexeme
    }

    /// The canonical payload preserves leading whitespace.
    pub fn text(&self) -> &'a str {
        self.payload.trim_end()
    }

    /// Lower admitted content at the postcode's own bytes in `source`, or
    /// report the recoverable lexical match there.
    pub(crate) fn to_model(
        &self,
        source: crate::source_text::SourceText<'_>,
        errors: &(impl ErrorSink + ?Sized),
    ) -> Postcode {
        let value = Postcode::from_lexed(
            crate::convert::locate(source, self.lexeme),
            self.payload,
            self.lexeme,
        );
        if value.text().is_none() {
            errors.report(ParseError::new(
                ErrorCode::InvalidPostcode,
                Severity::Error,
                crate::convert::diagnostic_location(source, self.lexeme),
                None,
                "Postcode is missing required content",
            ));
        }
        value
    }
}
