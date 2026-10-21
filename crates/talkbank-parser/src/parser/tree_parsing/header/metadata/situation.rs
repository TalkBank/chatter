//! Parsing for `@Situation` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>

use crate::generated_traversal::{SituationHeaderNode, SourceBound, SourceSlotView};

use crate::error::ErrorSink;
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

    // Grammar: seq(situation_prefix, header_sep, free_text, newline). The free
    // text is directly at child_2. Per-rule admission proves its presence;
    // readable-range admission remains a distinct obligation.
    let children = crate::parser::typed_cst::extract_envelope(typed)?;
    let SourceSlotView::Present(free_text) = children.field_child_2().slot().view();

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
