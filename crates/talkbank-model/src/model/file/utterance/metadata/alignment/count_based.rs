use super::diagnostics::build_count_mismatch_error;
use crate::alignment::indices::{MainWordIndex, PhoItemIndex};
use crate::{ErrorCode, Span};

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
