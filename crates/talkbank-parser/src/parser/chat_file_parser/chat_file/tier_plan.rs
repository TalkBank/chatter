//! Dependent-tier plans as phase types.
//!
//! A plan is either undecided ([`PlanEntry::AfterHeaders`], a callback over
//! the document's headers) or decided. Lowering consumes the entry and hands
//! back the decided plan, so routing a tier, or reading a selection, before
//! the decision does not type-check. Each decided plan implements
//! [`TierRouting`], which says what happens to every concrete dependent tier
//! at its generated position.
//!
//! ```mermaid
//! stateDiagram-v2
//!     [*] --> AfterHeaders: header-dependent admission
//!     [*] --> Decided: fixed admission, plain parsing
//!     AfterHeaders --> Decided: last header lowered, callback runs once
//!     Decided --> TierRemoval: removes selected domains, defers nothing
//!     Decided --> WordTimingDecision: defers word tiers, removes nothing
//! ```
//!
//! [`TierRemoval`] defers nothing, and its deferred type is uninhabited, so
//! its utterances carry no word candidates by type. [`WordTimingDecision`]
//! makes no removal receipt during lowering: receipts come later, from each
//! deferred tier's own evidence. Receipts and candidates never coexist.
use super::replacement::{RemovedTier, ReplacementTiers, SourceTier, WordTimingPlan};
use super::word_timing_plan::{DeferredWordTier, LoweredUtterance, PlacedTier, WordTimingLine};
use crate::generated_traversal::{
    AsRawNode, SourceBound, UtteranceChild1Choice as Choice,
    UtteranceChild1ChoiceBoundView as ChoiceView, WorDependentTierNode,
};
use crate::parser::CstNodeId;
use talkbank_model::InternalFailure;
use talkbank_model::model::{Header, Line, ParseHealthTier};

/// The decision a header-dependent plan defers: called once, with every
/// lowered header (misplaced ones included), it returns the decided plan `P`.
pub(crate) type HeaderSelection<'select, P> = Box<dyn FnOnce(&[&Header]) -> P + 'select>;

/// A dependent-tier plan before the document's headers decided it.
///
/// The decision is a transition, not a field: lowering consumes the entry and
/// hands back the decided plan `P`, so no code can route a tier, or ask for a
/// selection, while the plan is still undecided. An entry that needs headers
/// makes lowering hold every utterance until the last header is lowered.
pub(crate) enum PlanEntry<'select, P> {
    /// Fixed before lowering; utterances lower as they are met.
    Decided(P),
    /// Chosen from every lowered header, misplaced ones included.
    AfterHeaders(HeaderSelection<'select, P>),
}

/// What happens to one concrete dependent tier at its generated position.
pub(crate) enum TierRoute<D> {
    /// Lower the tier into the utterance as usual.
    Lower,
    /// The plan removed the tier. It never enters the model or its parse
    /// health, and the plan holds its receipt.
    Removed,
    /// The plan lowered the tier apart from the document, to judge it later.
    Deferred(D),
}

/// The routing a DECIDED plan performs while utterances lower. Only decided
/// plans implement it, so routing an undecided plan does not type-check.
pub(crate) trait TierRouting {
    /// A tier this plan lowers apart from the document.
    /// [`std::convert::Infallible`] for a plan that never defers, which makes
    /// its utterances' deferred lists empty by type.
    type Deferred;

    /// Route one concrete tier at its generated position.
    fn route<'tree>(
        &mut self,
        choice: SourceBound<'tree, '_, Choice<'tree>>,
    ) -> TierRoute<Self::Deferred>;

    /// Refuse a tool failure in any deferred tier's own lowering, before a
    /// retained parse error refuses the document: a producer fault is retained
    /// even when a different retained region also failed.
    fn admit_deferred_diagnostics(&mut self) -> Result<(), InternalFailure>;

    /// Push a lowered utterance into the document, keeping its deferred tiers
    /// bound to its actual line.
    fn push_utterance(
        &mut self,
        lines: &mut Vec<Line>,
        utterance: LoweredUtterance<Self::Deferred>,
    );

    /// CST nodes the plan took out of normal lowering. The recovery backstop
    /// excludes them by exact identity, never by diagnostic code or span.
    fn withheld_nodes(&self) -> impl Iterator<Item = CstNodeId> + '_;
}

/// The removable alignment domain of a concrete tier, read once from the
/// generated choice. Every other tier kind is retained by every plan.
enum RemovableTier<'tree, 'source> {
    Morphology,
    GrammaticalRelations,
    WordTiming(SourceBound<'tree, 'source, WorDependentTierNode<'tree>>),
}

impl<'tree, 'source> RemovableTier<'tree, 'source> {
    /// Exhaustive over the generated choice: a new tier kind is a compile
    /// error here rather than silently retained.
    fn of(view: ChoiceView<'tree, 'source>) -> Option<Self> {
        match view {
            ChoiceView::MorDependentTier(_) => Some(Self::Morphology),
            ChoiceView::GraDependentTier(_) => Some(Self::GrammaticalRelations),
            ChoiceView::WorDependentTier(node) => Some(Self::WordTiming(node)),
            ChoiceView::ActDependentTier(_)
            | ChoiceView::AddDependentTier(_)
            | ChoiceView::AltDependentTier(_)
            | ChoiceView::CodDependentTier(_)
            | ChoiceView::CohDependentTier(_)
            | ChoiceView::ComDependentTier(_)
            | ChoiceView::DefDependentTier(_)
            | ChoiceView::EngDependentTier(_)
            | ChoiceView::ErrDependentTier(_)
            | ChoiceView::ExpDependentTier(_)
            | ChoiceView::FacDependentTier(_)
            | ChoiceView::FloDependentTier(_)
            | ChoiceView::GlsDependentTier(_)
            | ChoiceView::GpxDependentTier(_)
            | ChoiceView::IntDependentTier(_)
            | ChoiceView::ModDependentTier(_)
            | ChoiceView::ModsylDependentTier(_)
            | ChoiceView::OrtDependentTier(_)
            | ChoiceView::ParDependentTier(_)
            | ChoiceView::PhoDependentTier(_)
            | ChoiceView::PhoalnDependentTier(_)
            | ChoiceView::PhosylDependentTier(_)
            | ChoiceView::SinDependentTier(_)
            | ChoiceView::SitDependentTier(_)
            | ChoiceView::SpaDependentTier(_)
            | ChoiceView::TimDependentTier(_)
            | ChoiceView::UnsupportedDependentTier(_)
            | ChoiceView::XDependentTier(_)
            | ChoiceView::XphointDependentTier(_) => None,
        }
    }

    /// The tier's domain when `selection` removes it.
    fn removed_by(&self, selection: ReplacementTiers) -> Option<ParseHealthTier> {
        match (selection, self) {
            (ReplacementTiers::Morphosyntax, Self::Morphology) => Some(ParseHealthTier::Mor),
            (ReplacementTiers::Morphosyntax, Self::GrammaticalRelations) => {
                Some(ParseHealthTier::Gra)
            }
            (ReplacementTiers::WordTiming, Self::WordTiming(_)) => Some(ParseHealthTier::Wor),
            (ReplacementTiers::Morphosyntax, Self::WordTiming(_))
            | (ReplacementTiers::WordTiming, Self::Morphology | Self::GrammaticalRelations) => None,
        }
    }
}

/// The plain-parsing plan: every tier lowers, nothing is deferred or
/// withheld, and there is no receipt to return.
pub(crate) struct RetainAll;

impl TierRouting for RetainAll {
    type Deferred = std::convert::Infallible;

    fn route<'tree>(
        &mut self,
        _choice: SourceBound<'tree, '_, Choice<'tree>>,
    ) -> TierRoute<Self::Deferred> {
        TierRoute::Lower
    }

    fn admit_deferred_diagnostics(&mut self) -> Result<(), InternalFailure> {
        Ok(())
    }

    fn push_utterance(
        &mut self,
        lines: &mut Vec<Line>,
        utterance: LoweredUtterance<Self::Deferred>,
    ) {
        push_undeferred(lines, utterance);
    }

    fn withheld_nodes(&self) -> impl Iterator<Item = CstNodeId> + '_ {
        std::iter::empty()
    }
}

/// Push an utterance of a plan that never defers: its deferred list is
/// empty by type.
fn push_undeferred(lines: &mut Vec<Line>, utterance: LoweredUtterance<std::convert::Infallible>) {
    let LoweredUtterance { utterance, words } = utterance;
    // Each element would hold an `Infallible`, so the list is empty and the
    // branch cannot be taken.
    if let Some(never) = words.into_iter().map(PlacedTier::into_tier).next() {
        match never {}
    }
    lines.push(Line::utterance(utterance));
}

/// A fixed plan, and afterwards its receipt: nothing removed, or the
/// caller's selection with every concrete tier it removed. A receipt with
/// removals and no selection is unrepresentable.
#[derive(Debug)]
pub(crate) enum TierRemoval {
    /// No selection: every dependent tier is retained.
    Nothing,
    /// Every tier of the selected domains is removed; `removed` may be empty
    /// when no tier matched, which still never certifies the original bytes.
    Selected {
        /// The domains removed: the caller's fixed selection, or
        /// `WordTiming` for the adaptive plan's evidence-named removals.
        selection: ReplacementTiers,
        /// One receipt per concrete tier removed, in source order.
        removed: Vec<RemovedTier>,
    },
}

impl TierRemoval {
    pub(super) fn from_selection(selection: Option<ReplacementTiers>) -> Self {
        match selection {
            None => Self::Nothing,
            Some(selection) => Self::Selected {
                selection,
                removed: Vec::new(),
            },
        }
    }

    /// The receipt of the adaptive word-timing plan: nothing when it removed
    /// no tier, otherwise the word tiers its evidence named.
    pub(super) fn adaptive(removed: Vec<RemovedTier>) -> Self {
        match removed.as_slice() {
            [] => Self::Nothing,
            [_, ..] => Self::Selected {
                selection: ReplacementTiers::WordTiming,
                removed,
            },
        }
    }

    pub(super) fn selection(&self) -> Option<ReplacementTiers> {
        match self {
            Self::Nothing => None,
            Self::Selected { selection, .. } => Some(*selection),
        }
    }

    pub(super) fn removed(&self) -> &[RemovedTier] {
        match self {
            Self::Nothing => &[],
            Self::Selected { removed, .. } => removed,
        }
    }
}

impl TierRouting for TierRemoval {
    type Deferred = std::convert::Infallible;

    /// Selection and physical removal happen at the same generated position.
    fn route<'tree>(
        &mut self,
        choice: SourceBound<'tree, '_, Choice<'tree>>,
    ) -> TierRoute<Self::Deferred> {
        match self {
            Self::Nothing => TierRoute::Lower,
            Self::Selected { selection, removed } => {
                match RemovableTier::of(choice.view()).and_then(|tier| tier.removed_by(*selection))
                {
                    Some(tier) => {
                        removed.push(RemovedTier::selected(SourceTier::of(
                            tier,
                            choice.raw_node(),
                        )));
                        TierRoute::Removed
                    }
                    None => TierRoute::Lower,
                }
            }
        }
    }

    /// Nothing is deferred, so nothing can carry a producer fault.
    fn admit_deferred_diagnostics(&mut self) -> Result<(), InternalFailure> {
        Ok(())
    }

    fn push_utterance(
        &mut self,
        lines: &mut Vec<Line>,
        utterance: LoweredUtterance<Self::Deferred>,
    ) {
        push_undeferred(lines, utterance);
    }

    fn withheld_nodes(&self) -> impl Iterator<Item = CstNodeId> + '_ {
        self.removed().iter().map(RemovedTier::node_id)
    }
}

/// The adaptive word-timing plan once the document's headers decided it.
/// Only `PreferRetained` defers word tiers, and it removes none during
/// lowering: removal receipts are made later, from the deferred tiers' own
/// evidence, so deferred candidates and removals cannot coexist here.
pub(super) enum WordTimingDecision {
    /// Every tier is retained and must pass complete admission.
    Preserve,
    /// Every concrete word tier is lowered apart from the document, bound to
    /// the line of its utterance.
    PreferRetained(Vec<WordTimingLine>),
}

impl From<WordTimingPlan> for WordTimingDecision {
    fn from(plan: WordTimingPlan) -> Self {
        match plan {
            WordTimingPlan::Preserve => Self::Preserve,
            WordTimingPlan::PreferRetained => Self::PreferRetained(Vec::new()),
        }
    }
}

impl TierRouting for WordTimingDecision {
    type Deferred = DeferredWordTier;

    /// Under `PreferRetained` every word tier is deferred; everything else
    /// lowers as usual.
    fn route<'tree>(
        &mut self,
        choice: SourceBound<'tree, '_, Choice<'tree>>,
    ) -> TierRoute<Self::Deferred> {
        match self {
            Self::Preserve => TierRoute::Lower,
            Self::PreferRetained(_) => match RemovableTier::of(choice.view()) {
                Some(RemovableTier::WordTiming(node)) => {
                    TierRoute::Deferred(DeferredWordTier::lower(node))
                }
                Some(RemovableTier::Morphology | RemovableTier::GrammaticalRelations) | None => {
                    TierRoute::Lower
                }
            },
        }
    }

    fn admit_deferred_diagnostics(&mut self) -> Result<(), InternalFailure> {
        match self {
            Self::Preserve => Ok(()),
            Self::PreferRetained(candidates) => candidates
                .iter_mut()
                .flat_map(|line| &mut line.words)
                .try_for_each(|word| word.tier_mut().admit_diagnostics()),
        }
    }

    fn push_utterance(
        &mut self,
        lines: &mut Vec<Line>,
        utterance: LoweredUtterance<Self::Deferred>,
    ) {
        let line = utterance.push(lines);
        match self {
            Self::PreferRetained(candidates) if !line.words.is_empty() => candidates.push(line),
            Self::PreferRetained(_) => {}
            // `route` defers nothing under `Preserve`, so its lines carry no
            // candidates. The type does not say so: both variants share one
            // deferred type because the variant is chosen during lowering.
            Self::Preserve => {}
        }
    }

    fn withheld_nodes(&self) -> impl Iterator<Item = CstNodeId> + '_ {
        let candidates: &[WordTimingLine] = match self {
            Self::Preserve => &[],
            Self::PreferRetained(candidates) => candidates,
        };
        candidates
            .iter()
            .flat_map(|line| line.words.iter().map(|word| word.tier().node_id()))
    }
}
