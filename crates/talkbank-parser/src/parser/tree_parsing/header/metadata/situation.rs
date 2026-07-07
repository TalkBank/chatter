//! Parsing for `@Situation` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>

use crate::generated_traversal::{AsRawNode, SituationHeaderNode, extract_situation_header};
use crate::node_types::SITUATION_HEADER;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, SituationDescription, WarningText};

/// Build `Header::Unknown` for malformed `@Situation` input.
fn unknown_situation_header(node: Node, source: &str, parse_reason: impl Into<String>) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@Situation".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @Situation:\t<description>".to_string()),
    }
}

/// Parse Situation header from tree-sitter node
///
/// **Grammar Rule:**
/// ```javascript
/// situation_header: $ => seq(
///     '@', 'Situation', $.colon, $.tab,
///     $.free_text,    // Position 4
///     $.newline
/// )
/// ```
pub fn parse_situation_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is a situation_header node
    if node.kind() != SITUATION_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected situation_header node, got: {}", node.kind()),
        ));
        return unknown_situation_header(
            node,
            source,
            "Situation header CST node had unexpected kind",
        );
    }

    // Grammar: seq(situation_prefix, header_sep, free_text, newline). The free
    // text is DIRECT at `child_2` (there is no inner contents node, and no
    // interstitial whitespace position at this level, so the index is unchanged
    // from the OLD module); read it through the NEW backend's free
    // `extract_situation_header`. `present_or_recover().ok()` keeps only a
    // Present free_text; every non-Present recovery state funnels to the SAME
    // "missing situation text" diagnostic at the HEADER NODE span, exactly as the
    // pre-migration `find_child_by_kind` None branch did.
    let children = extract_situation_header(SituationHeaderNode(node));
    let Some(free_text) = children.child_2.slot.present_or_recover().ok() else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(
                source,
                node.start_byte()..node.end_byte(),
                "situation_header",
            ),
            "Missing situation text in @Situation header",
        ));
        surface_unexpected(&children.unexpected, source, errors);
        return unknown_situation_header(
            node,
            source,
            "Missing situation text in @Situation header",
        );
    };

    // Decode through the shared `decode_present_child` helper, which reads from the
    // RAW node's `utf8_text` (NOT the wrapper's `.text()` accessor, which swallows
    // UTF-8 errors via `unwrap_or("")`), reproducing the pre-migration
    // `find_child_by_kind` Ok/Err handling. The `@Situation`-specific diagnostic
    // (context = `"situation_text"`) is supplied here, so it stays byte-identical;
    // on rejection we return the same `Header::Unknown`.
    let ParseOutcome::Parsed(text) = decode_present_child(
        free_text.raw_node(),
        source,
        errors,
        "situation_text",
        |err| format!("Failed to extract @Situation text as UTF-8: {}", err),
    ) else {
        surface_unexpected(&children.unexpected, source, errors);
        return unknown_situation_header(node, source, "Could not decode @Situation text");
    };

    surface_unexpected(&children.unexpected, source, errors);
    Header::Situation {
        text: SituationDescription::new(text),
    }
}
