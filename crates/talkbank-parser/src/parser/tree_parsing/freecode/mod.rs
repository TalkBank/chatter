//! Parser for main-tier freecode spans (`[^ ... ]`).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Freecodes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{AsRawNode, FreecodeNode};
use crate::model::{Freecode, UtteranceContent};
use talkbank_model::ParseOutcome;

/// Parse freecode node [^ text] into UtteranceContent.
///
/// **Grammar Rule (coarsened):**
/// ```text
/// freecode: $ => token(/\[\^ [^\]\r\n]+\]/)
/// ```
///
/// The node is now a single leaf token. Extract the code by stripping
/// the `[^ ` prefix and `]` suffix, then trimming trailing whitespace.
pub fn parse_freecode(
    typed: FreecodeNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let node = typed.raw_node();
    let text = match source.get(node.byte_range()) {
        Some(t) => t,
        None => {
            errors.report(ParseError::new(
                ErrorCode::EmptyUtterance,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "freecode"),
                "Freecode node range is not a UTF-8 slice of the supplied source",
            ));
            return ParseOutcome::rejected();
        }
    };

    // Strip "[^ " prefix and "]" suffix
    let code = text
        .strip_prefix("[^ ")
        .and_then(|s| s.strip_suffix(']'))
        .map(|s| s.trim_end());

    match code {
        Some(c) if !c.is_empty() => {
            let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
            let freecode = Freecode::with_span(c, span);
            ParseOutcome::parsed(UtteranceContent::Freecode(freecode))
        }
        _ => {
            errors.report(ParseError::new(
                ErrorCode::EmptyUtterance,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "freecode"),
                "Empty freecode content",
            ));
            ParseOutcome::rejected()
        }
    }
}
