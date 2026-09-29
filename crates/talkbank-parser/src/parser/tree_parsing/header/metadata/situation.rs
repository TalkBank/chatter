//! Parsing for `@Situation` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>

use crate::generated_traversal::{AsRawNode, SituationHeaderNode, SourceBound, SourceSlotView};

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
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
pub fn parse_situation_header<'tree>(
    typed: SourceBound<'tree, '_, SituationHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Header, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();

    // Grammar: seq(situation_prefix, header_sep, free_text, newline). The free
    // text is directly at child_2. Source-bound projection retains its owner;
    // every non-present recovery state keeps the missing-text diagnostic at
    // the header span. Readable-range admission remains a distinct obligation.
    let children = typed.extract()?;
    let SourceSlotView::Present(free_text) = children.field_child_2().slot().view() else {
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
        surface_displaced(
            &children.children().unexpected,
            "situation_header",
            source,
            errors,
        );
        return super::super::unknown_header(
            node,
            source,
            "@Situation",
            "Expected @Situation:\t<description>",
            "Missing situation text in @Situation header",
        );
    };

    // A failed range read is a producer failure, not missing authored text.
    let text = free_text.read()?.text();

    surface_displaced(
        &children.children().unexpected,
        "situation_header",
        source,
        errors,
    );
    Ok(Header::Situation {
        text: SituationDescription::new(text),
    })
}
