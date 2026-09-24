//! Parsers for Conversation Analysis marker tokens.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Unicode_Option>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{AsRawNode, CaDelimiterNode, CaElementNode};
use crate::model::{CADelimiter, CADelimiterType, CAElement, CAElementType};
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;

/// Closed within this module: a generated CA kind determines both the symbol
/// registry and the output model. A caller cannot mix element and delimiter
/// labels, decoders or model constructors.
trait CaToken<'tree>: AsRawNode<'tree> {
    type Output;
    const LABEL: &'static str;
    fn from_char(ch: char, span: Span) -> Option<Self::Output>;
}

impl<'tree> CaToken<'tree> for CaElementNode<'tree> {
    type Output = CAElement;
    const LABEL: &'static str = "element";

    fn from_char(ch: char, span: Span) -> Option<Self::Output> {
        CAElementType::from_char(ch).map(|kind| CAElement::new(kind).with_span(span))
    }
}

impl<'tree> CaToken<'tree> for CaDelimiterNode<'tree> {
    type Output = CADelimiter;
    const LABEL: &'static str = "delimiter";

    fn from_char(ch: char, span: Span) -> Option<Self::Output> {
        CADelimiterType::from_char(ch).map(|kind| CADelimiter::new(kind).with_span(span))
    }
}

/// Converts a generated CA element through checked source admission.
pub(crate) fn parse_ca_element_node(
    node: CaElementNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<CAElement> {
    parse_ca_token(node, source, errors)
}

/// Converts a generated CA delimiter through checked source admission.
pub(crate) fn parse_ca_delimiter_node(
    node: CaDelimiterNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<CADelimiter> {
    parse_ca_token(node, source, errors)
}

fn parse_ca_token<'tree, T: CaToken<'tree>>(
    typed: T,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<T::Output> {
    let node = typed.raw_node();
    // The whole-tree recovery pass reports MISSING (E342); it names no symbol.
    if node.is_missing() {
        return ParseOutcome::rejected();
    }
    let ParseOutcome::Parsed(text) = decode_present_child(&typed, source, errors, "", |err| {
        format!("Failed to extract CA {} text: {err}", T::LABEL)
    }) else {
        return ParseOutcome::rejected();
    };
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let Some(ch) = text.chars().next() else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.byte_range(), ""),
            format!("Empty CA {} token", T::LABEL),
        ));
        return ParseOutcome::rejected();
    };
    match T::from_char(ch, span) {
        Some(value) => ParseOutcome::parsed(value),
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.byte_range(), ""),
                format!("Unknown CA {} character '{ch}'", T::LABEL),
            ));
            ParseOutcome::rejected()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::error::ErrorCollector;
    use crate::generated_traversal::FromNodeKind;

    /// Real typed tokens retain their registry and reject incompatible sources
    /// at the boundary, rather than indexing past them or inventing symbols.
    #[test]
    fn reference_ca_tokens_keep_typed_registries_and_checked_source_reads() {
        let parser = TreeSitterParser::new().expect("grammar");
        let mut elements = 0;
        let mut delimiters = 0;
        for source in [
            include_str!("../../../../../../../corpus/reference/ca/intonation.cha"),
            include_str!("../../../../../../../corpus/reference/ca/nonvocal-and-long-features.cha"),
            include_str!("../../../../../../../corpus/reference/ca/stacked-markers.cha"),
        ] {
            let parsed = parser
                .parse_source_incremental(source, None)
                .expect("parse");
            let wrong_symbols = "x".repeat(source.len());
            let mut pending = vec![parsed.root_node()];
            while let Some(node) = pending.pop() {
                let mut cursor = node.walk();
                pending.extend(node.children(&mut cursor));
                if let Some(typed) = CaElementNode::from_node(node) {
                    check_sources(typed, source, &wrong_symbols);
                    elements += 1;
                }
                if let Some(typed) = CaDelimiterNode::from_node(node) {
                    check_sources(typed, source, &wrong_symbols);
                    delimiters += 1;
                }
            }
        }
        assert!(elements > 0);
        assert!(delimiters > 0);
    }

    fn check_sources<'tree, T: CaToken<'tree> + Copy>(typed: T, source: &str, wrong: &str) {
        let errors = ErrorCollector::new();
        assert!(
            parse_ca_token(typed, source, &errors)
                .into_option()
                .is_some()
        );
        assert!(errors.into_vec().is_empty());
        for incompatible in ["", wrong] {
            let errors = ErrorCollector::new();
            assert!(
                parse_ca_token(typed, incompatible, &errors)
                    .into_option()
                    .is_none()
            );
            let diagnostics = errors.into_vec();
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, ErrorCode::TreeParsingError);
        }
    }
}
