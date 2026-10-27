//! Parser for pause tokens (`(.)`, `(..)`, `(3.5)`, etc.).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Pauses>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{AsRawNode, PauseTokenNode};
use crate::model::{Pause, PauseDuration, TimedPause};
use crate::parser::node_span::span_of;
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
    let span = span_of(node);

    let ParseOutcome::Parsed(text) = extract_utf8_text(node, source, errors, "pause_token") else {
        return ParseOutcome::rejected();
    };

    match text {
        "(.)" => ParseOutcome::parsed(Pause::new(span, PauseDuration::Short)),
        "(..)" => ParseOutcome::parsed(Pause::new(span, PauseDuration::Medium)),
        "(...)" => ParseOutcome::parsed(Pause::new(span, PauseDuration::Long)),
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
                    DiagnosticLocation::new(span_of(node)),
                    ErrorContext::new(source, span_of(node), text),
                    "pause_token is not enclosed in parentheses",
                ));
                return ParseOutcome::rejected();
            };

            let Ok(duration) = TimedPause::new(duration_text) else {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    DiagnosticLocation::new(span_of(node)),
                    ErrorContext::new(source, span_of(node), duration_text),
                    "pause_token duration has invalid CHAT spelling",
                ));
                return ParseOutcome::rejected();
            };

            ParseOutcome::parsed(Pause::new(span, PauseDuration::Timed(duration)))
        }
    }
}
