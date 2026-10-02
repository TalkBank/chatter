//! The refusals of a coordinated splice, each naming what it refused.
//!
//! Every variant is returned before either tier changes.

use super::BlockChunk;
// Named only by the variants' documentation.
#[cfg(doc)]
use super::{HostRedirects, ItemTarget, SpanRoot};
use crate::alignment::indices::SemanticWordIndex1;

/// Errors returned by coordinated Mor-Gra mutations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoordinatedMutationError {
    /// Replacement requires an ordered, nonempty item range.
    #[error("Replacement range {start}..{end} must be ordered and nonempty")]
    InvalidItemRange {
        /// Inclusive start of the requested range.
        start: usize,
        /// Exclusive end of the requested range.
        end: usize,
    },
    /// Requested item index is out of bounds.
    #[error("Item index {index} out of bounds (len={len})")]
    ItemIndexOutOfBounds {
        /// The requested 0-indexed item position.
        index: usize,
        /// Total number of items in the tier.
        len: usize,
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
    /// A host `%gra` relation whose index is not its chunk (relation `k` of
    /// the tier, from 1, must have index `k`): the splice reads every head
    /// as a chunk number, so on such a host it cannot say which word a head
    /// names.
    #[error("Host %gra relation at chunk {chunk} has index {index}")]
    HostIndexOutOfOrder {
        /// The chunk the relation stands at.
        chunk: SemanticWordIndex1,
        /// The index it carries.
        index: usize,
    },
    /// [`HostRedirects::ByItem`] needs one new item per replaced old item.
    #[error(
        "Host redirects by item need equal item counts, but {old_items} old item(s) \
         are replaced by {new_items} new item(s); state the redirects per item"
    )]
    RedirectItemCountsDiffer {
        /// Items in the replaced range.
        old_items: usize,
        /// Items in the new block.
        new_items: usize,
    },
    /// [`HostRedirects::PerItem`] must name one target per replaced old item.
    #[error("Host redirects name {targets} target(s) for {old_items} replaced item(s)")]
    RedirectCountMismatch {
        /// Targets supplied.
        targets: usize,
        /// Items in the replaced range.
        old_items: usize,
    },
    /// An [`ItemTarget::Chunk`] is not a chunk of the new block.
    #[error(
        "Host redirect for replaced item {item} targets chunk {target}, \
         but the new block has {new_chunks} chunk(s)"
    )]
    RedirectOutOfBlock {
        /// The replaced item's position within the range (0-based).
        item: usize,
        /// The block chunk it named.
        target: BlockChunk,
        /// Chunks in the new block.
        new_chunks: usize,
    },
    /// An [`ItemTarget::Counterpart`] for a replaced item that has none: the
    /// block has fewer items than its position.
    #[error(
        "Host redirect for replaced item {item} follows its counterpart, \
         but the new block has {new_items} item(s)"
    )]
    NoCounterpart {
        /// The replaced item's position within the range (0-based).
        item: usize,
        /// Items in the new block.
        new_items: usize,
    },
    /// A host relation depends on a replaced item whose counterpart has no
    /// unique head chunk (none, or several, of its chunks have a head outside
    /// the item), so where the dependent belongs cannot be read off the
    /// block. State an [`ItemTarget::Chunk`] for that item instead.
    #[error(
        "Host relation {dependent} depends on replaced item {item}, whose new item \
         has {head_chunks} chunk(s) headed outside it (exactly one is needed)"
    )]
    NoUniqueHeadChunk {
        /// The host relation's 1-based index.
        dependent: usize,
        /// The replaced item's position within the range (0-based).
        item: usize,
        /// How many chunks of the new item have a head outside it.
        head_chunks: usize,
    },
    /// [`SpanRoot::UtteranceRoot`] while a host relation outside the
    /// replaced range is already the utterance's root: the result would have
    /// two. Attach the span under a host chunk instead.
    #[error(
        "The span root names the utterance's root, but host chunk {host_root}, \
         outside the replaced range, is already the root"
    )]
    UtteranceRootTaken {
        /// The host chunk whose head is `0` (before the splice).
        host_root: SemanticWordIndex1,
    },
    /// [`SpanRoot::HostChunk`] names a chunk inside the replaced range,
    /// which the splice removes.
    #[error(
        "The span root names host chunk {chunk}, inside the replaced chunks \
         {first}..={last}"
    )]
    SpanRootInReplacedRange {
        /// The chunk named (before the splice).
        chunk: SemanticWordIndex1,
        /// The first replaced chunk.
        first: SemanticWordIndex1,
        /// The last replaced chunk.
        last: SemanticWordIndex1,
    },
    /// [`SpanRoot::HostChunk`] names a host chunk whose own chain of heads
    /// reaches the replaced range: the span would depend on a word that
    /// depends on the span, a cycle with no root.
    #[error(
        "The span root names host chunk {chunk}, which depends on replaced chunk \
         {through}: the span would depend on itself"
    )]
    SpanRootDependsOnSpan {
        /// The chunk named (before the splice).
        chunk: SemanticWordIndex1,
        /// The replaced chunk its chain of heads reaches.
        through: SemanticWordIndex1,
    },
    /// [`SpanRoot::HostChunk`] names a chunk past the host tier.
    #[error("The span root names host chunk {chunk}, but the host has {host_chunks} chunk(s)")]
    SpanRootOutOfHost {
        /// The chunk named (before the splice).
        chunk: SemanticWordIndex1,
        /// Relations in the host `%gra` tier before the splice.
        host_chunks: usize,
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
