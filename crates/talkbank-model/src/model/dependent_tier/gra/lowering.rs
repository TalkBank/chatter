//! All-or-rejected construction of a shared grammatical tier.

use super::{GraTier, GrammaticalRelation};
use crate::ParseOutcome;

/// Collects every observed relation outcome before constructing a tier.
///
/// A rejected relation irreversibly rejects the whole tier. No partial graph
/// escapes to serialization or downstream validation. Producers must still
/// report diagnostics and retain raw source when file recovery is required.
/// This transition establishes successful lowering, not CHAT validity or
/// independent proof that the producer visited every source construct.
#[derive(Debug)]
#[must_use]
pub struct GraLowering {
    outcome: ParseOutcome<Vec<GrammaticalRelation>>,
}

impl GraLowering {
    /// Starts collection with an allocation hint, not a declaration count.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            outcome: ParseOutcome::Parsed(Vec::with_capacity(capacity)),
        }
    }

    /// Records one observed relation outcome. Success cannot undo rejection.
    pub fn record(&mut self, outcome: ParseOutcome<GrammaticalRelation>) {
        match outcome {
            ParseOutcome::Parsed(relation) => match &mut self.outcome {
                ParseOutcome::Parsed(relations) => relations.push(relation),
                ParseOutcome::Rejected => {}
            },
            ParseOutcome::Rejected => self.outcome = ParseOutcome::Rejected,
        }
    }

    /// Consumes collection into a shared tier only if every outcome succeeded.
    pub fn finish(self, location: impl Into<crate::Location>) -> ParseOutcome<GraTier> {
        self.outcome
            .map(|relations| GraTier::new_gra(location, relations))
    }
}

impl FromIterator<ParseOutcome<GrammaticalRelation>> for GraLowering {
    fn from_iter<T: IntoIterator<Item = ParseOutcome<GrammaticalRelation>>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let mut lowering = Self::with_capacity(iter.size_hint().0);
        for outcome in iter {
            lowering.record(outcome);
        }
        lowering
    }
}
