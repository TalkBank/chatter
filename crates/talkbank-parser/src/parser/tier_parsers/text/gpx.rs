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
/// body as `child_2.slot`, matched exhaustively by the shared
/// `parse_optional_text_tier_content`, which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_gpx_tier<'tree>(
    typed: SourceBound<'tree, '_, GpxDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> GpxTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = typed.extract();
    let content = parse_optional_text_tier_content(
        typed,
        children.field_child_2().slot(),
        &children.children().unexpected,
        errors,
    );
    GpxTier::new(content).with_span(span)
}
