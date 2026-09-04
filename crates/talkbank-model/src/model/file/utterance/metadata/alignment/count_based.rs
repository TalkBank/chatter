use super::diagnostics::build_count_mismatch_error;
use crate::Utterance;
use crate::alignment::indices::{MainWordIndex, PhoItemIndex};
use crate::{ErrorCode, Span};

/// Build a `MorTier` from items, inheriting span and terminator from
/// the utterance's existing `%mor:` tier. Returns `None` when no
/// existing `%mor:` tier is present; there is no terminator or span
/// to inherit, and the constructor cannot synthesize them.
pub(super) fn build_mor_tier_from_items(
    utterance: &Utterance,
    items: &[crate::model::Mor],
) -> Option<crate::model::MorTier> {
    let existing = utterance.mor_tier()?;
    let mut tier = crate::model::MorTier::new_mor(items.to_vec(), existing.terminator.clone());
    tier.span = existing.span;
    Some(tier)
}

pub(super) fn build_tier_to_tier_alignment(
    source_count: usize,
    source_span: Span,
    source_label: &str,
    target_count: usize,
    target_span: Span,
    target_label: &str,
    mismatch_code: ErrorCode,
) -> crate::alignment::PhoAlignment {
    let mut alignment = crate::alignment::PhoAlignment::new();

    let min_len = source_count.min(target_count);
    for i in 0..min_len {
        alignment = alignment.with_pair(crate::alignment::AlignmentPair::new(
            Some(MainWordIndex::new(i)),
            Some(PhoItemIndex::new(i)),
        ));
    }

    if source_count != target_count {
        alignment = alignment.with_error(build_count_mismatch_error(
            source_count,
            source_span,
            source_label,
            target_count,
            target_span,
            target_label,
            mismatch_code,
        ));
    }

    alignment
}
