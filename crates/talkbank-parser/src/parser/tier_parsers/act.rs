//! Parser for `%act` action tiers.
//!
//! `%act` content is modeled as bullet-capable free text and is typically
//! aligned with events around the main tier.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Action_Code>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::generated_traversal::{ActDependentTierNode, AsRawNode, SourceBound};
use crate::parser::node_span::span_of;
use crate::parser::tier_parsers::text::helpers::parse_optional_text_tier_content;
use talkbank_model::ErrorSink;
use talkbank_model::model::ActTier;

/// Converts one `%act` tier node into a `ActTier`.
///
/// **Grammar Rule:**
/// ```text
/// act_dependent_tier: seq('%', 'act', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Delegates the range-admitted optional-body policy to
/// `parse_optional_text_tier_content`, the owner shared with the seven
/// text-like tiers. All nine retain the malformed-body diagnostic before the
/// no-content report, rather than preserving the old hand-loop omission here.
/// The concrete owner surfaces displaced recovery and admits selected ranges;
/// the shared reader receives its whole carrier, not independent tier/body fields.
///
/// The `Option` on the body slot is the grammar's `optional(...)` (E756
/// widening, 2026-08-16): an absent body is the empty tier, not a parse
/// failure, so it lowers to empty content with no diagnostic and the validator
/// reports E756.
pub fn parse_act_tier<'tree>(
    typed: SourceBound<'tree, '_, ActDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<talkbank_model::ParseOutcome<ActTier>, crate::CstFailure> {
    parse_act_tier_with_separator(typed, errors)
        .map(|outcome| outcome.map(crate::parser::tier_parsers::ParsedDependentTier::into_tier))
}

/// Retain the tier and its separator from one selected extraction.
pub(crate) fn parse_act_tier_with_separator<'tree>(
    typed: SourceBound<'tree, '_, ActDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<
    talkbank_model::ParseOutcome<crate::parser::tier_parsers::ParsedDependentTier<ActTier>>,
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
            ActTier::new(span, content),
            separator,
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::WriteChat;

    /// Tests act tier construction.
    #[test]
    fn test_act_tier_construction() {
        let tier = ActTier::from_text(talkbank_model::Location::Unlocated, "picks up toy");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%act:\tpicks up toy");
    }

    /// Tests act tier with timing.
    #[test]
    fn test_act_tier_with_timing() {
        let tier = ActTier::from_text(
            talkbank_model::Location::Unlocated,
            "<1w-2w> holds object out to Amy",
        );
        assert!(!tier.content.is_empty());
        assert_eq!(
            tier.to_chat_string(),
            "%act:\t<1w-2w> holds object out to Amy"
        );
    }

    /// Tests act tier empty.
    #[test]
    fn test_act_tier_empty() {
        let tier = ActTier::from_text(talkbank_model::Location::Unlocated, "");
        assert!(tier.is_empty());
        assert_eq!(tier.to_chat_string(), "%act:\t");
    }

    /// Tests act tier complex.
    #[test]
    fn test_act_tier_complex() {
        let tier = ActTier::from_text(
            talkbank_model::Location::Unlocated,
            "<aft> manipulates chicken in hands",
        );
        assert!(!tier.content.is_empty());
        assert_eq!(
            tier.to_chat_string(),
            "%act:\t<aft> manipulates chicken in hands"
        );
    }
}
