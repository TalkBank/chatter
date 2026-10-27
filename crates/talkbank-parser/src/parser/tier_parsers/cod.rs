//! Parser for `%cod` coding tiers.
//!
//! `%cod` carries analyst-defined coding content and reuses the same
//! bullet-capable free-text structure as `%act`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Coding_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::generated_traversal::{AsRawNode, CodDependentTierNode, SourceBound};
use crate::parser::node_span::span_of;
use crate::parser::tier_parsers::text::helpers::parse_optional_text_tier_content;
use talkbank_model::ErrorSink;
use talkbank_model::model::CodTier;

/// Converts one `%cod` tier node into a `CodTier`.
///
/// **Grammar Rule:**
/// ```text
/// cod_dependent_tier: seq('%', 'cod', colon, tab, optional(text_with_bullets), newline)
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
pub fn parse_cod_tier<'tree>(
    typed: SourceBound<'tree, '_, CodDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<talkbank_model::ParseOutcome<CodTier>, crate::CstFailure> {
    parse_cod_tier_with_separator(typed, errors)
        .map(|outcome| outcome.map(crate::parser::tier_parsers::ParsedDependentTier::into_tier))
}

/// Retain the tier and its separator from one selected extraction.
pub(crate) fn parse_cod_tier_with_separator<'tree>(
    typed: SourceBound<'tree, '_, CodDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<
    talkbank_model::ParseOutcome<crate::parser::tier_parsers::ParsedDependentTier<CodTier>>,
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
            CodTier::new(span, content),
            separator,
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::WriteChat;

    /// Tests cod tier construction.
    #[test]
    fn test_cod_tier_construction() {
        let tier = CodTier::from_text(talkbank_model::Location::Unlocated, "general coding");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\tgeneral coding");
    }

    /// Tests cod tier single index.
    #[test]
    fn test_cod_tier_single_index() {
        let tier = CodTier::from_text(talkbank_model::Location::Unlocated, "<1> atul");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1> atul");
    }

    /// Tests cod tier compound index.
    #[test]
    fn test_cod_tier_compound_index() {
        let tier = CodTier::from_text(talkbank_model::Location::Unlocated, "<1+2> eje");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1+2> eje");
    }

    /// Tests cod tier multiple indices.
    #[test]
    fn test_cod_tier_multiple_indices() {
        let tier = CodTier::from_text(talkbank_model::Location::Unlocated, "<1 , 3> atul");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1 , 3> atul");
    }

    /// Tests cod tier complex.
    #[test]
    fn test_cod_tier_complex() {
        let tier = CodTier::from_text(
            talkbank_model::Location::Unlocated,
            "<2 , 7> ledet <8> Itamar",
        );
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<2 , 7> ledet <8> Itamar");
    }

    /// Tests cod tier empty.
    #[test]
    fn test_cod_tier_empty() {
        let tier = CodTier::from_text(talkbank_model::Location::Unlocated, "");
        assert!(tier.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t");
    }
}
