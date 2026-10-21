//! Raw dependent-tier recovery that cannot be mistaken for a valid text tier.

use crate::token::Token;
use serde::Serialize;
use talkbank_model::{DiagnosticLocation, ErrorCode, ErrorSink, ParseError, Severity, Span};

/// A rejected tier retains its lexical input for AST inspection. Construction
/// streams its diagnostic; model conversion must not fabricate a supported or
/// unsupported model tier from this recovery state.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RejectedMorTier<'a> {
    prefix: Token<'a>,
    content: Vec<Token<'a>>,
}

impl<'a> RejectedMorTier<'a> {
    pub(crate) fn report(
        prefix: Token<'a>,
        content: &[Token<'a>],
        span: Span,
        errors: &impl ErrorSink,
    ) -> Self {
        errors.report(ParseError::new(
            ErrorCode::TierValidationError,
            Severity::Error,
            DiagnosticLocation::new(span),
            None,
            "Could not fully parse morphological tier",
        ));
        Self {
            prefix,
            content: content.to_vec(),
        }
    }
}
