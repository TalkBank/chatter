//! Main-tier hover resolver with cross-tier alignment lookups.
//!
//! When hovering over a word on the main tier (`*SPK:` line), this resolver
//! looks up the aligned `%mor`, `%pho`, and `%sin` items and assembles a
//! combined [`AlignmentHoverInfo`](crate::alignment::types::AlignmentHoverInfo)
//! showing the word alongside all its dependent-tier annotations.

// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]

use super::helpers::{find_target_index_for_source, format_gra_alignment_text, position_to_offset};
use crate::alignment::formatters::{
    format_content_item, format_mor_item, format_pho_item, format_sin_item,
};
use crate::alignment::types::AlignmentHoverInfo;
use crate::editor_target::{find_content_index_at_offset, mor_alignment_index};
use talkbank_model::model::Utterance;
use tower_lsp::lsp_types::Position;

/// Build hover info for a main-tier element under the cursor.
pub fn find_main_tier_hover_info(
    utterance: &Utterance,
    _tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
) -> Option<AlignmentHoverInfo> {
    let offset = position_to_offset(document, position);
    let content_idx = find_content_index_at_offset(&utterance.main.content.content, offset)?;

    // One question, not two: an item that contributes no `%mor` position has
    // no index, so there is nothing to compute before checking.
    let alignment_idx = mor_alignment_index(&utterance.main.content.content, content_idx)?;

    // Get content text for display
    let element_content = format_content_item(utterance.main.content.content.get(content_idx)?);

    // Build hover info with alignments
    let mut info = AlignmentHoverInfo::new("Main Tier Word", element_content);

    // Look up %mor/%gra alignment via utterance-level AlignmentSet.
    if let Some(alignments) = &utterance.alignments {
        let mor_idx = alignments.mor.as_ref().and_then(|mor_alignment| {
            find_target_index_for_source(&mor_alignment.pairs, alignment_idx)
        });

        if let Some(mor_idx) = mor_idx
            && let Some(mor_tier) = utterance.mor_tier()
            && let Some(mor_item) = mor_tier.items().get(mor_idx)
        {
            info.aligned_to_mor = Some(format_mor_item(mor_item));
        }

        if let Some(mor_idx) = mor_idx
            && let Some(mor_tier) = utterance.mor_tier()
            && let Some(gra_alignment) = &alignments.gra
            && let Some(gra_idx) = find_target_index_for_source(&gra_alignment.pairs, mor_idx)
            && let Some(gra_tier) = utterance.gra_tier()
            && let Some(gra_relation) = gra_tier.relations().get(gra_idx)
        {
            info.aligned_to_gra = Some(format_gra_alignment_text(mor_tier, gra_relation));
        }

        // Look up %pho alignment
        if let Some(pho_alignment) = &alignments.pho
            && let Some(pho_idx) = find_target_index_for_source(&pho_alignment.pairs, alignment_idx)
            && let Some(pho_tier) = &utterance.pho()
            && let Some(pho_token) = pho_tier.items.get(pho_idx)
        {
            info.aligned_to_pho = Some(format_pho_item(pho_token));
        }

        // Look up %mod alignment
        if let Some(mod_alignment) = &alignments.mod_
            && let Some(mod_idx) = find_target_index_for_source(&mod_alignment.pairs, alignment_idx)
            && let Some(mod_tier) = &utterance.mod_tier()
            && let Some(mod_token) = mod_tier.items.get(mod_idx)
        {
            info.aligned_to_mod = Some(format_pho_item(mod_token));
        }

        // Look up %sin alignment
        if let Some(sin_alignment) = &alignments.sin
            && let Some(sin_idx) = find_target_index_for_source(&sin_alignment.pairs, alignment_idx)
            && let Some(sin_tier) = &utterance.sin()
            && let Some(sin_token) = sin_tier.items.get(sin_idx)
        {
            info.aligned_to_sin = Some(format_sin_item(sin_token));
        }
    }

    Some(info)
}
