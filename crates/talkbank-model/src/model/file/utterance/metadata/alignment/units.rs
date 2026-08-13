// Every match over the content enums in this file is exhaustive, so the lint
// costs nothing today and makes it stay that way: a new `UtteranceContent` or
// `BracketedItem` variant becomes a COMPILE ERROR here rather than a silent
// `_ =>` that answers wrong. Four such catch-alls have already shipped as
// defects; see `talkbank-parser-tests/src/content_catch_alls.rs`.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::Utterance;
use crate::model::{AlignmentUnit, AlignmentUnits};
use crate::validation::ValidationContext;

impl AlignmentUnits {
    /// Build alignment unit inventories for every alignable tier in an utterance.
    pub fn from_utterance(utterance: &Utterance, _context: &ValidationContext) -> Self {
        let mut units = AlignmentUnits {
            main_mor: build_main_units(
                &utterance.main.content.content,
                crate::alignment::TierDomain::Mor,
            ),
            main_pho: build_main_units(
                &utterance.main.content.content,
                crate::alignment::TierDomain::Pho,
            ),
            main_sin: build_main_units(
                &utterance.main.content.content,
                crate::alignment::TierDomain::Sin,
            ),
            main_wor: build_main_units(
                &utterance.main.content.content,
                crate::alignment::TierDomain::Wor,
            ),
            ..Default::default()
        };

        if let Some(tier) = utterance.mor_tier() {
            let item_count = tier.items.len();
            units.mor = (0..item_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
            let chunk_count = tier.count_chunks();
            units.mor_chunks = (0..chunk_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.gra_tier() {
            let item_count = tier.relations.0.len();
            units.gra = (0..item_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.pho_tier() {
            let pho_count = tier.items.len();
            units.pho = (0..pho_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.mod_tier() {
            let mod_count = tier.items.len();
            units.mod_ = (0..mod_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.wor_tier() {
            let wor_count = tier
                .items
                .iter()
                .filter(|item| matches!(item, crate::model::dependent_tier::WorItem::Word(_)))
                .count();
            units.wor = (0..wor_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.sin_tier() {
            let sin_count = tier.items.len();
            units.sin = (0..sin_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.modsyl_tier() {
            units.modsyl = (0..tier.word_count())
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.phosyl_tier() {
            units.phosyl = (0..tier.word_count())
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.phoaln_tier() {
            units.phoaln = (0..tier.word_count())
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        units
    }
}

/// One `AlignmentUnit` per alignable position, indexed in traversal order.
///
/// # This file used to count for itself
///
/// Lines 107-329 were a second implementation of the alignment counting rules:
/// `count_main_item_units` / `count_bracketed_units` / `retraced_units`,
/// arm-for-arm identical to `alignment::helpers::count`'s
/// `count_alignable_item` / `count_bracketed_item` over all 26 content variants
/// and all 26 bracketed variants. Two owners of one rule.
///
/// The cost is not hypothetical and this file recorded it: the two copies had
/// already disagreed about `AnnotatedRetrace` across 8,766 utterances, and
/// nothing failed to compile. That note is the argument for deleting the copy,
/// not for keeping a comment about it.
///
/// What is built here carries no information the count does not: every unit is
/// `{ index, span: None }` with `index` running `0..total`, and the only reader
/// of any `main_*` field in the workspace asks it for `.len()`. So the units
/// are the count, written longhand.
fn build_main_units(
    content: &[crate::model::UtteranceContent],
    domain: crate::alignment::TierDomain,
) -> Vec<AlignmentUnit> {
    (0..crate::alignment::helpers::count_tier_positions(content, domain))
        .map(|index| AlignmentUnit { index, span: None })
        .collect()
}
