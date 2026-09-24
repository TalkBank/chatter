//! Parser for pause tokens (`(.)`, `(..)`, `(3.5)`, etc.).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Pauses>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{AsRawNode, PauseTokenNode};
use crate::model::{Pause, PauseDuration, PauseTimedDuration};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use talkbank_model::ParseOutcome;

/// Parse a pause node to Pause enum using token text dispatch.
///
/// After coarsening, `pause_token` is a single atomic leaf token that matches
/// `(.)`, `(..)`, `(...)`, or timed patterns like `(3.5)` / `(3:2.5)`.
/// We dispatch on the token text to determine the pause type.
pub(crate) fn parse_pause_node(
    typed: PauseTokenNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Pause> {
    let node = typed.raw_node();
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    let ParseOutcome::Parsed(text) = extract_utf8_text(node, source, errors, "pause_token") else {
        return ParseOutcome::rejected();
    };

    match text {
        "(.)" => ParseOutcome::parsed(Pause::new(PauseDuration::Short).with_span(span)),
        "(..)" => ParseOutcome::parsed(Pause::new(PauseDuration::Medium).with_span(span)),
        "(...)" => ParseOutcome::parsed(Pause::new(PauseDuration::Long).with_span(span)),
        _ => {
            // Timed pause: the duration between the parentheses the token
            // guarantees (`/\(\d+(?::\d+)?\.\d*\)/`). A token without them
            // does not fit its own grammar and is reported as the traversal's
            // failure, never read whole as a duration.
            let Some(duration_text) = text.strip_prefix('(').and_then(|s| s.strip_suffix(')'))
            else {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), text),
                    "pause_token is not enclosed in parentheses",
                ));
                return ParseOutcome::rejected();
            };

            if duration_text.is_empty() {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), duration_text),
                    "pause_token duration is empty",
                ));
                return ParseOutcome::rejected();
            }

            ParseOutcome::parsed(
                Pause::new(PauseDuration::Timed(PauseTimedDuration::new(
                    duration_text.to_string(),
                )))
                .with_span(span),
            )
        }
    }
}
