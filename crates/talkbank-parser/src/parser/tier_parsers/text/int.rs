//! Parser for `%int` (intonation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Intonation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::IntTier;

use crate::generated_traversal::{AsRawNode, IntDependentTierNode, SourceBound};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%int` tier node.
///
/// **Grammar Rule:**
/// ```text
/// int_dependent_tier: seq('%', 'int', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_int_dependent_tier` yields the
/// body as `child_2.slot`. The concrete owner surfaces displaced recovery and
/// consumes selected range admission before the shared whole-carrier reader.
pub fn parse_int_tier<'tree>(
    typed: SourceBound<'tree, '_, IntDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<IntTier, crate::CstFailure> {
    parse_int_tier_with_separator(typed, errors)
        .map(crate::parser::tier_parsers::ParsedDependentTier::into_tier)
}

/// Retain the tier and its separator from one selected extraction.
pub(crate) fn parse_int_tier_with_separator<'tree>(
    typed: SourceBound<'tree, '_, IntDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<crate::parser::tier_parsers::ParsedDependentTier<IntTier>, crate::CstFailure> {
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
        IntTier::new(content).with_span(span),
        separator,
    ))
}
