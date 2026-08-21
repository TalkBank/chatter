//! Parsers for Conversation Analysis marker tokens.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Unicode_Option>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{CADelimiter, CADelimiterType, CAElement, CAElementType};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

// The two character-to-variant dispatch tables that used to live here are
// gone. They were a hand-written copy of the symbol registry, one arm per
// symbol, sitting next to `to_symbol`, which is the same table written
// backwards. Both directions are now generated from one record per symbol:
// `CAElementType::from_char` and `CADelimiterType::from_char`.

/// Converts one `ca_element` token node to `CAElement`.
///
/// After coarsening, ca_element is a single-character token.
/// We dispatch by examining the token text character.
pub(crate) fn parse_ca_element_node(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<CAElement> {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let text = extract_utf8_text(node, source, errors, "ca_element", "");

    let Some(ch) = text.chars().next() else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "Empty CA element token",
        ));
        return ParseOutcome::rejected();
    };

    match CAElementType::from_char(ch) {
        Some(element_type) => ParseOutcome::parsed(CAElement::new(element_type).with_span(span)),
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!("Unknown CA element character '{ch}'"),
            ));
            ParseOutcome::rejected()
        }
    }
}

/// Converts one `ca_delimiter` token node to `CADelimiter`.
///
/// After coarsening, ca_delimiter is a single-character token.
/// We dispatch by examining the token text character.
pub(crate) fn parse_ca_delimiter_node(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<CADelimiter> {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let text = extract_utf8_text(node, source, errors, "ca_delimiter", "");

    let Some(ch) = text.chars().next() else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "Empty CA delimiter token",
        ));
        return ParseOutcome::rejected();
    };

    match CADelimiterType::from_char(ch) {
        Some(delimiter_type) => {
            ParseOutcome::parsed(CADelimiter::new(delimiter_type).with_span(span))
        }
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!("Unknown CA delimiter character '{ch}'"),
            ));
            ParseOutcome::rejected()
        }
    }
}
