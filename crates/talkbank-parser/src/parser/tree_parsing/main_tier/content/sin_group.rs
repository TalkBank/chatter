//! Parsing for main-tier sign/gesture groups (`〔 ... 〕`), over the
//! generated typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::ErrorSink;
use crate::generated_traversal::{AsRawNode, MainSinGroupNode, SlotView, SourceBound};
use crate::model::UtteranceContent;
use talkbank_model::ParseOutcome;

use super::group::{contents_of, parse_group_contents};
use super::recovery::surface_main_tier_sink;

/// Parse a `main_sin_group` node into `UtteranceContent::SinGroup`.
///
/// Grammar: `seq(sin_begin_group, contents, sin_end_group)`. The delimiters
/// are structure, the contents go through the one shared walker, and a
/// group with no items is rejected. Sign groups carry no annotations.
pub(crate) fn parse_sin_group_content<'tree>(
    typed: SourceBound<'tree, '_, MainSinGroupNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let source = typed.source();
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_inner(typed)
            .and_then(|children| children.admit_ranges().map_err(Into::into)),
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let children = associated.children();

    let SlotView::Present(contents) = children.child_1.slot.view();
    let group_items = match contents_of(*contents, errors) {
        Some(contents) => parse_group_contents(&contents, errors),
        None => ParseOutcome::Rejected,
    };
    surface_main_tier_sink(children, source, errors);

    let ParseOutcome::Parsed(group_items) = group_items else {
        return ParseOutcome::Rejected;
    };
    if group_items.is_empty() {
        return ParseOutcome::rejected();
    }
    let bracketed = crate::model::BracketedContent::new(group_items);
    ParseOutcome::parsed(UtteranceContent::SinGroup(crate::model::SinGroup::new(
        crate::parser::node_span::span_of(typed.raw_node()),
        bracketed,
    )))
}
