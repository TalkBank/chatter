//! Parsing for `@Situation` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>

use crate::generated_traversal::{AsRawNode, SituationHeaderNode, extract_situation_header};

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::present;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, SituationDescription};

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
pub fn parse_situation_header(
    typed: SituationHeaderNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Header {
    let node = typed.raw_node();

    // Grammar: seq(situation_prefix, header_sep, free_text, newline). The free
    // text is DIRECT at `child_2` (there is no inner contents node, and no
    // interstitial whitespace position at this level, so the index is unchanged
    // from the OLD module); read it through the NEW backend's free
    // `extract_situation_header`. `present_or_recover().ok()` keeps only a
    // Present free_text; every non-Present recovery state funnels to the SAME
    // "missing situation text" diagnostic at the HEADER NODE span, exactly as the
    // pre-migration `find_child_by_kind` None branch did.
    let children = extract_situation_header(typed);
    let Some(free_text) = present(children.child_2.slot()) else {
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
        surface_displaced(&children.unexpected, "situation_header", source, errors);
        return super::super::unknown_header(
            node,
            source,
            "@Situation",
            "Expected @Situation:\t<description>",
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
        surface_displaced(&children.unexpected, "situation_header", source, errors);
        return super::super::unknown_header(
            node,
            source,
            "@Situation",
            "Expected @Situation:\t<description>",
            "Could not decode @Situation text",
        );
    };

    surface_displaced(&children.unexpected, "situation_header", source, errors);
    Header::Situation {
        text: SituationDescription::new(text),
    }
}
