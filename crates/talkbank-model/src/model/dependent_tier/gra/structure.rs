//! One immutable tier analysis, consumed by the whole-tier admission boundary.

use super::graph::{CycleEvidence, ObservedGraGraph};
use super::{GraTier, GrammaticalRelation};
use crate::{ErrorCode, ErrorSink, ParseError, Severity};

/// Shared facts borrowed from an unchanged tier. Consumers cannot pair a
/// second tier with these facts or mutate the tier while the analysis is live.
pub(crate) struct GraStructure<'a> {
    tier: &'a GraTier,
    root_count: usize,
    cycle: CycleEvidence,
}

impl<'a> GraStructure<'a> {
    /// Analyze one borrowed tier without certifying its validity or alignment.
    pub(crate) fn analyze(tier: &'a GraTier) -> Self {
        Self {
            tier,
            root_count: tier
                .relations()
                .iter()
                .filter(|relation| is_valid_root_relation(relation))
                .count(),
            cycle: ObservedGraGraph::from_relations(tier.relations()).classify(),
        }
    }

    /// E723 and E724 survive removal of unrelated relations. The graph retains
    /// every edge for repeated indices, so deletion cannot expose a new cycle
    /// by selecting a different parent.
    pub(crate) fn report_monotone(&self, errors: &impl ErrorSink) {
        if self.root_count > 1 {
            errors.report(
                ParseError::at_location(
                    ErrorCode::GraMultipleRoots,
                    Severity::Error,
                    self.tier.location,
                    format!(
                        "%gra tier has {} ROOT relations, expected 1",
                        self.root_count
                    ),
                )
                .with_suggestion("Re-run morphotag to regenerate valid %gra"),
            );
        }
        match self.cycle {
            CycleEvidence::Present => errors.report(
                ParseError::at_location(
                    ErrorCode::GraCircularDependency,
                    Severity::Error,
                    self.tier.location,
                    "%gra tier has circular dependency",
                )
                .with_suggestion("Re-run morphotag to regenerate valid %gra"),
            ),
            CycleEvidence::Absent => {}
        }
    }

    /// The utterance owns this pairing with its alignment diagnostics. Empty
    /// tiers and disputed alignments cannot support whole-tier findings.
    pub(crate) fn into_whole(self, alignment_diagnostics: &[ParseError]) -> Option<WholeGra<'a>> {
        if self.tier.is_empty()
            || alignment_diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.code,
                    ErrorCode::MorGraCountMismatch
                        | ErrorCode::GraInvalidWordIndex
                        | ErrorCode::GraInvalidHeadIndex
                )
            })
        {
            None
        } else {
            Some(WholeGra { structure: self })
        }
    }
}

/// A nonempty GRA tier whose alignment diagnostics do not dispute its whole
/// interpretation. This gates sequential-index and missing-root checks, but
/// does not certify valid CHAT or a valid dependency tree.
///
/// Rejected lowering cannot construct a GraTier. UnparsedGra recovery never
/// reaches graph validation. The admitted analysis keeps its original borrow.
pub struct WholeGra<'a> {
    structure: GraStructure<'a>,
}

impl WholeGra<'_> {
    /// E721 and E722 consume the same root/cycle evidence already analyzed.
    pub(crate) fn validate_whole_tier(&self, errors: &impl ErrorSink) {
        let structure = &self.structure;
        report_non_sequential_index(structure.tier.relations(), structure.tier.location, errors);
        if structure.root_count == 0 {
            match structure.cycle {
                // The missing root is a consequence of this already reported cycle.
                CycleEvidence::Present => {}
                CycleEvidence::Absent => errors.report(
                    ParseError::at_location(
                        ErrorCode::GraNoRoot,
                        Severity::Error,
                        structure.tier.location,
                        "%gra tier has no ROOT relation",
                    )
                    .with_suggestion("Re-run morphotag to regenerate valid %gra"),
                ),
            }
        }
    }
}

/// E721: the indices run 1, 2, ..., N.
fn report_non_sequential_index(
    relations: &[GrammaticalRelation],
    location: crate::Location,
    errors: &impl crate::ErrorSink,
) {
    use crate::{ErrorCode, ParseError, Severity};

    for (position, relation) in relations.iter().enumerate() {
        let expected = position + 1;
        if relation.index != expected {
            errors.report(
                ParseError::at_location(
                    ErrorCode::GraNonSequentialIndex,
                    Severity::Error,
                    location,
                    format!(
                        "%gra indices not sequential: expected {expected}, found {}",
                        relation.index
                    ),
                )
                .with_suggestion("Indices must be 1, 2, 3, ..., N"),
            );
            // One is enough: after the first break every later index is
            // reported against an expectation the tier has already left.
            break;
        }
    }
}

/// Count actual ROOT relations by position, including a root at the final
/// position. A self-headed non-ROOT edge remains a cycle.
fn is_valid_root_relation(relation: &GrammaticalRelation) -> bool {
    relation.relation.as_str().eq_ignore_ascii_case("ROOT")
        && (relation.head == 0 || relation.head == relation.index)
}
