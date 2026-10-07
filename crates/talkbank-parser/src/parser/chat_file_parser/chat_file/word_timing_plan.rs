//! Producer-owned word tiers, held before retention or regeneration is chosen.
//!
//! The adaptive word-timing plan lowers every concrete `%wor` tier separately
//! from the retained document. [`WordTimingCandidate`] then moves the entries
//! that lowered cleanly into that document at their source positions. When
//! validation rejects the document, every tier holding an error inside its
//! source span is taken out again and the rest are kept. Each removal is
//! recorded as a [`RemovedTier`] carrying the diagnostics that cost it, so the
//! smallest wrong unit is refused and the evidence is never discarded.
//!
//! ```mermaid
//! stateDiagram-v2
//!     [*] --> Deferred: lower each %wor once
//!     Deferred --> Removed: own lowering failed (OwnLowering)
//!     Deferred --> Inserted: entry moved into the document
//!     Inserted --> Removed: validation error inside this tier (LocatedValidation)
//!     Inserted --> Removed: errors in no word tier, remove all (UnattributedValidation)
//! ```
use std::collections::BTreeMap;
use std::sync::Arc;

use super::super::dependent_tier_dispatch::{LoweredWorTier, parse_wor_entry};
use super::replacement::{RemovalCause, RemovedTier, ReplacementFailure, SourceTier, TierTiming};
use crate::error::{ErrorCollector, ParseError};
use crate::generated_traversal::{SourceBound, WorDependentTierNode};
use crate::parser::CstNodeId;
use talkbank_model::model::dependent_tier::DependentTierEntry;
use talkbank_model::model::{ChatFile, DependentTier, Line, Utterance};
use talkbank_model::validation::ValidationFailure;
use talkbank_model::{InternalFailure, Severity, Span};

/// One concrete word tier and its own lowering evidence, never a global
/// diagnostic-code filter or a tier recovered from a rendered model.
pub(crate) struct DeferredWordTier {
    entry: Option<LoweredWorTier>,
    source: SourceTier,
    diagnostics: Vec<ParseError>,
}

impl DeferredWordTier {
    pub(crate) fn lower<'tree>(node: SourceBound<'tree, '_, WorDependentTierNode<'tree>>) -> Self {
        let errors = ErrorCollector::new();
        let entry = parse_wor_entry(node, &errors);
        Self {
            entry,
            source: SourceTier::from_word(node),
            diagnostics: errors.into_vec(),
        }
    }

    /// Exact CST identity, for the recovery backstop's exclusion.
    pub(super) fn node_id(&self) -> CstNodeId {
        self.source.node_id()
    }

    /// Refuse a tool failure in this tier's own lowering. The diagnostics are
    /// moved through the admission and back, never copied.
    pub(super) fn admit_diagnostics(&mut self) -> Result<(), InternalFailure> {
        self.diagnostics =
            talkbank_model::CompletedDiagnostics::admit(std::mem::take(&mut self.diagnostics))?
                .into_diagnostics();
        Ok(())
    }

    /// The lowered entry, if the tier's own lowering produced a clean one.
    /// A missing entry (which syntax recovery always yields) or an own error
    /// refuses only this tier.
    fn into_lowered(
        self,
        line: usize,
    ) -> Result<(DependentTierEntry, InsertedWordTier), RemovedTier> {
        let own_error = self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        let timing = TierTiming::of(self.entry.as_ref().map(LoweredWorTier::tier));
        match self.entry {
            Some(entry) if !own_error => Ok((
                entry.into_entry(),
                InsertedWordTier {
                    line,
                    source: self.source,
                    timing,
                    diagnostics: self.diagnostics,
                },
            )),
            Some(_) | None => Err(RemovedTier::word(
                self.source,
                RemovalCause::OwnLowering {
                    diagnostics: self.diagnostics,
                },
                timing,
            )),
        }
    }
}

/// A tier a plan lowered apart from its utterance, and where it sat.
pub(crate) struct PlacedTier<D> {
    /// Index among the utterance's dependent tiers in source order, counting
    /// every earlier deferred tier as present.
    position: usize,
    tier: D,
}

impl<D> PlacedTier<D> {
    /// Place `tier` after every tier the utterance already holds and every
    /// tier deferred from it earlier: the only position it can have.
    pub(crate) fn after(utterance: &Utterance, earlier: &[PlacedTier<D>], tier: D) -> Self {
        Self {
            position: utterance.dependent_tiers.len() + earlier.len(),
            tier,
        }
    }

    /// The deferred tier, without its placement.
    pub(crate) fn tier(&self) -> &D {
        &self.tier
    }

    /// The deferred tier, mutably, without its placement.
    pub(crate) fn tier_mut(&mut self) -> &mut D {
        &mut self.tier
    }

    /// The deferred tier, consumed.
    pub(crate) fn into_tier(self) -> D {
        self.tier
    }
}

/// The utterance and its deferred tiers move together until their actual line
/// position is known. The document owner alone can bind the insertion point.
/// `D` is the plan's deferred type, uninhabited for a plan that never defers.
pub(crate) struct LoweredUtterance<D> {
    pub(crate) utterance: Utterance,
    pub(crate) words: Vec<PlacedTier<D>>,
}

/// Exact position assigned when the owning utterance is pushed into the file.
pub(super) struct WordTimingLine {
    line: usize,
    pub(super) words: Vec<PlacedTier<DeferredWordTier>>,
}

impl LoweredUtterance<DeferredWordTier> {
    pub(super) fn push(self, lines: &mut Vec<Line>) -> WordTimingLine {
        let line = lines.len();
        lines.push(Line::utterance(self.utterance));
        WordTimingLine {
            line,
            words: self.words,
        }
    }
}

/// Start offset of a word tier's source span. Distinct tiers never overlap,
/// so it identifies a tier and orders tiers in source order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SourceStart(u32);

impl SourceStart {
    fn of(source: &SourceTier) -> Self {
        Self(source.span().start)
    }
}

/// A word tier whose entry now lives in the candidate document.
struct InsertedWordTier {
    /// The document line of the owning utterance.
    line: usize,
    source: SourceTier,
    /// Read from the lowered entry before it moved into the document.
    timing: TierTiming,
    /// Its own lowering warnings, reported with the retained document.
    diagnostics: Vec<ParseError>,
}

/// A retained word tier holding an error, already taken out of the
/// candidate's `inserted` set, with every diagnostic located inside it.
struct FaultyTier {
    start: SourceStart,
    tier: InsertedWordTier,
    diagnostics: Vec<ParseError>,
}

/// What a rejected candidate's diagnostics say about the retained word tiers.
enum Attribution {
    /// Errors lie inside these tiers, which attribution moved out of the
    /// retained set. Never empty, so every round removes a tier.
    Located(Vec<FaultyTier>),
    /// No error lies inside a retained word tier. This includes a failure
    /// whose errors carry no source location and an incomplete-parse failure
    /// with no error at all.
    Unattributed(Vec<ParseError>),
}

/// Which word tiers are in the candidate document and which were removed.
/// The document itself is moved through validation separately and handed
/// back to [`Self::reduce`] on rejection.
pub(super) struct WordTimingCandidate {
    /// Every word tier now in the document, in source order.
    inserted: BTreeMap<SourceStart, InsertedWordTier>,
    /// Every removed tier's receipt, in source order.
    removed: BTreeMap<SourceStart, RemovedTier>,
}

impl WordTimingCandidate {
    /// Move every cleanly lowered entry into `document` at its source
    /// position; record every tier whose own lowering failed as removed.
    pub(super) fn assemble(
        document: &mut ChatFile,
        planned: Vec<WordTimingLine>,
    ) -> Result<Self, ReplacementFailure> {
        let mut candidate = Self {
            inserted: BTreeMap::new(),
            removed: BTreeMap::new(),
        };
        for planned_line in planned {
            // `position` counts every earlier word tier of the utterance; the
            // ones refused here are not in the document.
            let mut absent = 0;
            for PlacedTier {
                position,
                tier: word,
            } in planned_line.words
            {
                let start = SourceStart::of(&word.source);
                let span = word.source.span();
                match word.into_lowered(planned_line.line) {
                    Ok((entry, inserted)) => {
                        let utterance = utterance_at(document, planned_line.line, span)?;
                        let index = position
                            .checked_sub(absent)
                            .filter(|index| *index <= utterance.dependent_tiers.len())
                            .ok_or_else(|| disagreement("a word tier's position", span))?;
                        utterance.dependent_tiers.insert(index, entry);
                        candidate.inserted.insert(start, inserted);
                    }
                    Err(removed) => {
                        absent += 1;
                        candidate.removed.insert(start, removed);
                    }
                }
            }
        }
        Ok(candidate)
    }

    /// Derived from the actual removals: whether any removed tier's own
    /// lowered content carried a recorded word bullet.
    pub(super) fn discarded_recorded_timing(&self) -> bool {
        self.removed
            .values()
            .any(RemovedTier::discarded_recorded_timing)
    }

    /// Take a rejection and return the reduced document, or refuse.
    ///
    /// A tool failure is no evidence about any tier, and a rejection with no
    /// word tier left is a retained fault: both refuse. Otherwise the word
    /// tiers holding an error inside their source span are removed, each
    /// bound to the diagnostics located inside it; when no error lies inside
    /// a retained word tier, every remaining one is removed, all sharing the
    /// unattributed diagnostics. Each successful call removes at least one
    /// tier, so revalidating after each call terminates.
    ///
    /// Diagnostics outside every word tier in a round that also located a
    /// word-tier error are not bound to anything: the reduced document is
    /// validated again, and if they persist they are either bound by the
    /// unattributed fallback or refuse admission.
    pub(super) fn reduce(
        &mut self,
        failure: ValidationFailure,
    ) -> Result<ChatFile, ReplacementFailure> {
        if self.inserted.is_empty() {
            return Err(failure.into());
        }
        let (mut document, diagnostics) = failure.into_rejection()?;
        match self.attribute(diagnostics)? {
            Attribution::Located(faulty) => {
                for FaultyTier {
                    start,
                    tier,
                    diagnostics,
                } in faulty
                {
                    Self::take_out(
                        &mut self.removed,
                        &mut document,
                        start,
                        tier,
                        RemovalCause::LocatedValidation { diagnostics },
                    )?;
                }
            }
            Attribution::Unattributed(diagnostics) => {
                let shared: Arc<[ParseError]> = diagnostics.into();
                for (start, tier) in std::mem::take(&mut self.inserted) {
                    Self::take_out(
                        &mut self.removed,
                        &mut document,
                        start,
                        tier,
                        RemovalCause::UnattributedValidation {
                            diagnostics: Arc::clone(&shared),
                        },
                    )?;
                }
            }
        }
        Ok(document)
    }

    /// Consume the plan: every removal receipt, and the own-lowering
    /// diagnostics of every tier still retained, both in source order.
    pub(super) fn into_parts(self) -> (Vec<RemovedTier>, impl Iterator<Item = ParseError>) {
        let retained = self
            .inserted
            .into_values()
            .flat_map(|tier| tier.diagnostics);
        (self.removed.into_values().collect(), retained)
    }

    /// Bind each diagnostic to the retained tier it lies in, and move every
    /// tier holding an error out of the retained set.
    fn attribute(
        &mut self,
        diagnostics: Vec<ParseError>,
    ) -> Result<Attribution, ReplacementFailure> {
        let mut located: BTreeMap<SourceStart, Vec<ParseError>> = BTreeMap::new();
        let mut unlocated = Vec::new();
        for diagnostic in diagnostics {
            match self.locate(diagnostic.location.span) {
                Some(start) => located.entry(start).or_default().push(diagnostic),
                None => unlocated.push(diagnostic),
            }
        }
        // A tier is wrong only if an error lies inside it; a tier holding
        // only warnings stays, and revalidation reports them again.
        located.retain(|_, diagnostics| {
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error)
        });
        if located.is_empty() {
            return Ok(Attribution::Unattributed(unlocated));
        }
        let mut faulty = Vec::with_capacity(located.len());
        for (start, diagnostics) in located {
            // `locate` returns only keys of `inserted`; a miss is a tool fault.
            let tier = self
                .inserted
                .remove(&start)
                .ok_or_else(|| disagreement("a located word tier", Span::new(start.0, start.0)))?;
            faulty.push(FaultyTier {
                start,
                tier,
                diagnostics,
            });
        }
        Ok(Attribution::Located(faulty))
    }

    /// The retained word tier whose source span contains `span`, if any. A
    /// span with no source location is in no tier.
    fn locate(&self, span: Span) -> Option<SourceStart> {
        if span.is_dummy() {
            return None;
        }
        let (start, tier) = self
            .inserted
            .range(..=SourceStart(span.start))
            .next_back()?;
        tier.source.span().contains_span(span).then_some(*start)
    }

    /// Take one inserted tier's entry out of `document`, found by its own
    /// source span, and record why.
    fn take_out(
        removed: &mut BTreeMap<SourceStart, RemovedTier>,
        document: &mut ChatFile,
        start: SourceStart,
        tier: InsertedWordTier,
        cause: RemovalCause,
    ) -> Result<(), ReplacementFailure> {
        let span = tier.source.span();
        let utterance = utterance_at(document, tier.line, span)?;
        let index = utterance
            .dependent_tiers
            .iter()
            .position(|entry| matches!(&entry.tier, DependentTier::Wor(wor) if wor.span == span))
            .ok_or_else(|| disagreement("a retained word tier's entry", span))?;
        // The entry itself is dropped: the receipt keeps its source span and
        // its timing was read before it moved into the document.
        drop(utterance.dependent_tiers.remove(index));
        removed.insert(start, RemovedTier::word(tier.source, cause, tier.timing));
        Ok(())
    }
}

/// The utterance the producer bound to `line`, for the word tier at `span`.
/// Anything else there means the plan and the document disagree, which is a
/// tool failure.
fn utterance_at(
    document: &mut ChatFile,
    line: usize,
    span: Span,
) -> Result<&mut Utterance, ReplacementFailure> {
    match document.lines.as_mut_slice().get_mut(line) {
        Some(Line::Utterance(utterance)) => Ok(utterance),
        Some(Line::Header { .. }) | None => {
            Err(disagreement("the line of a word tier's utterance", span))
        }
    }
}

/// The plan and the document it assembled disagree about `what`. That is a
/// tool failure, never evidence about the CHAT input.
fn disagreement(what: &str, span: Span) -> ReplacementFailure {
    ReplacementFailure::Internal(InternalFailure::tool_fault(
        format!("word-timing plan and document disagree about {what}"),
        span,
    ))
}
