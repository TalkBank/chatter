//! Parser for `%spa` speech-act tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speech_Act>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::SpaTier;

use crate::generated_traversal::{AsRawNode, SourceBound, SpaDependentTierNode};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%spa` tier node.
///
/// **Grammar Rule:**
/// ```text
/// spa_dependent_tier: seq('%', 'spa', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_spa_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// `parse_optional_text_tier_content`, which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_spa_tier<'tree>(
    typed: SourceBound<'tree, '_, SpaDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> SpaTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = typed.extract();
    let content = parse_optional_text_tier_content(
        typed,
        children.field_child_2().slot(),
        &children.children().unexpected,
        errors,
    );
    SpaTier::new(content).with_span(span)
}
