//! Parser for `%add` (addressee) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::AddTier;

use crate::generated_traversal::{AddDependentTierNode, AsRawNode, SourceBound};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%add` tier node.
///
/// **Grammar Rule:**
/// ```text
/// add_dependent_tier: seq('%', 'add', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_add_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// `parse_optional_text_tier_content`, which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_add_tier<'tree>(
    typed: SourceBound<'tree, '_, AddDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> AddTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = typed.extract();
    let content = parse_optional_text_tier_content(
        typed,
        children.field_child_2().slot(),
        &children.children().unexpected,
        errors,
    );
    AddTier::new(content).with_span(span)
}
