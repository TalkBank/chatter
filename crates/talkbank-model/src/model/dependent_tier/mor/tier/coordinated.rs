//! Coordinated `%mor` / `%gra` mutation: splice items on the morphological tier
//! while keeping the grammatical-relation tier's indices, heads, and cardinality
//! consistent.
//!
//! The two public methods share host admission and one rewrite implementation:
//! ([`MorTier::splice_coordinated`](super::MorTier::splice_coordinated) and the
//! multi-item [`MorTier::splice_range_coordinated`](super::MorTier::splice_range_coordinated))
//! both stay inherent methods of [`MorTier`](super::MorTier); the parent re-exports
//! [`CoordinatedMutationError`] so its path is unchanged.

use crate::alignment::indices::SemanticWordIndex1;
use crate::model::dependent_tier::gra::{GraTier, GrammaticalRelation};
use crate::model::dependent_tier::mor::item::Mor;

use super::MorTier;

/// Errors returned by coordinated Mor-Gra mutations.
#[derive(Debug, thiserror::Error)]
pub enum CoordinatedMutationError {
    /// Replacement requires an ordered, nonempty item range.
    #[error("Replacement range {start}..{end} must be ordered and nonempty")]
    InvalidItemRange {
        /// Inclusive start of the requested range.
        start: usize,
        /// Exclusive end of the requested range.
        end: usize,
    },
    /// Mor and Gra tiers have mismatched chunk counts before or after mutation.
    #[error("Mor and Gra tiers have mismatched chunk counts: mor={mor}, gra={gra}")]
    CountMismatch {
        /// Chunk count in the %mor tier.
        mor: usize,
        /// Number of relations in the %gra tier.
        gra: usize,
    },
    /// Requested item index is out of bounds.
    #[error("Item index {index} out of bounds (len={len})")]
    ItemIndexOutOfBounds {
        /// The requested 0-indexed item position.
        index: usize,
        /// Total number of items in the tier.
        len: usize,
    },
    /// A new relation's head value is outside the new block but the
    /// caller's contract said it should be in span-relative-1-indexed
    /// space (i.e. within `1..=new_chunks`). Indicates a bug in the
    /// caller's head computation, not a misalignment of the host file.
    #[error(
        "New gra relation has head={head} but the new block has only \
         {new_chunks} chunks (heads must be in 1..={new_chunks} or 0)"
    )]
    HeadOutOfNewBlock {
        /// The offending head value.
        head: usize,
        /// Total chunk count of the new block.
        new_chunks: usize,
    },
    /// The host `%gra` tier is shorter than the chunk range the caller
    /// asked us to splice over. The caller passed a stale `item_range`
    /// or the host file has a pre-existing alignment defect that the
    /// caller should have detected first.
    #[error(
        "Host %gra tier has {gra_len} relations but splice needs {needed} \
         (chunk_offset={chunk_offset}, old_chunks={old_chunks})"
    )]
    GraTierTooShort {
        /// Number of relations currently in the host tier.
        gra_len: usize,
        /// Minimum needed: `chunk_offset + old_chunks`.
        needed: usize,
        /// Where the splice would start.
        chunk_offset: usize,
        /// How many chunks the splice would consume.
        old_chunks: usize,
    },
    /// A helper needed the `%gra` relation at a semantic chunk position but the
    /// host tier ended earlier.
    #[error(
        "Host %gra tier has {gra_len} relations but item start needs semantic index {semantic_index}"
    )]
    GraRelationMissing {
        /// The 1-indexed semantic chunk position that should have a matching
        /// `%gra` relation.
        semantic_index: SemanticWordIndex1,
        /// Number of relations currently in the host tier.
        gra_len: usize,
    },
}

/// Exclusive ownership of the exact host pair whose replacement range was
/// checked. No tier can change between admission and the coordinated edit.
struct HostReplacement<'a> {
    mor: &'a mut MorTier,
    gra: &'a mut GraTier,
    item_range: std::ops::Range<usize>,
    chunk_offset: usize,
    old_chunks: usize,
    old_head: usize,
}

impl<'a> HostReplacement<'a> {
    fn admit(
        mor: &'a mut MorTier,
        gra: &'a mut GraTier,
        item_range: std::ops::Range<usize>,
    ) -> Result<Self, CoordinatedMutationError> {
        if item_range.start >= item_range.end {
            return Err(CoordinatedMutationError::InvalidItemRange {
                start: item_range.start,
                end: item_range.end,
            });
        }
        let Some(items) = mor.items.as_slice().get(item_range.clone()) else {
            return Err(CoordinatedMutationError::ItemIndexOutOfBounds {
                index: item_range.end,
                len: mor.items.len(),
            });
        };
        let old_chunks = items.iter().map(Mor::count_chunks).sum();
        let chunk_offset: usize = mor
            .items
            .iter()
            .take(item_range.start)
            .map(Mor::count_chunks)
            .sum();
        let needed = chunk_offset + old_chunks;
        let Some(relations) = gra.relations.as_slice().get(chunk_offset..needed) else {
            return Err(CoordinatedMutationError::GraTierTooShort {
                gra_len: gra.relations.len(),
                needed,
                chunk_offset,
                old_chunks,
            });
        };
        let Some(first) = relations.first() else {
            return Err(CoordinatedMutationError::GraTierTooShort {
                gra_len: gra.relations.len(),
                needed: chunk_offset + 1,
                chunk_offset,
                old_chunks,
            });
        };
        let old_head = first.head;
        Ok(Self {
            mor,
            gra,
            item_range,
            chunk_offset,
            old_chunks,
            old_head,
        })
    }
}

impl MorTier {
    /// Replace a CONTIGUOUS RANGE of items and adjust the corresponding
    /// `%gra` relations atomically.
    ///
    /// This is the multi-item analog of [`Self::splice_coordinated`].
    /// The two methods share the same head-rewrite contract, heads in
    /// `1..=new_chunks` are within-block and remapped to
    /// `chunk_offset + head`; head 0 is the root anchor, but the range
    /// version interprets `new_chunks` as the SUM of chunks across all
    /// items in `new_mors`. This is what makes it correct for L2 spans
    /// covering multiple host words: the secondary Stanza sentence
    /// produces gras with cross-word heads (e.g. `la → fecha`), and the
    /// per-item splice path misclassifies those as within-MWT and
    /// remaps them with the wrong `chunk_offset`.
    ///
    /// The `new_relations` list must have length equal to
    /// `sum(new_mors[i].count_chunks())`. Heads inside `new_relations`
    /// must be either `0` (span-internal root marker) or in
    /// `1..=sum(new_mors[i].count_chunks())` (span-relative within-block
    /// reference). Heads outside that range yield
    /// [`CoordinatedMutationError::HeadOutOfNewBlock`], that contract
    /// holds the caller responsible for resolving any TRULY external
    /// references to host-absolute indices BEFORE calling this method,
    /// so the model layer never has to guess.
    ///
    /// Existing relations OUTSIDE the new block are reindexed and
    /// head-shifted by `delta = new_chunks - old_chunks`.
    ///
    /// The replaced item range must be ordered and nonempty. Refuses
    /// [`CoordinatedMutationError::InvalidItemRange`] otherwise, without mutation.
    /// Refuses (returns [`CoordinatedMutationError::GraTierTooShort`])
    /// when the host gra tier does not contain at least
    /// `chunk_offset + old_chunks` relations. We do NOT clamp silently,
    /// soft-clamping is explicitly rejected here, since it hides exactly
    /// the cardinality regressions this method exists to fix.
    pub fn splice_range_coordinated(
        &mut self,
        gra: &mut GraTier,
        item_range: std::ops::Range<usize>,
        new_mors: Vec<Mor>,
        new_relations: Vec<GrammaticalRelation>,
        root_anchor_override: Option<usize>,
    ) -> Result<(), CoordinatedMutationError> {
        let HostReplacement {
            mor,
            gra,
            item_range,
            chunk_offset,
            old_chunks,
            old_head,
        } = HostReplacement::admit(self, gra, item_range)?;
        // New chunk count is the sum across all new mors.
        let new_chunks: usize = new_mors.iter().map(|m| m.count_chunks()).sum();

        if new_relations.len() != new_chunks {
            return Err(CoordinatedMutationError::CountMismatch {
                mor: new_chunks,
                gra: new_relations.len(),
            });
        }

        // Validate every new relation's head BEFORE mutating anything,
        // so on error we leave both tiers unchanged.
        for rel in &new_relations {
            if rel.head != 0 && rel.head > new_chunks {
                return Err(CoordinatedMutationError::HeadOutOfNewBlock {
                    head: rel.head,
                    new_chunks,
                });
            }
        }

        let delta = (new_chunks as isize) - (old_chunks as isize);

        // 1. Update %mor items: replace the entire range with new_mors.
        mor.items.0.splice(item_range, new_mors);

        // 2. Reindex the new relations to host-1-indexed.
        let mut fixed_relations = new_relations;
        for (i, rel) in fixed_relations.iter_mut().enumerate() {
            rel.index = chunk_offset + i + 1;
        }

        // 4. Splice the gra range.
        gra.relations
            .0
            .splice(chunk_offset..chunk_offset + old_chunks, fixed_relations);

        // 5. Reindex relations AFTER the new block so their `index`
        //    fields match their new tier position.
        if delta != 0 {
            for i in (chunk_offset + new_chunks)..gra.relations.len() {
                let rel = &mut gra.relations.0[i];
                rel.index = (rel.index as isize + delta) as usize;
            }
        }

        // 6. Adjust heads:
        //    - For new relations (positions [chunk_offset, chunk_offset+new_chunks)):
        //      head=0 → root_anchor_override or old_head_at_start
        //      head ∈ [1, new_chunks] → chunk_offset + head (within span)
        //    - For existing relations elsewhere:
        //      head pointing into the replaced range → collapse to chunk_offset + 1
        //      head pointing past the replaced range → shift by delta
        for (i, rel) in gra.relations.0.iter_mut().enumerate() {
            if i >= chunk_offset && i < chunk_offset + new_chunks {
                if rel.head == 0 {
                    rel.head = root_anchor_override.unwrap_or(old_head);
                } else {
                    // Already validated head ≤ new_chunks above, so this
                    // is always a within-block reference.
                    rel.head += chunk_offset;
                }
                if rel.head == 0 {
                    rel.relation = "ROOT".into();
                }
            } else if rel.head > chunk_offset {
                if rel.head <= chunk_offset + old_chunks {
                    // Head was pointing into the range we replaced; the
                    // safest collapse target is the first chunk of the
                    // new block (matches splice_coordinated convention).
                    rel.head = chunk_offset + 1;
                } else {
                    // Head was pointing past the replaced range; shift
                    // by delta to keep pointing at the same conceptual
                    // chunk after the splice.
                    rel.head = (rel.head as isize + delta) as usize;
                }
            }
        }

        Ok(())
    }

    /// Replace the item at `item_idx` and adjust the corresponding `%gra`
    /// relations to maintain cardinality and index invariants.
    ///
    /// The `new_relations` list must match the chunk count of `new_mor`. This
    /// method handles re-indexing and head-adjustment for all subsequent
    /// relations in the `%gra` tier.
    pub fn splice_coordinated(
        &mut self,
        gra: &mut GraTier,
        item_idx: usize,
        new_mor: Mor,
        new_relations: Vec<GrammaticalRelation>,
        root_anchor_override: Option<usize>,
    ) -> Result<(), CoordinatedMutationError> {
        if item_idx >= self.items.len() {
            return Err(CoordinatedMutationError::ItemIndexOutOfBounds {
                index: item_idx,
                len: self.items.len(),
            });
        }

        self.splice_range_coordinated(
            gra,
            item_idx..item_idx + 1,
            vec![new_mor],
            new_relations,
            root_anchor_override,
        )
    }
}
