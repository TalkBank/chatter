//! `%mor` chunk to `%gra` relation alignment.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::model::{GraTier, MorChunk, MorTier};
use crate::{ErrorCode, ParseError, Severity};

use super::super::format::format_positional_mismatch;
use super::super::helpers::{TierPosition, to_chat_display_string as to_string};
use super::types::{GraAlignment, GraAlignmentPair};

/// Align %mor tier to %gra tier
///
/// Performs 1-1 alignment validation between `%mor` chunks and `%gra` relations.
/// Continues alignment even on mismatch, creating error placeholders.
///
/// This API also accepts independently constructed tiers. Their zero word
/// indices report E712 when counts match; source parsing rejects that spelling
/// earlier with E709. Count mismatches report E720 instead of cascading range
/// findings. Alignment does not certify dependency-tree validity.
///
/// # Important: Chunks vs Items
///
/// %gra aligns with %mor **chunks**, not items!
/// - A single %mor item can produce multiple chunks due to clitics
/// - Example: `pro|it~v|be&PRES` → 2 chunks (pre-clitic + main)
/// - Use `MorTier::count_chunks()` to get the correct count
///
/// # Algorithm
///
/// 1. Count %mor tier chunks (including clitics)
/// 2. Count %gra tier relations
/// 3. Pair up indices 1-1 up to min(mor_chunks, gra_relations)
/// 4. Create error placeholders for extras
/// 5. Return alignment with collected errors
pub fn align_mor_to_gra(mor: &MorTier, gra: &GraTier) -> GraAlignment {
    let mut alignment = GraAlignment::new();

    // Count chunks (NOT items!) - clitics create additional chunks
    let mor_chunk_count = mor.count_chunks();
    let gra_count = gra.len();

    // Create 1-1 pairs for the common range
    let min_len = mor_chunk_count.min(gra_count);
    for i in 0..min_len {
        alignment = alignment.with_pair(GraAlignmentPair::from_raw(Some(i), Some(i)));
    }

    // Handle length mismatch.
    //
    // Emit E720 (MorGraCountMismatch) for count disagreements between
    // %mor chunks and %gra relations. E712 (GraInvalidWordIndex) and E713
    // (GraInvalidHeadIndex) are reserved for per-relation validation
    // (explicit word/head indices that fall outside the valid range) below.
    if mor_chunk_count > gra_count {
        // %mor has more chunks - %gra tier too short
        let mor_items = extract_mor_chunk_items(mor);
        let gra_items = extract_gra_relation_items(gra);
        let detailed_message =
            format_positional_mismatch("%mor chunks", "%gra relations", &mor_items, &gra_items);

        let error = ParseError::at_location(
            ErrorCode::MorGraCountMismatch,
            Severity::Error,
            mor.location,
            detailed_message,
        )
        .with_label_at(mor.location, "%mor tier")
        .with_label_at(gra.location, "%gra tier")
        .with_suggestion("Each %mor chunk (including pre/post-clitics) needs a %gra relation");

        alignment = alignment.with_error(error);

        // Add placeholders for extra %mor chunks
        for i in gra_count..mor_chunk_count {
            alignment = alignment.with_pair(GraAlignmentPair::from_raw(Some(i), None));
        }
    } else if gra_count > mor_chunk_count {
        // %gra tier has more relations - %mor tier too short
        let mor_items = extract_mor_chunk_items(mor);
        let gra_items = extract_gra_relation_items(gra);
        let detailed_message =
            format_positional_mismatch("%mor chunks", "%gra relations", &mor_items, &gra_items);

        let error = ParseError::at_location(
            ErrorCode::MorGraCountMismatch,
            Severity::Error,
            mor.location,
            detailed_message,
        )
        .with_label_at(mor.location, "%mor tier")
        .with_label_at(gra.location, "%gra tier")
        .with_suggestion("Remove extra %gra relations or add corresponding %mor chunks");

        alignment = alignment.with_error(error);

        // Add placeholders for extra %gra relations
        for i in mor_chunk_count..gra_count {
            alignment = alignment.with_pair(GraAlignmentPair::from_raw(None, Some(i)));
        }
    }

    // Validate explicit relation indices when cardinalities match.
    // If lengths already mismatch, length diagnostics above are clearer and non-duplicative.
    if mor_chunk_count == gra_count {
        let max_index = mor_chunk_count;
        for relation in gra.relations.iter() {
            if relation.index == 0 || relation.index > max_index {
                alignment = alignment.with_error(
                    ParseError::at_location(
                        ErrorCode::GraInvalidWordIndex,
                        Severity::Error,
                        gra.location,
                        format!(
                            "%gra relation word index {} is out of bounds for %mor chunk count {}",
                            relation.index, mor_chunk_count
                        ),
                    )
                    .with_suggestion(format!(
                        "Use word indices in range 1..={} (0 is reserved for ROOT head only)",
                        mor_chunk_count
                    )),
                );
            }

            if relation.head > max_index {
                alignment = alignment.with_error(
                    ParseError::at_location(
                        ErrorCode::GraInvalidHeadIndex,
                        Severity::Error,
                        gra.location,
                        format!(
                            "%gra relation head index {} is out of bounds for %mor chunk count {}",
                            relation.head, mor_chunk_count
                        ),
                    )
                    .with_suggestion(format!(
                        "Use head indices in range 0..={} (0 marks ROOT)",
                        mor_chunk_count
                    )),
                );
            }
        }
    }

    alignment
}

/// Extract %mor chunks as individual [`TierPosition`]s for diagnostic display.
///
/// Delegates the chunk walk to [`MorTier::chunks`] so there is exactly one
/// definition of "the %mor chunk sequence" in the workspace. The only
/// responsibility here is rendering each chunk as a display string and
/// attaching the `description` label that downstream diagnostic formatting
/// expects for non-main chunks.
fn extract_mor_chunk_items(mor: &MorTier) -> Vec<TierPosition> {
    mor.chunks()
        .map(|chunk| match chunk {
            MorChunk::Main(item) => TierPosition {
                text: to_string(&item.main),
                description: None,
            },
            MorChunk::PostClitic(_, clitic) => TierPosition {
                text: to_string(clitic),
                description: Some("post-clitic".to_owned()),
            },
            MorChunk::Terminator(term) => TierPosition {
                text: term.to_string(),
                description: Some("terminator".to_owned()),
            },
        })
        .collect()
}

/// Extract %gra relations as TierPositions for diagnostic display.
fn extract_gra_relation_items(gra: &GraTier) -> Vec<TierPosition> {
    gra.relations
        .iter()
        .map(|rel| TierPosition {
            text: to_string(rel),
            description: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::align_mor_to_gra;
    use crate::model::{GraTier, GrammaticalRelation, MorTier, Terminator};
    use crate::{ErrorCode, Location};

    #[derive(Clone, Copy, Debug)]
    enum WordIndex {
        Zero,
        InRange,
        PastEnd,
    }

    #[derive(Clone, Copy, Debug)]
    enum Cardinality {
        Matched,
        ExtraRelation,
    }

    #[test]
    fn independent_gra_indices_respect_range_and_count_diagnostic_boundaries() {
        // One terminator is one MOR chunk. These constructor inputs exercise
        // the model API, not a source parser's admission of malformed indices.
        let mor = MorTier::new_mor(
            Location::Unlocated,
            vec![],
            Terminator::Period {
                location: Location::Unlocated,
            },
        );
        assert_eq!(mor.count_chunks(), 1);
        for index in [WordIndex::Zero, WordIndex::InRange, WordIndex::PastEnd] {
            let raw_index = match index {
                WordIndex::Zero => 0,
                WordIndex::InRange => 1,
                WordIndex::PastEnd => 2,
            };
            for cardinality in [Cardinality::Matched, Cardinality::ExtraRelation] {
                let mut relations = vec![GrammaticalRelation::new(
                    raw_index,
                    0,
                    "PUNCT".try_into().expect("checked label"),
                )];
                match cardinality {
                    Cardinality::Matched => {}
                    Cardinality::ExtraRelation => relations.push(GrammaticalRelation::new(
                        2,
                        0,
                        "PUNCT".try_into().expect("checked label"),
                    )),
                }
                let gra = GraTier::new_gra(Location::Unlocated, relations);
                let alignment = align_mor_to_gra(&mor, &gra);
                let actual: Vec<_> = alignment.errors.iter().map(|error| error.code).collect();
                let expected = match (cardinality, index) {
                    (Cardinality::Matched, WordIndex::InRange) => vec![],
                    (Cardinality::Matched, WordIndex::Zero | WordIndex::PastEnd) => {
                        vec![ErrorCode::GraInvalidWordIndex]
                    }
                    (
                        Cardinality::ExtraRelation,
                        WordIndex::Zero | WordIndex::InRange | WordIndex::PastEnd,
                    ) => vec![ErrorCode::MorGraCountMismatch],
                };
                assert_eq!(actual, expected, "{cardinality:?}, {index:?}");
                assert_eq!(
                    gra.relations()[0].index,
                    raw_index,
                    "diagnosis preserves observed indices"
                );
            }
        }
    }
}
