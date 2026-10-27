//! The new block of a coordinated splice: its items and its relations in
//! block-relative form, validated once where they are handed over.
//!
//! A [`SplicedBlock`] is a dependency tree over its own chunks with exactly
//! one root, the span root, whose attachment the splice's [`SpanRoot`]
//! states. Building it is the only route from raw `%gra` relations to a
//! block, so the splice never sees a block with no root, two roots, a cycle,
//! a head outside the block or a relation count that differs from the chunk
//! count.
//!
//! [`SpanRoot`]: super::SpanRoot

use std::num::NonZeroUsize;

use crate::model::dependent_tier::gra::{GrammaticalRelation, GrammaticalRelationType};
use crate::model::dependent_tier::mor::item::Mor;

/// A chunk of a [`SplicedBlock`], numbered from 1 within the block.
///
/// Its own space, apart from the host's chunk numbering
/// ([`SemanticWordIndex1`](crate::alignment::indices::SemanticWordIndex1)):
/// block chunk 1 is the block's first chunk wherever the block lands in the
/// host. A caller names one in [`ItemTarget::Chunk`](super::ItemTarget::Chunk);
/// the splice refuses one past the block it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockChunk(NonZeroUsize);

impl BlockChunk {
    /// The block's first chunk.
    pub const FIRST: Self = Self(NonZeroUsize::MIN);

    /// The block chunk numbered `chunk` (1-based within the block).
    pub const fn new(chunk: NonZeroUsize) -> Self {
        Self(chunk)
    }

    /// The 1-based number within the block.
    pub const fn get(self) -> usize {
        self.0.get()
    }

    /// The chunk at 0-based `position` within the block.
    pub(super) const fn at_position(position: usize) -> Self {
        Self(NonZeroUsize::MIN.saturating_add(position))
    }

    /// The 1-based number within the block, for a renumbering into the host.
    pub(super) const fn as_nonzero(self) -> NonZeroUsize {
        self.0
    }

    /// The `count` chunks starting at this one.
    pub(super) fn and_following(self, count: usize) -> impl Iterator<Item = Self> + Clone {
        (0..count).map(move |offset| Self(self.0.saturating_add(offset)))
    }

    /// The chunk `count` chunks after this one.
    pub(super) fn advanced_by(self, count: usize) -> Self {
        Self(self.0.saturating_add(count))
    }
}

impl std::fmt::Display for BlockChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// What a chunk of the block depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockHead {
    /// Nothing in the block: this chunk is the span root, attached where the
    /// splice's `SpanRoot` says.
    SpanRoot,
    /// Another chunk of the block.
    Chunk(BlockChunk),
}

/// One chunk's relation inside the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BlockRelation {
    /// What the chunk depends on.
    pub(super) head: BlockHead,
    /// Its label. The span root's label is replaced by its attachment's.
    pub(super) relation: GrammaticalRelationType,
}

/// Why a [`SplicedBlock`] could not be built from items and relations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplicedBlockError {
    /// One relation per chunk of the items is needed.
    #[error("The block's items have {chunks} chunk(s) but {relations} relation(s) were given")]
    CountMismatch {
        /// Chunks of the items, summed.
        chunks: usize,
        /// Relations given.
        relations: usize,
    },
    /// A relation's `index` is not its chunk: relations are given in chunk
    /// order and numbered from 1 within the block, the numbering their heads
    /// use.
    #[error("The relation for block chunk {chunk} has index {index}")]
    IndexOutOfOrder {
        /// The chunk the relation stands at.
        chunk: BlockChunk,
        /// The index it carries.
        index: usize,
    },
    /// A head names no chunk of the block (heads are `0`, the span root, or
    /// `1..=chunks`). A head the caller knows to lie outside the span is
    /// resolved to the span root (`0`) before the block is built.
    #[error("Block chunk {chunk} has head {head}, outside the block's {chunks} chunk(s)")]
    HeadOutOfBlock {
        /// The chunk whose head is outside.
        chunk: BlockChunk,
        /// The head it gave.
        head: usize,
        /// Chunks in the block.
        chunks: usize,
    },
    /// A block has exactly one span root (one relation with head `0`).
    #[error("The block has {roots} span root(s) (relations with head 0); it needs exactly one")]
    RootCount {
        /// Relations with head `0`.
        roots: usize,
    },
    /// A chunk's chain of heads never reaches the span root.
    #[error("Block chunk {chunk} is on a cycle: its chain of heads never reaches the span root")]
    Cycle {
        /// A chunk whose chain does not reach the root.
        chunk: BlockChunk,
    },
}

/// Dense chunk order and all head bounds have been checked. No root or
/// reachability claim is available until this owner is consumed.
struct BoundedBlockGraph {
    chunk_count: NonZeroUsize,
    relations: Vec<BlockRelation>,
}

/// Every admitted chain reaches this unique root. Only graph admission can
/// construct the immutable facts retained by a replacement block.
#[derive(Debug, Clone, PartialEq)]
struct RootedBlockGraph {
    chunk_count: NonZeroUsize,
    root: BlockChunk,
    relations: Vec<BlockRelation>,
}

impl BoundedBlockGraph {
    fn new(relations: Vec<GrammaticalRelation>, chunks: usize) -> Result<Self, SplicedBlockError> {
        if relations.len() != chunks {
            return Err(SplicedBlockError::CountMismatch {
                chunks,
                relations: relations.len(),
            });
        }
        // No chunk is no relation, so no span root.
        let Some(chunk_count) = NonZeroUsize::new(chunks) else {
            return Err(SplicedBlockError::RootCount { roots: 0 });
        };
        let relations = relations
            .into_iter()
            .enumerate()
            .map(|(position, relation)| {
                let chunk = BlockChunk::at_position(position);
                if relation.index != chunk.get() {
                    return Err(SplicedBlockError::IndexOutOfOrder {
                        chunk,
                        index: relation.index,
                    });
                }
                let head = match NonZeroUsize::new(relation.head) {
                    None => BlockHead::SpanRoot,
                    Some(head) if head.get() <= chunks => BlockHead::Chunk(BlockChunk(head)),
                    Some(_) => {
                        return Err(SplicedBlockError::HeadOutOfBlock {
                            chunk,
                            head: relation.head,
                            chunks,
                        });
                    }
                };
                Ok(BlockRelation {
                    head,
                    relation: relation.relation,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            chunk_count,
            relations,
        })
    }

    fn into_rooted(self) -> Result<RootedBlockGraph, SplicedBlockError> {
        let Self {
            chunk_count,
            relations,
        } = self;
        let mut roots = relations
            .iter()
            .enumerate()
            .filter(|(_, relation)| relation.head == BlockHead::SpanRoot)
            .map(|(position, _)| BlockChunk::at_position(position));
        let Some(root) = roots.next() else {
            return Err(SplicedBlockError::RootCount { roots: 0 });
        };
        let additional_roots = roots.count();
        if additional_roots != 0 {
            return Err(SplicedBlockError::RootCount {
                roots: additional_roots + 1,
            });
        }
        #[derive(Clone, Copy)]
        enum Reachability {
            Unseen,
            Visiting,
            Rooted,
        }
        let mut states = vec![Reachability::Unseen; chunk_count.get()];
        let mut path = Vec::new();
        for position in 0..chunk_count.get() {
            let start = BlockChunk::at_position(position);
            let mut current = start;
            loop {
                match states[current.get() - 1] {
                    Reachability::Rooted => break,
                    Reachability::Visiting => {
                        return Err(SplicedBlockError::Cycle { chunk: start });
                    }
                    Reachability::Unseen => {
                        states[current.get() - 1] = Reachability::Visiting;
                        path.push(current);
                        match relations[current.get() - 1].head {
                            BlockHead::SpanRoot => break,
                            BlockHead::Chunk(next) => current = next,
                        }
                    }
                }
            }
            // Every node on this path reaches either the root or an already
            // rooted path. Each head is traversed once across all starts.
            for chunk in path.drain(..) {
                states[chunk.get() - 1] = Reachability::Rooted;
            }
        }
        Ok(RootedBlockGraph {
            chunk_count,
            root,
            relations,
        })
    }
}

/// The items a coordinated splice puts in place of a range, with one `%gra`
/// relation per chunk in block-relative form: a tree with exactly one root.
///
/// Built only by [`SplicedBlock::new`], which parses the raw relations once:
/// head `0` becomes the span root, every other head a [`BlockChunk`].
#[derive(Debug, Clone, PartialEq)]
pub struct SplicedBlock {
    /// The replacement items, in order.
    mors: Vec<Mor>,
    /// Immutable, rooted relations admitted together with the chunk count.
    graph: RootedBlockGraph,
}

impl SplicedBlock {
    /// Validate `mors` and `relations` as one block.
    ///
    /// `relations` holds one relation per chunk of `mors`, in chunk order,
    /// each with `index` its 1-based chunk within the block and `head` either
    /// `0` (the span root, exactly one) or a chunk of the block; every chain
    /// of heads reaches the span root. The span root's own label is not
    /// kept: the splice's `SpanRoot` supplies the relation it takes.
    pub fn new(
        mors: Vec<Mor>,
        relations: Vec<GrammaticalRelation>,
    ) -> Result<Self, SplicedBlockError> {
        let chunks: usize = mors.iter().map(Mor::count_chunks).sum();
        let graph = BoundedBlockGraph::new(relations, chunks)?.into_rooted()?;
        Ok(Self { mors, graph })
    }

    /// Chunks in the block (its items' chunks, summed).
    pub fn chunk_count(&self) -> usize {
        self.graph.chunk_count.get()
    }

    /// The block's unique root, in block-relative numbering.
    ///
    /// A caller can redirect host dependents to this chunk without inspecting
    /// or admitting the relations a second time. The root and the relations
    /// are constructed together and cannot be mutated independently.
    pub fn root_chunk(&self) -> BlockChunk {
        self.graph.root
    }

    /// Chunks in the block, which has at least one, for a renumbering.
    pub(super) fn chunks(&self) -> NonZeroUsize {
        self.graph.chunk_count
    }

    /// The replacement items.
    pub fn items(&self) -> &[Mor] {
        &self.mors
    }

    /// The relation of `chunk`, which the caller has taken from this block.
    pub(super) fn relation(&self, chunk: BlockChunk) -> &BlockRelation {
        &self.graph.relations[chunk.get() - 1]
    }

    /// The items and relations, for the splice to put in place.
    pub(super) fn into_parts(self) -> (Vec<Mor>, Vec<BlockRelation>) {
        (self.mors, self.graph.relations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::dependent_tier::mor::{MorStem, MorWord, PosCategory};

    /// `n` one-chunk items.
    fn items(n: usize) -> Vec<Mor> {
        (0..n)
            .map(|_| Mor::new(MorWord::new(PosCategory::new("x"), MorStem::new("w"))))
            .collect()
    }

    /// The relations `index|head|DEP` for `heads`, numbered from 1.
    fn relations(heads: &[usize]) -> Vec<GrammaticalRelation> {
        heads
            .iter()
            .enumerate()
            .map(|(position, head)| {
                GrammaticalRelation::new(position + 1, *head, "DEP".try_into().unwrap())
            })
            .collect()
    }

    /// A tree with one root is a block, its root parsed once.
    #[test]
    fn a_tree_with_one_root_is_a_block() {
        let block = SplicedBlock::new(items(3), relations(&[2, 0, 2])).expect("a tree");
        assert_eq!(block.chunk_count(), 3);
        assert_eq!(block.root_chunk().get(), 2);
        assert_eq!(
            block.relation(BlockChunk::FIRST).head,
            BlockHead::Chunk(BlockChunk::at_position(1))
        );
        assert_eq!(
            block.relation(BlockChunk::at_position(1)).head,
            BlockHead::SpanRoot
        );
    }

    /// Every shape that is not a one-rooted tree over the block's chunks is
    /// refused at the boundary, so the splice never sees one. The rootless
    /// pair (`1 -> 2`, `2 -> 1`) is the shape a morphotag reparse produced
    /// for `por@s favor@s`, which the splice used to write into the host.
    #[test]
    fn a_block_that_is_not_a_one_rooted_tree_is_refused() {
        let refused = |chunks: usize, heads: &[usize]| {
            SplicedBlock::new(items(chunks), relations(heads)).expect_err("refused")
        };
        assert_eq!(
            refused(2, &[2, 1]),
            SplicedBlockError::RootCount { roots: 0 }
        );
        assert_eq!(
            refused(2, &[0, 0]),
            SplicedBlockError::RootCount { roots: 2 }
        );
        assert_eq!(
            refused(3, &[2, 1, 0]),
            SplicedBlockError::Cycle {
                chunk: BlockChunk::FIRST
            }
        );
        assert_eq!(
            refused(2, &[0, 3]),
            SplicedBlockError::HeadOutOfBlock {
                chunk: BlockChunk::at_position(1),
                head: 3,
                chunks: 2,
            }
        );
        assert_eq!(
            refused(2, &[0]),
            SplicedBlockError::CountMismatch {
                chunks: 2,
                relations: 1,
            }
        );
        assert_eq!(refused(0, &[]), SplicedBlockError::RootCount { roots: 0 });
    }

    /// Heads are read by chunk position, so a relation whose index is not its
    /// position would have its heads misread: refused.
    #[test]
    fn a_relation_out_of_chunk_order_is_refused() {
        let mut out_of_order = relations(&[0, 1]);
        out_of_order[1].index = 5;
        assert_eq!(
            SplicedBlock::new(items(2), out_of_order).expect_err("refused"),
            SplicedBlockError::IndexOutOfOrder {
                chunk: BlockChunk::at_position(1),
                index: 5,
            }
        );
    }

    /// Independent bounded walks check all small raw graphs, including paths
    /// that converge, cycles with incoming tails and every root position.
    #[test]
    fn graph_admission_agrees_with_bounded_reachability() {
        for count in 1_usize..=5 {
            for encoding in 0..(count + 1).pow(count as u32) {
                let mut remaining = encoding;
                let heads = (0..count)
                    .map(|_| {
                        let head = remaining % (count + 1);
                        remaining /= count + 1;
                        head
                    })
                    .collect::<Vec<_>>();
                let roots = heads.iter().filter(|&&head| head == 0).count();
                let expected = if roots != 1 {
                    Err(SplicedBlockError::RootCount { roots })
                } else {
                    (1..=count)
                        .find(|&start| {
                            let mut head = start;
                            for _ in 0..count {
                                if head == 0 {
                                    break;
                                }
                                head = heads[head - 1];
                            }
                            head != 0
                        })
                        .map_or(Ok(()), |start| {
                            Err(SplicedBlockError::Cycle {
                                chunk: BlockChunk::at_position(start - 1),
                            })
                        })
                };
                assert_eq!(
                    SplicedBlock::new(items(count), relations(&heads)).map(|_| ()),
                    expected,
                    "{heads:?}"
                );
            }
        }
    }

    /// Long chains in either orientation avoid recursion; a separate root
    /// cannot conceal a disconnected cycle.
    #[test]
    fn long_chains_are_rooted_and_disconnected_cycles_are_refused() {
        let count = 25_000;
        let mut forward = (2..=count).chain([0]).collect::<Vec<_>>();
        let block = SplicedBlock::new(items(count), relations(&forward)).unwrap();
        assert_eq!(block.root_chunk().get(), count);
        let backward = (0..count).collect::<Vec<_>>();
        let block = SplicedBlock::new(items(count), relations(&backward)).unwrap();
        assert_eq!(block.root_chunk(), BlockChunk::FIRST);
        forward[count - 2] = 1;
        assert_eq!(
            SplicedBlock::new(items(count), relations(&forward)).unwrap_err(),
            SplicedBlockError::Cycle {
                chunk: BlockChunk::FIRST
            }
        );
    }
}
