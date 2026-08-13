//! Overlap validation functions.
//!
//! Validates CA overlap markers (⌈⌉⌊⌋) within individual utterances:
//! - **E373**: Invalid overlap index (must be 2-9 if present)
//!
//! NOT E348. This module claimed to check it and did not: the pairing function
//! walked the tree and both its branches were empty, so `MissingOverlapEnd` is
//! constructed nowhere in the workspace. Within one utterance an unpaired
//! marker is legitimate; across utterances it is E347's business.
//!
//! Uses [`extract_overlap_info`] from `alignment::helpers::overlap` for the
//! content traversal, same traversal used by the alignment pipeline,
//! eliminating duplicated walk logic.
//!
//! Cross-utterance checks (E347 unbalanced across speakers, E704 self-overlap)
//! are in `validation/cross_utterance/`.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>

use crate::ErrorSink;
use crate::alignment::helpers::overlap::{OverlapRegionKind, extract_overlap_info};
use crate::model::Utterance;
use crate::validation::{Validate, ValidationContext};

/// Validate overlap markers within one utterance.
///
/// Checks index validity (E373). Pairing is a cross-utterance question; see
/// the note in the body and `validation/cross_utterance/`.
pub(crate) fn check_overlap_markers(
    utterance: &Utterance,
    context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    check_overlap_index_values(utterance, context, errors);
    // No pairing check. `check_overlap_pairing` used to sit here: it walked the
    // whole utterance, allocated the marker list, paired it, and then did
    // nothing, because BOTH of its arms had been emptied. Its own comments say
    // why, and they are right: an opening marker with no close, or a close with
    // no opening, is a legitimate cross-utterance overlap span, and pairing
    // ACROSS utterances is E347's job in `validation/cross_utterance/`. There is
    // no within-utterance pairing rule left to enforce.
}

/// Validate overlap-point indices throughout one utterance tree (E373).
///
/// Collects all overlap points and validates that indices are in range 2-9.
/// Uses the shared traversal from `alignment::helpers::overlap`.
pub(crate) fn check_overlap_index_values(
    utterance: &Utterance,
    context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    let index_context = context
        .clone()
        .with_field_span(utterance.main.span)
        .with_field_label("overlap_index");

    // Collect all overlap points via the shared traversal.
    // The regions give us the index values; we validate each one.
    let info = extract_overlap_info(utterance.main.content.content.as_slice());
    for region in &info.regions {
        if let Some(index) = region.index {
            // Validate the index value (must be 2-9).
            // Create a temporary OverlapPoint for the Validate trait.
            let point = crate::model::OverlapPoint::new(
                match region.kind {
                    OverlapRegionKind::Top => crate::model::OverlapPointKind::TopOverlapBegin,
                    OverlapRegionKind::Bottom => crate::model::OverlapPointKind::BottomOverlapBegin,
                },
                Some(index),
            );
            point.validate(&index_context, errors);
        }
    }
}
