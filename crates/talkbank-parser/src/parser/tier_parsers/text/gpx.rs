//! Parser for `%gpx` tiers.
//!
//! `%gpx` content is represented as text-with-bullets in the same family as
//! `%com`/`%exp`/`%add`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gems>

use talkbank_model::ErrorSink;
use talkbank_model::model::GpxTier;

use crate::generated_traversal::{AsRawNode, GpxDependentTierNode, SourceBound};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%gpx` tier node.
///
/// **Grammar Rule:**
/// ```text
/// gpx_dependent_tier: seq('%', 'gpx', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_gpx_dependent_tier` yields the
/// body as `child_2.slot`. The concrete owner surfaces displaced recovery and
/// consumes selected range admission before the shared whole-carrier reader.
pub fn parse_gpx_tier<'tree>(
    typed: SourceBound<'tree, '_, GpxDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<GpxTier, crate::CstFailure> {
    parse_gpx_tier_with_separator(typed, errors)
        .map(crate::parser::tier_parsers::ParsedDependentTier::into_tier)
}

/// Retain the tier and its separator from one selected extraction.
pub(crate) fn parse_gpx_tier_with_separator<'tree>(
    typed: SourceBound<'tree, '_, GpxDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<crate::parser::tier_parsers::ParsedDependentTier<GpxTier>, crate::CstFailure> {
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
    Ok(crate::parser::tier_parsers::ParsedDependentTier::new(
        GpxTier::new(span, content),
        separator,
    ))
}
