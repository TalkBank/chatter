//! Coordinated `%mor` / `%gra` mutation: splice items on the morphological tier
//! while keeping the grammatical-relation tier's indices, heads, and cardinality
//! consistent.
//!
//! [`MorTier::splice_range_coordinated`]
//! replaces a range of items with a [`SplicedBlock`];
//! [`MorTier::splice_coordinated`] is the
//! same for a range of one item. The parent re-exports every type here
//! (`mor::tier::...` and `mor::...`).
//!
//! # Where a host dependent of a replaced item points afterwards
//!
//! A host relation outside the replaced range whose head is a replaced chunk
//! depended on a WORD. After the splice it must depend on the chunk of the new
//! block that stands for that word, and only the caller knows the
//! correspondence between old and new items. The caller states it as a
//! [`HostRedirects`], one [`ItemTarget`] per replaced item: a named block
//! chunk, or the item's counterpart in the block. There is no default target:
//! a statement that cannot place a dependent is refused.
//!
//! # Where the block's root attaches
//!
//! The block's span root (its one relation with head `0`) attaches where the
//! caller's [`SpanRoot`] says: the utterance's root (relation `ROOT`), or a
//! host chunk named as it is numbered BEFORE the splice, with the relation the
//! span root takes under it (an [`AttachmentRelation`], which cannot be
//! `ROOT`). The splice translates the chunk like every other host head.
//!
//! What the splice guarantees and refuses, with a worked L2 example, is
//! documented on [`MorTier::splice_range_coordinated`].

mod block;
mod error;
mod host_chunks;

pub use block::{BlockChunk, SplicedBlock, SplicedBlockError};
pub use error::CoordinatedMutationError;

use std::num::NonZeroUsize;

use block::{BlockHead, BlockRelation};
use host_chunks::{
    AdmittedHost, Geometry, Place, PostHead, PreChunk, PreHead, ReplacedChunk, ReplacedItem,
};

use crate::alignment::indices::{GraHeadRef, SemanticWordIndex1};
use crate::model::dependent_tier::gra::{GraTier, GrammaticalRelation, GrammaticalRelationType};
use crate::model::dependent_tier::mor::item::Mor;

use super::MorTier;

/// Where every host dependent of one replaced item points after the splice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemTarget {
    /// The item's counterpart: the block item at the same position. A
    /// dependent of old chunk `j` points at the counterpart's chunk `j` when
    /// the two items have the same chunk count, and otherwise at the
    /// counterpart's head chunk, its one chunk whose head lies outside the
    /// item ([`CoordinatedMutationError::NoUniqueHeadChunk`] when a dependent
    /// needs one that is not unique).
    Counterpart,
    /// This chunk of the block, for every dependent of the item.
    Chunk(BlockChunk),
}

/// Where host dependents of the replaced items point after a coordinated
/// splice: the caller's statement of how old items correspond to new ones.
///
/// It is checked against the admitted host and the block before anything
/// changes, so it is validated where the facts it depends on (the old items'
/// chunk counts, the block's chunks and heads) are known, and nowhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRedirects {
    /// Old item `k` corresponds to block item `k`: [`ItemTarget::Counterpart`]
    /// for every item, and the item counts must be equal.
    ByItem,
    /// One target per replaced old item, in order. A caller can state one
    /// item's target and let the others follow their counterparts.
    PerItem(Vec<ItemTarget>),
}

/// The relation a span root takes under a host chunk: any label but `ROOT`
/// (or the retired `INCROOT`), so a root label under a head is
/// unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRelation(GrammaticalRelationType);

/// A root label (`ROOT` or `INCROOT`, any case, any subtype) offered as the
/// relation of a span root under a host chunk.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{label} is a root label; a span root under a host chunk takes a dependent relation")]
pub struct RootRelationUnderHost {
    /// The label offered.
    pub label: GrammaticalRelationType,
}

impl AttachmentRelation {
    /// The relation `label`, refused when it is a root label.
    pub fn new(label: impl Into<GrammaticalRelationType>) -> Result<Self, RootRelationUnderHost> {
        let label = label.into();
        match crate::validation::utterance::gra_relation_vocabulary::is_root_label(label.as_str()) {
            true => Err(RootRelationUnderHost { label }),
            false => Ok(Self(label)),
        }
    }

    /// The label.
    pub fn as_relation(&self) -> &GrammaticalRelationType {
        &self.0
    }
}

/// Where the block's span root attaches after a coordinated splice.
///
/// There is no positional default: the caller states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanRoot {
    /// The utterance's root: the span root gets head `0` and relation
    /// `ROOT`. Refused while a host relation outside the replaced range is
    /// already the root.
    UtteranceRoot,
    /// Under a host chunk, with a dependent relation.
    HostChunk {
        /// The host chunk, numbered as BEFORE the splice (the numbering the
        /// caller read it in); the splice translates it. It must lie outside
        /// the replaced range, within the host, and not depend on the range.
        chunk: SemanticWordIndex1,
        /// The relation the span root takes under it.
        relation: AttachmentRelation,
    },
}

impl SpanRoot {
    /// The span root a `%gra` head names, for a caller that reads the anchor
    /// off a host relation (such as the head the span's last word had, from
    /// [`GrammaticalRelation::head_ref`] or
    /// [`MorTier::governing_head_for_item`]): the head is already typed, and
    /// [`GraHeadRef::Word`] carries the host chunk as the
    /// [`SemanticWordIndex1`] that [`SpanRoot::HostChunk`] takes, so nothing
    /// is checked again.
    /// [`GraHeadRef::Root`] is the utterance's root, whose relation is always
    /// `ROOT`, so `relation` is the span root's relation only under a word.
    pub fn from_gra_head(head: GraHeadRef, relation: AttachmentRelation) -> Self {
        match head {
            GraHeadRef::Root => Self::UtteranceRoot,
            GraHeadRef::Word(chunk) => Self::HostChunk { chunk, relation },
        }
    }
}

/// Where a dependent of one replaced old chunk goes.
#[derive(Debug, Clone, Copy)]
enum ChunkRedirect {
    /// To this chunk of the block.
    To(BlockChunk),
    /// Nowhere that can be read off the block: its item's counterpart has
    /// `head_chunks` (not exactly one) chunks headed outside it. An error
    /// only if a host relation actually depends on it.
    NoUniqueHead {
        /// The replaced item.
        item: ReplacedItem,
        /// How many chunks of the new item have a head outside it.
        head_chunks: usize,
    },
}

/// A [`HostRedirects`] validated against one admitted host range and one
/// block: one [`ChunkRedirect`] per replaced old chunk, in the range's chunk
/// order, so a [`ReplacedChunk`] is its position here. Built only by
/// [`ChunkRedirects::resolve`].
struct ChunkRedirects(Vec<ChunkRedirect>);

impl ChunkRedirects {
    /// Validate `redirects` against the replaced `old_items` and `block`.
    fn resolve(
        redirects: HostRedirects,
        old_items: &[Mor],
        block: &SplicedBlock,
    ) -> Result<Self, CoordinatedMutationError> {
        let new_items = block.items();
        let targets = match redirects {
            HostRedirects::ByItem => match old_items.len() == new_items.len() {
                true => vec![ItemTarget::Counterpart; old_items.len()],
                false => {
                    return Err(CoordinatedMutationError::RedirectItemCountsDiffer {
                        old_items: old_items.len(),
                        new_items: new_items.len(),
                    });
                }
            },
            HostRedirects::PerItem(targets) => match targets.len() == old_items.len() {
                true => targets,
                false => {
                    return Err(CoordinatedMutationError::RedirectCountMismatch {
                        targets: targets.len(),
                        old_items: old_items.len(),
                    });
                }
            },
        };
        let mut chunks = Vec::new();
        // The block chunk where the current item's counterpart begins.
        let mut counterpart_start = BlockChunk::FIRST;
        for (item, (old, target)) in ReplacedItem::numbered(old_items.iter().zip(targets)) {
            let counterpart = item.counterpart(new_items);
            match (target, counterpart) {
                (ItemTarget::Chunk(target), _) => match target.get() <= block.chunk_count() {
                    true => chunks.extend(std::iter::repeat_n(
                        ChunkRedirect::To(target),
                        old.count_chunks(),
                    )),
                    false => {
                        return Err(CoordinatedMutationError::RedirectOutOfBlock {
                            item: item.get(),
                            target,
                            new_chunks: block.chunk_count(),
                        });
                    }
                },
                (ItemTarget::Counterpart, None) => {
                    return Err(CoordinatedMutationError::NoCounterpart {
                        item: item.get(),
                        new_items: new_items.len(),
                    });
                }
                (ItemTarget::Counterpart, Some(new)) => {
                    let item_chunks = counterpart_start.and_following(new.count_chunks());
                    match old.count_chunks() == new.count_chunks() {
                        // Same shape: chunk j stands for chunk j.
                        true => chunks.extend(item_chunks.map(ChunkRedirect::To)),
                        false => {
                            let redirect = Self::head_chunk(block, item, item_chunks);
                            chunks.extend(std::iter::repeat_n(redirect, old.count_chunks()));
                        }
                    }
                }
            }
            if let Some(new) = counterpart {
                counterpart_start = counterpart_start.advanced_by(new.count_chunks());
            }
        }
        Ok(Self(chunks))
    }

    /// The head chunk of a block item (`item_chunks`): its one chunk whose
    /// head is the span root or another item's chunk.
    fn head_chunk(
        block: &SplicedBlock,
        item: ReplacedItem,
        item_chunks: impl Iterator<Item = BlockChunk> + Clone,
    ) -> ChunkRedirect {
        let inside = |chunk: BlockChunk| item_chunks.clone().any(|own| own == chunk);
        let mut headed_outside =
            item_chunks
                .clone()
                .filter(|chunk| match block.relation(*chunk).head {
                    BlockHead::SpanRoot => true,
                    BlockHead::Chunk(head) => !inside(head),
                });
        match (headed_outside.next(), headed_outside.next()) {
            (Some(chunk), None) => ChunkRedirect::To(chunk),
            (None, _) => ChunkRedirect::NoUniqueHead {
                item,
                head_chunks: 0,
            },
            (Some(_), Some(_)) => ChunkRedirect::NoUniqueHead {
                item,
                head_chunks: 2 + headed_outside.count(),
            },
        }
    }

    /// The block chunk for host relation `dependent`, whose head is the
    /// replaced chunk `head`. `resolve` built one entry per replaced chunk
    /// and only [`Geometry::locate`] makes a [`ReplacedChunk`], from the same
    /// range, so the entry exists.
    fn target(
        &self,
        dependent: PreChunk,
        head: ReplacedChunk,
    ) -> Result<BlockChunk, CoordinatedMutationError> {
        match self.0[head.position()] {
            ChunkRedirect::To(chunk) => Ok(chunk),
            ChunkRedirect::NoUniqueHead { item, head_chunks } => {
                Err(CoordinatedMutationError::NoUniqueHeadChunk {
                    dependent: dependent.as_semantic().as_usize(),
                    item: item.get(),
                    head_chunks,
                })
            }
        }
    }
}

/// Where the span root attaches after the splice: its post-splice head and
/// its relation.
struct RootAttachment {
    /// The utterance's root, or a kept host chunk renumbered.
    head: PostHead,
    /// `ROOT` under the utterance's root, else the caller's relation.
    relation: GrammaticalRelationType,
}

impl RootAttachment {
    /// Where `root` attaches after the splice, validated against the
    /// admitted `host` and the range's `geometry` before anything changes.
    fn resolve(
        host: &AdmittedHost<'_>,
        geometry: Geometry,
        root: SpanRoot,
    ) -> Result<Self, CoordinatedMutationError> {
        let (anchor, relation) = match root {
            SpanRoot::UtteranceRoot => {
                let host_root = host.chunks().find(|(chunk, relation)| {
                    matches!(PreHead::of(relation), PreHead::Root)
                        && matches!(geometry.locate(*chunk), Place::Kept(_))
                });
                return match host_root {
                    Some((chunk, _)) => Err(CoordinatedMutationError::UtteranceRootTaken {
                        host_root: chunk.as_semantic(),
                    }),
                    None => Ok(Self {
                        head: PostHead::Root,
                        relation: GrammaticalRelationType::new("ROOT"),
                    }),
                };
            }
            SpanRoot::HostChunk { chunk, relation } => (PreChunk::named(chunk), relation),
        };
        if host.relation(anchor).is_none() {
            return Err(CoordinatedMutationError::SpanRootOutOfHost {
                chunk: anchor.as_semantic(),
                host_chunks: host.chunk_count(),
            });
        }
        let kept = match geometry.locate(anchor) {
            Place::Kept(kept) => kept,
            Place::Replaced(_) => {
                return Err(CoordinatedMutationError::SpanRootInReplacedRange {
                    chunk: anchor.as_semantic(),
                    first: geometry.first_replaced().as_semantic(),
                    last: geometry.last_replaced().as_semantic(),
                });
            }
        };
        // The anchor's own chain of heads must not reach the replaced range,
        // or the span would hang under a word that hangs under the span.
        // The walk ends at the root, at a head past the host (a host defect
        // the splice does not touch), or after as many steps as the host has
        // chunks (a host cycle that does not pass through the range, which
        // the splice neither makes nor joins).
        let mut current = anchor;
        for _ in 0..host.chunk_count() {
            let Some(relation) = host.relation(current) else {
                break;
            };
            match PreHead::of(relation) {
                PreHead::Root => break,
                PreHead::Chunk(head) => match geometry.locate(head) {
                    Place::Replaced(_) => {
                        return Err(CoordinatedMutationError::SpanRootDependsOnSpan {
                            chunk: anchor.as_semantic(),
                            through: head.as_semantic(),
                        });
                    }
                    Place::Kept(_) => current = head,
                },
            }
        }
        Ok(Self {
            head: PostHead::Chunk(geometry.translate(kept)),
            relation: relation.0,
        })
    }
}

/// A splice decided and checked against the host, not yet written: the new
/// items and the whole post-splice `%gra`, holding the exact tiers it was
/// checked against, so it can be written into no others. Every refusal
/// happens while it is built, before anything is written, so writing it
/// cannot fail and a refusal has nothing to undo.
struct SplicePlan<'a> {
    /// The host `%mor` tier the plan was checked against.
    mor: &'a mut MorTier,
    /// The host `%gra` tier the plan was checked against.
    gra: &'a mut GraTier,
    /// The host items replaced.
    item_range: std::ops::Range<usize>,
    /// The items that replace them.
    mors: Vec<Mor>,
    /// The whole `%gra` tier after the splice, in the post-splice numbering.
    relations: Vec<GrammaticalRelation>,
}

impl<'a> SplicePlan<'a> {
    fn new(
        mor: &'a mut MorTier,
        gra: &'a mut GraTier,
        item_range: std::ops::Range<usize>,
        block: SplicedBlock,
        root: SpanRoot,
        redirects: HostRedirects,
    ) -> Result<Self, CoordinatedMutationError> {
        if item_range.start >= item_range.end {
            return Err(CoordinatedMutationError::InvalidItemRange {
                start: item_range.start,
                end: item_range.end,
            });
        }
        let Some(old_items) = mor.items.as_slice().get(item_range.clone()) else {
            return Err(CoordinatedMutationError::ItemIndexOutOfBounds {
                index: item_range.end,
                len: mor.items.len(),
            });
        };
        let old_chunks: usize = old_items.iter().map(Mor::count_chunks).sum();
        let chunk_offset: usize = mor
            .items
            .iter()
            .take(item_range.start)
            .map(Mor::count_chunks)
            .sum();
        let needed = chunk_offset + old_chunks;
        if gra.relations.len() < needed {
            return Err(CoordinatedMutationError::GraTierTooShort {
                gra_len: gra.relations.len(),
                needed,
                chunk_offset,
                old_chunks,
            });
        }
        let host = AdmittedHost::admit(gra.relations.as_slice())?;
        // Every item has a chunk, so only an empty range has none; the check
        // above already refused that, and this names the same refusal.
        let Some(old) = NonZeroUsize::new(old_chunks) else {
            return Err(CoordinatedMutationError::InvalidItemRange {
                start: item_range.start,
                end: item_range.end,
            });
        };
        let geometry = Geometry::new(chunk_offset, old, block.chunks());
        let root = RootAttachment::resolve(&host, geometry, root)?;
        let redirects = ChunkRedirects::resolve(redirects, old_items, &block)?;

        let (mors, block_relations) = block.into_parts();
        // The block's relations at their host numbers, its span root where
        // `root` attaches it.
        let mut placed_block: Vec<GrammaticalRelation> = BlockChunk::FIRST
            .and_following(block_relations.len())
            .zip(block_relations)
            .map(|(chunk, BlockRelation { head, relation })| {
                let index = geometry.place(chunk).get();
                match head {
                    BlockHead::SpanRoot => GrammaticalRelation {
                        index,
                        head: root.head.written(),
                        relation: root.relation.clone(),
                    },
                    BlockHead::Chunk(head) => GrammaticalRelation {
                        index,
                        head: geometry.place(head).get(),
                        relation,
                    },
                }
            })
            .collect();

        // Every host relation outside the range, renumbered, with its head
        // renumbered or (a head into the range) redirected into the block;
        // the block goes where the range began.
        let mut relations = Vec::with_capacity(host.chunk_count() - old.get() + placed_block.len());
        for (chunk, relation) in host.chunks() {
            match geometry.locate(chunk) {
                Place::Kept(kept) => {
                    let head = match PreHead::of(relation) {
                        PreHead::Root => PostHead::Root,
                        PreHead::Chunk(head) => match geometry.locate(head) {
                            Place::Kept(head) => PostHead::Chunk(geometry.translate(head)),
                            Place::Replaced(head) => {
                                PostHead::Chunk(geometry.place(redirects.target(chunk, head)?))
                            }
                        },
                    };
                    relations.push(GrammaticalRelation {
                        index: geometry.translate(kept).get(),
                        head: head.written(),
                        relation: relation.relation.clone(),
                    });
                }
                // The block takes the place of the replaced range's first
                // chunk; the range's later chunks contribute nothing.
                Place::Replaced(replaced) => {
                    if replaced.is_first() {
                        relations.append(&mut placed_block);
                    }
                }
            }
        }
        Ok(Self {
            mor,
            gra,
            item_range,
            mors,
            relations,
        })
    }

    /// Write the plan into the tiers it was checked against.
    fn write(self) {
        self.mor.items.0.splice(self.item_range, self.mors);
        self.gra.relations.0 = self.relations;
    }
}

impl MorTier {
    /// Replace a contiguous range of items with `block` and adjust the
    /// `%gra` tier to match, atomically: on any error, neither tier changes.
    ///
    /// # Arguments
    ///
    /// - `item_range`: the `%mor` items replaced, ordered and nonempty
    ///   ([`CoordinatedMutationError::InvalidItemRange`],
    ///   [`CoordinatedMutationError::ItemIndexOutOfBounds`]). The host
    ///   `%gra` tier must hold a relation for every chunk up to the end of
    ///   the range ([`CoordinatedMutationError::GraTierTooShort`]); it is
    ///   never clamped. Each host relation's index must be its chunk
    ///   ([`CoordinatedMutationError::HostIndexOutOfOrder`]), since every
    ///   head is read as a chunk number.
    /// - `block`: the replacement items and their block-relative relations,
    ///   a tree with one span root ([`SplicedBlock`]).
    /// - `root`: where the span root attaches, a [`SpanRoot`].
    /// - `redirects`: which block chunk each host dependent of a replaced item
    ///   points at afterwards, a [`HostRedirects`].
    ///
    /// # What changes
    ///
    /// The items and their relations are replaced; a block relation's index
    /// and head become host chunks (the block's chunk plus the number of
    /// chunks before the range), and the span root takes the head and
    /// relation its [`SpanRoot`] gives. Host relations after the block move
    /// by `new_chunks - old_chunks`, as do host heads pointing past the
    /// range. A host head pointing INTO the range is rewritten through
    /// `redirects`. Every number the result holds is made by one translation
    /// from the host's numbering before the splice, which a replaced chunk
    /// cannot pass through (`host_chunks.rs` in this module).
    ///
    /// # Guarantees and refusals
    ///
    /// On success the splice adds no cycle and no second root: if the host
    /// `%gra` was a tree (exactly one relation with head `0`, every chain of heads
    /// reaching it), the result is a tree. Three facts make that so, each checked
    /// before anything changes:
    ///
    /// - the block is a tree with exactly one root, the span root
    ///   ([`SplicedBlock::new`] refuses anything else);
    /// - the span root attaches to the utterance's root only when no host relation
    ///   outside the replaced range is already a root
    ///   ([`CoordinatedMutationError::UtteranceRootTaken`]), and to a host chunk
    ///   only when that chunk lies outside the range, within the host, and its own
    ///   chain of heads does not reach the range
    ///   ([`CoordinatedMutationError::SpanRootInReplacedRange`],
    ///   [`CoordinatedMutationError::SpanRootOutOfHost`],
    ///   [`CoordinatedMutationError::SpanRootDependsOnSpan`]);
    /// - every host relation that depended on a replaced chunk is redirected into
    ///   the block, by the caller's statement ([`HostRedirects`]), never by
    ///   position.
    ///
    /// A host that was not a tree keeps its own defects (a cycle elsewhere, a
    /// second root) and gains none. Every refusal leaves both tiers unchanged.
    ///
    ///
    /// # Example: an L2 span that grows past its host head
    ///
    /// In `dont@s:eng mal geh .` the host's primary parse has `dont -> geh` and
    /// `mal -> dont`. The span `dont` is reparsed as `do~n't`, one item of two
    /// chunks whose head chunk is `do` (`n't` depends on it). The dependent of
    /// the span's primary representative, `mal`, moves to the span root `do`
    /// (its counterpart's head chunk), and the span root keeps depending on
    /// `geh`, chunk 3 as numbered before the splice and chunk 4 after it, with
    /// the relation it had there.
    ///
    /// ```text
    /// host   *CHI:  dont@s:eng mal geh .
    ///        %mor:  x|dont adv|mal v|gehen .
    ///        %gra:  1|3|AUX 2|1|ADV 3|0|ROOT 4|3|PUNCT
    /// block  %mor:  aux|do~neg|not            (one item, two chunks)
    ///        %gra:  1|0|AUX 2|1|NEG           (do is the span root)
    ///
    /// splice_range_coordinated(gra, 0..1, SplicedBlock::new(..)?,
    ///     SpanRoot::HostChunk { chunk: 3, relation: AUX },   (where dont was)
    ///     HostRedirects::ByItem)
    ///
    /// result %mor:  aux|do~neg|not adv|mal v|gehen .
    ///        %gra:  1|4|AUX 2|1|NEG 3|1|ADV 4|0|ROOT 5|4|PUNCT
    /// ```
    ///
    /// `mal` (`3|1|ADV`) depends on `do`, the head chunk of the counterpart
    /// of `dont`; the span root (`1|4|AUX`) depends on `geh`, named as chunk 3
    /// and now chunk 4. The anchor is read off the host relation of the
    /// span's word ([`GrammaticalRelation::head_ref`], or
    /// [`MorTier::governing_head_for_item`]) with [`SpanRoot::from_gra_head`].
    /// This exact case runs over parsed CHAT in the parser-tests crate
    /// (`host_redirects_corpus.rs`,
    /// `the_span_root_and_a_dependent_follow_their_words_through_a_growing_splice`),
    /// beside the other L2 shapes and every refusal.
    pub fn splice_range_coordinated(
        &mut self,
        gra: &mut GraTier,
        item_range: std::ops::Range<usize>,
        block: SplicedBlock,
        root: SpanRoot,
        redirects: HostRedirects,
    ) -> Result<(), CoordinatedMutationError> {
        SplicePlan::new(self, gra, item_range, block, root, redirects)?.write();
        Ok(())
    }

    /// Replace the item at `item_idx` with `block`: exactly
    /// [`Self::splice_range_coordinated`] over `item_idx..item_idx + 1`
    /// ([`HostRedirects::ByItem`] is the natural statement for one item
    /// replaced by one).
    pub fn splice_coordinated(
        &mut self,
        gra: &mut GraTier,
        item_idx: usize,
        block: SplicedBlock,
        root: SpanRoot,
        redirects: HostRedirects,
    ) -> Result<(), CoordinatedMutationError> {
        // The one index whose range cannot be written is past every tier.
        let Some(end) = item_idx.checked_add(1) else {
            return Err(CoordinatedMutationError::ItemIndexOutOfBounds {
                index: item_idx,
                len: self.items.len(),
            });
        };
        self.splice_range_coordinated(gra, item_idx..end, block, root, redirects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A root label, in any case and with any subtype, cannot be the relation
    /// of a span root under a host chunk; a dependent label can.
    #[test]
    fn a_root_label_is_not_an_attachment_relation() {
        for label in ["ROOT", "root", "ROOT-x", "INCROOT"] {
            assert_eq!(
                AttachmentRelation::new(label),
                Err(RootRelationUnderHost {
                    label: GrammaticalRelationType::new(label)
                }),
                "{label}"
            );
        }
        for label in ["NMOD", "ROOTS", "DEP"] {
            assert_eq!(
                AttachmentRelation::new(label)
                    .map(|relation| relation.as_relation().as_str().to_owned()),
                Ok(label.to_owned())
            );
        }
    }
}
