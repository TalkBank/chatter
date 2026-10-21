//! Main-tier separator validity, including legacy syntax retained by parsing.

use crate::alignment::helpers::{ContentItem, walk_content};
use crate::model::{Separator, Utterance};
use crate::{ErrorCode, ErrorSink, ParseError, Severity};

/// Current CHAT disallows main-tier semicolons, including nested/retraced ones.
/// The typed separator owns its source span; header and opaque tier text never
/// enter this classification. Parsing a legacy separator is not validating it.
pub(crate) fn check_semicolon_separators(utterance: &Utterance, errors: &impl ErrorSink) {
    walk_content(&utterance.main.content.content, None, &mut |item| {
        if let ContentItem::Separator(Separator::Semicolon { location }) = item {
            errors.report(
                ParseError::at_location(
                    ErrorCode::SemicolonOnMainTier,
                    Severity::Error,
                    *location,
                    "Semicolons are not allowed on main tiers",
                )
                .with_suggestion("Split separate utterances; use ↘ for a CA light final drop"),
            );
        }
    });
}
