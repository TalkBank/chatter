//! Parser for `%com` (comment) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::ComTier;

use crate::generated_traversal::{AsRawNode, ComDependentTierNode, SourceBound};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%com` tier node.
///
/// **Grammar Rule:**
/// ```text
/// com_dependent_tier: seq('%', 'com', colon, tab, optional(text_with_bullets_and_pics), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_com_dependent_tier` yields the
/// prefix / tier-sep / body / newline as typed `Positioned` slots, and the body
/// (`child_2.slot`, a `text_with_bullets_and_pics` node) is matched exhaustively by
/// the shared whole-carrier reader after the owner surfaces displaced recovery
/// and consumes selected range admission.
pub fn parse_com_tier<'tree>(
    typed: SourceBound<'tree, '_, ComDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ComTier, crate::CstFailure> {
    parse_com_tier_with_separator(typed, errors)
        .map(crate::parser::tier_parsers::ParsedDependentTier::into_tier)
}

/// Retain the tier and its separator from one selected extraction.
pub(crate) fn parse_com_tier_with_separator<'tree>(
    typed: SourceBound<'tree, '_, ComDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<crate::parser::tier_parsers::ParsedDependentTier<ComTier>, crate::CstFailure> {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let separator =
        crate::parser::tier_parsers::dependent_tier_separator(children.children().child_1.slot());
    crate::parser::tree_parsing::parser_helpers::surface_displaced(
        &children.children().unexpected,
        node.kind(),
        typed.source(),
        errors,
    );
    let children = children.admit_ranges()?;
    let content = parse_optional_text_tier_content(&children, errors)?;
    Ok(crate::parser::tier_parsers::ParsedDependentTier::new(
        ComTier::new(content).with_span(span),
        separator,
    ))
}
