//! Tier-specific dependent-tier parsers.
//!
//! Each submodule accepts CST nodes already identified as a specific tier and
//! converts them into typed model values while streaming parse errors.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Coding_Tier>

use crate::error::Span;
use crate::generated_traversal::{AsRawNode, NodeSlot, SlotView, TierSepNode, extract_tier_sep};
use crate::model::{DependentTier, DependentTierEntry, TierSeparator};

pub mod act;
pub mod cod;
pub mod gra;
pub mod mor;
pub mod pho;
pub mod sin;
pub mod text;
pub mod wor;

/// One tier reading retains its separator until document attachment.
/// Low-level public APIs explicitly request only the tier with `into_tier`.
#[must_use]
pub(crate) struct ParsedDependentTier<T> {
    tier: T,
    separator: TierSeparator,
}

impl<T> ParsedDependentTier<T> {
    pub(in crate::parser::tier_parsers) fn new(tier: T, separator: TierSeparator) -> Self {
        Self { tier, separator }
    }

    pub(in crate::parser::tier_parsers) fn into_tier(self) -> T {
        self.tier
    }

    pub(crate) fn into_entry(self, variant: impl FnOnce(T) -> DependentTier) -> DependentTierEntry {
        DependentTierEntry::with_separator(variant(self.tier), self.separator)
    }
}

/// Read separator provenance from the selected tier's positional child 1.
///
/// The nested tab and captured trailing space must remain adjacent. Recovery
/// or an absent capture is a clean separator; E758 validates real captured
/// space later, never at this parse boundary. Only its Present state is read;
/// the producer retains ownership of the recovery payload.
pub(crate) fn dependent_tier_separator<M>(
    slot: &crate::parser::typed_cst::AnyKindSlot<'_, TierSepNode<'_>, M>,
) -> TierSeparator {
    let NodeSlot::Present(tier_sep) = slot else {
        return TierSeparator::CLEAN;
    };
    let tier_sep_children = extract_tier_sep(*tier_sep);
    let trailing = tier_sep_children.child_2.slot();
    match trailing.as_ref().map(NodeSlot::view) {
        Some(SlotView::Present(sep_node))
            if matches!(tier_sep_children.child_1.slot(), NodeSlot::Present(tab)
                if tab.raw_node().end_byte() == sep_node.raw_node().start_byte()) =>
        {
            let node = sep_node.raw_node();
            TierSeparator::with_trailing_space(Span::new(
                node.start_byte() as u32,
                node.end_byte() as u32,
            ))
        }
        Some(SlotView::Present(_) | SlotView::Missing(_) | SlotView::Error(_)) | None => {
            TierSeparator::CLEAN
        }
    }
}

// Re-export public parsing functions for convenience within parser module
