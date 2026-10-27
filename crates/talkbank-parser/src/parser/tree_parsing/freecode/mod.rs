//! Parser for main-tier freecode spans (`[^ ... ]`).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Freecodes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{AsRawNode, FreecodeNode, SourceBound};
use crate::model::{Freecode, UtteranceContent};
use crate::parser::node_span::span_of;
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
pub fn parse_freecode<'tree>(
    typed: SourceBound<'tree, '_, FreecodeNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let node = typed.raw_node();
    let source = typed.source();
    let text = typed.text();
    let value = match text.strip_prefix("[^ ").and_then(|s| s.strip_suffix(']')) {
        Some(payload) => Freecode::from_lexed(span_of(node), payload, text),
        None => Freecode::unparsed(span_of(node), text),
    };
    if value.text().is_none() {
        errors.report(ParseError::new(
            ErrorCode::UnparsableContent,
            Severity::Error,
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(source, span_of(node), "freecode"),
            "Freecode is missing required content",
        ));
    }
    ParseOutcome::parsed(UtteranceContent::Freecode(value))
}
