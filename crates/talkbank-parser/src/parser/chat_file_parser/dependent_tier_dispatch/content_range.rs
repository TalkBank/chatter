//! Exact source-body geometry from selected generated separator/newline slots.
//!
//! Recovery can preserve geometry without preserving valid content. No payload
//! text, label length or presumed newline width participates in this reading.

use crate::generated_traversal::{
    AsRawNode, Never, NewlineNode, ReadableSlot, SourceBindingError, SourceBound, TierSepNode,
    UtteranceChild1Choice, UtteranceChild1ChoiceBoundView, WorDependentTierNode,
};
use talkbank_model::Span;
use talkbank_model::model::dependent_tier::DependentTierContentSpan;

fn newline_start<'tree, 'source, E, A>(
    slot: &ReadableSlot<
        SourceBound<'tree, 'source, NewlineNode<'tree>>,
        SourceBound<'tree, 'source, NewlineNode<'tree>>,
        E,
        Never,
        A,
    >,
) -> Option<usize> {
    match slot {
        // Missing retains its native zero-width endpoint. It is not a present
        // newline or a syntax-validity claim, but no byte may be subtracted.
        ReadableSlot::Present(node) | ReadableSlot::Missing(node) => {
            Some(node.raw_node().start_byte())
        }
        ReadableSlot::Error(_) | ReadableSlot::Absent(_) => None,
        ReadableSlot::Unexpected(never) => match *never {},
    }
}

fn range<'tree>(
    tier: Span,
    separator: SourceBound<'tree, '_, TierSepNode<'tree>>,
    end: Option<usize>,
) -> Result<Option<DependentTierContentSpan>, crate::CstFailure> {
    let Some(end) = end else { return Ok(None) };
    DependentTierContentSpan::try_new(tier, separator.raw_node().end_byte(), end)
        .map(Some)
        .map_err(|_| SourceBindingError::InvalidRange.into())
}

/// The word tier nests its newline in the selected body. Deferred lowering
/// calls this same reader so retaining a word tier cannot discard its geometry.
pub(super) fn wor<'tree>(
    node: SourceBound<'tree, '_, WorDependentTierNode<'tree>>,
) -> Result<Option<DependentTierContentSpan>, crate::CstFailure> {
    let children = crate::parser::typed_cst::extract_envelope(node)?.admit_ranges()?;
    let crate::generated_traversal::SlotView::Present(separator) =
        children.children().child_1.slot.view();
    let crate::generated_traversal::SlotView::Present(body) =
        children.children().child_2.slot.view();
    let body = body.extract().admit_ranges()?;
    range(
        crate::parser::node_span::span_of(node.raw_node()),
        *separator,
        newline_start(&body.children().child_3.slot),
    )
}

/// Each arm selects its own generated shape; no native kind-string walk or
/// caller-supplied node/source pairing is introduced.
pub(super) fn selected<'tree>(
    choice: SourceBound<'tree, '_, UtteranceChild1Choice<'tree>>,
) -> Result<Option<DependentTierContentSpan>, crate::CstFailure> {
    use UtteranceChild1ChoiceBoundView as C;
    macro_rules! read {
        ($node:expr) => {{
            let node = $node;
            let children = crate::parser::typed_cst::extract_envelope(node)?.admit_ranges()?;
            let crate::generated_traversal::SlotView::Present(separator) =
                children.children().child_1.slot.view();
            range(
                crate::parser::node_span::span_of(node.raw_node()),
                *separator,
                newline_start(&children.children().child_3.slot),
            )
        }};
    }
    match choice.view() {
        C::MorDependentTier(n) => read!(n),
        C::GraDependentTier(n) => read!(n),
        C::PhoDependentTier(n) => read!(n),
        C::ModDependentTier(n) => read!(n),
        C::SinDependentTier(n) => read!(n),
        // Its owned lowering records the nested boundary, including when it
        // travels through deferred retention rather than this dispatch.
        C::WorDependentTier(_) => Ok(None),
        C::ComDependentTier(n) => read!(n),
        C::ExpDependentTier(n) => read!(n),
        C::AddDependentTier(n) => read!(n),
        C::SpaDependentTier(n) => read!(n),
        C::SitDependentTier(n) => read!(n),
        C::IntDependentTier(n) => read!(n),
        C::GpxDependentTier(n) => read!(n),
        C::CodDependentTier(n) => read!(n),
        C::ActDependentTier(n) => read!(n),
        C::OrtDependentTier(n) => read!(n),
        C::EngDependentTier(n) => read!(n),
        C::GlsDependentTier(n) => read!(n),
        C::AltDependentTier(n) => read!(n),
        C::CohDependentTier(n) => read!(n),
        C::DefDependentTier(n) => read!(n),
        C::ErrDependentTier(n) => read!(n),
        C::FacDependentTier(n) => read!(n),
        C::FloDependentTier(n) => read!(n),
        C::ParDependentTier(n) => read!(n),
        C::TimDependentTier(n) => read!(n),
        C::ModsylDependentTier(n) => read!(n),
        C::PhosylDependentTier(n) => read!(n),
        C::PhoalnDependentTier(n) => read!(n),
        C::XphointDependentTier(n) => read!(n),
        C::XDependentTier(n) => read!(n),
        C::UnsupportedDependentTier(node) => {
            // This rule is outside flat-envelope eligibility. Its recovery
            // geometry retains the broad selected separator and newline.
            let children = node.extract().admit_ranges()?;
            let ReadableSlot::Present(separator) = &children.children().child_1.slot else {
                return Ok(None);
            };
            range(
                crate::parser::node_span::span_of(node.raw_node()),
                *separator,
                newline_start(&children.children().child_3.slot),
            )
        }
    }
}
