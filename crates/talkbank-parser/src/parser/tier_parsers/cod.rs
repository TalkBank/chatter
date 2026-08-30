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

use crate::generated_traversal::{AsRawNode, CodDependentTierNode, extract_cod_dependent_tier};
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
/// Delegates the whole five-state body policy to
/// `parse_optional_text_tier_content`, the owner shared with the seven
/// text-like tiers. This file used to carry a private copy of that policy,
/// differing in ONE state: a malformed body gets no `unexpected_node_error`
/// here, because the removed hand-walk loop had none and simply fell through to
/// the "Missing content" rejection. That difference is now
/// `MalformedBody::ReportMissingOnly`, a value rather than a fork, so the four
/// states nobody disputes cannot drift between the copies.
///
/// The `Option` on the body slot is the grammar's `optional(...)` (E756
/// widening, 2026-08-16): an absent body is the empty tier, not a parse
/// failure, so it lowers to empty content with no diagnostic and the validator
/// reports E756.
pub fn parse_cod_tier(
    typed: CodDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> CodTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = extract_cod_dependent_tier(typed);
    let content = parse_optional_text_tier_content(
        typed,
        children.child_2.slot(),
        &children.unexpected,
        source,
        errors,
    );
    CodTier::new(content).with_span(span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::WriteChat;

    /// Tests cod tier construction.
    #[test]
    fn test_cod_tier_construction() {
        let tier = CodTier::from_text("general coding");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\tgeneral coding");
    }

    /// Tests cod tier single index.
    #[test]
    fn test_cod_tier_single_index() {
        let tier = CodTier::from_text("<1> atul");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1> atul");
    }

    /// Tests cod tier compound index.
    #[test]
    fn test_cod_tier_compound_index() {
        let tier = CodTier::from_text("<1+2> eje");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1+2> eje");
    }

    /// Tests cod tier multiple indices.
    #[test]
    fn test_cod_tier_multiple_indices() {
        let tier = CodTier::from_text("<1 , 3> atul");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1 , 3> atul");
    }

    /// Tests cod tier complex.
    #[test]
    fn test_cod_tier_complex() {
        let tier = CodTier::from_text("<2 , 7> ledet <8> Itamar");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<2 , 7> ledet <8> Itamar");
    }

    /// Tests cod tier empty.
    #[test]
    fn test_cod_tier_empty() {
        let tier = CodTier::from_text("");
        assert!(tier.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t");
    }
}
