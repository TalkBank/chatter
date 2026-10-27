//! Parser for `%exp` (explanation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::ExpTier;

use crate::generated_traversal::{AsRawNode, ExpDependentTierNode, SourceBound};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%exp` tier node.
///
/// **Grammar Rule:**
/// ```text
/// exp_dependent_tier: seq('%', 'exp', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_exp_dependent_tier` yields the
/// body as `child_2.slot`. The concrete owner surfaces displaced recovery and
/// consumes selected range admission before the shared whole-carrier reader.
pub fn parse_exp_tier<'tree>(
    typed: SourceBound<'tree, '_, ExpDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<talkbank_model::ParseOutcome<ExpTier>, crate::CstFailure> {
    parse_exp_tier_with_separator(typed, errors)
        .map(|outcome| outcome.map(crate::parser::tier_parsers::ParsedDependentTier::into_tier))
}

/// Retain the tier and its separator from one selected extraction.
pub(crate) fn parse_exp_tier_with_separator<'tree>(
    typed: SourceBound<'tree, '_, ExpDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<
    talkbank_model::ParseOutcome<crate::parser::tier_parsers::ParsedDependentTier<ExpTier>>,
    crate::CstFailure,
> {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = crate::parser::typed_cst::extract_envelope(typed)?;
    let separator =
        crate::parser::tier_parsers::dependent_tier_separator(children.children().child_1.slot())?;
    crate::parser::tree_parsing::parser_helpers::surface_displaced(
        &children.children().unexpected,
        node.kind(),
        typed.source(),
        errors,
    );
    let children = children.admit_ranges()?;
    let content = parse_optional_text_tier_content(&children, errors)?;
    Ok(content.map(|content| {
        crate::parser::tier_parsers::ParsedDependentTier::new(
            ExpTier::new(span, content),
            separator,
        )
    }))
}
