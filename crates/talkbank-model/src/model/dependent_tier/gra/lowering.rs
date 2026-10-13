//! One owner for `%gra` relation lowering and its retention evidence.

use super::{GraCompleteness, GraTier, GrammaticalRelation};
use crate::ParseOutcome;

/// Collects the accepted and rejected relation outcomes of one lowering.
///
/// Retention is derived from the recorded outcomes, never supplied alongside a
/// relation vector. This is not proof of source origin, input completeness or
/// CHAT validity: the producer must record the outcomes it actually observes.
#[derive(Debug)]
#[must_use]
pub struct GraLowering {
    relations: Vec<GrammaticalRelation>,
    retention: Retention,
}

#[derive(Debug)]
enum Retention {
    Retained,
    Truncated,
}

impl GraLowering {
    /// Starts a lowering with an allocation hint, not a declaration count.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            relations: Vec::with_capacity(capacity),
            retention: Retention::Retained,
        }
    }

    /// Records one declared relation's actual lowering result.
    ///
    /// A later accepted relation cannot undo an earlier rejection.
    pub fn record(&mut self, outcome: ParseOutcome<GrammaticalRelation>) {
        match outcome {
            ParseOutcome::Parsed(relation) => self.relations.push(relation),
            ParseOutcome::Rejected => self.retention = Retention::Truncated,
        }
    }

    /// Consumes the mutable phase into a stable, read-only receipt.
    pub fn finish(self) -> GraLoweringReceipt {
        GraLoweringReceipt {
            relations: self.relations,
            retention: self.retention,
        }
    }
}

impl FromIterator<ParseOutcome<GrammaticalRelation>> for GraLowering {
    /// Records a stream of actual relation outcomes without a second count.
    fn from_iter<T: IntoIterator<Item = ParseOutcome<GrammaticalRelation>>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let mut lowering = Self::with_capacity(iter.size_hint().0);
        for outcome in iter {
            lowering.record(outcome);
        }
        lowering
    }
}

/// Finalized relations and their inseparable recorded retention evidence.
///
/// There is no mutable relation accessor, independent completeness argument,
/// `Default` or wire constructor. Inspecting this receipt cannot be invalidated
/// by a later lowering event; consuming it carries the same fact onto the tier.
#[derive(Debug)]
#[must_use]
pub struct GraLoweringReceipt {
    relations: Vec<GrammaticalRelation>,
    retention: Retention,
}

impl GraLoweringReceipt {
    /// Records the existing recovery case where a tier's contents were lost.
    ///
    /// No relation is invented for the unavailable contents. This observation
    /// is distinct from completing an empty stream of declared relations.
    pub fn lost_contents() -> Self {
        Self {
            relations: Vec::new(),
            retention: Retention::Truncated,
        }
    }

    /// Whether this completed lowering rejected a relation or lost its contents.
    pub fn is_truncated(&self) -> bool {
        match self.retention {
            Retention::Retained => false,
            Retention::Truncated => true,
        }
    }

    /// Consumes the relations and their derived completeness together.
    pub fn into_tier(self) -> GraTier {
        let completeness = match self.retention {
            Retention::Retained => GraCompleteness::Whole,
            Retention::Truncated => GraCompleteness::Truncated,
        };
        let mut tier = GraTier::new_gra(self.relations);
        tier.completeness = completeness;
        tier
    }
}
