//! The numberings a coordinated splice moves between, each its own type, and
//! the one translation from the host's numbering before the splice to its
//! numbering after it.
//!
//! Five spaces meet in a splice, every one a `usize` underneath:
//!
//! - [`PreChunk`]: a host chunk numbered as BEFORE the splice, the numbering
//!   the caller read the host in and every host head is written in;
//! - [`PostChunk`]: a host chunk numbered as AFTER the splice, the numbering
//!   the result is written in;
//! - [`BlockChunk`]: a chunk of the new block, numbered within the block;
//! - [`ReplacedChunk`]: a chunk of the replaced range, 0-based within it,
//!   which is how a redirect is looked up;
//! - [`ReplacedItem`]: an item of the replaced range, 0-based within it,
//!   which names its counterpart in the block.
//!
//! ```mermaid
//! flowchart LR
//!     raw["host %gra relations"] -->|"AdmittedHost::admit<br/>(index is its chunk)"| pre["PreChunk"]
//!     named["SpanRoot::HostChunk"] -->|"PreChunk::named"| pre
//!     pre -->|"Geometry::locate"| place{"Place"}
//!     place -->|"Kept"| kept["KeptChunk"]
//!     place -->|"Replaced"| replaced["ReplacedChunk"]
//!     kept -->|"Geometry::translate"| post["PostChunk"]
//!     replaced -->|"ChunkRedirects::target"| block["BlockChunk"]
//!     block -->|"Geometry::place"| post
//!     post -->|"PostChunk::get"| written["written relation"]
//! ```
//!
//! [`Geometry::translate`] is the only route from a pre-splice chunk to a
//! post-splice one, and it takes only a [`KeptChunk`], which only
//! [`Geometry::locate`] makes: a replaced chunk has no post-splice number and
//! cannot be given one. [`Geometry::place`] is the only route from a block
//! chunk. A [`PostChunk`] is never built from a number, so no pre-splice index
//! can be written into the post-splice tier. A [`PreChunk`] is made only from
//! what is already in the pre-splice numbering: a position of the admitted
//! host, a head the host wrote ([`PreHead::of`]), a chunk the caller named
//! ([`PreChunk::named`]), or the range's own bounds, for a refusal to name.
//! [`ReplacedChunk::position`] stays a bare `usize` because it indexes the
//! redirect table built in the range's order; only [`Geometry::locate`] makes
//! one, from the same range, so it is in bounds.

use std::num::NonZeroUsize;

use super::CoordinatedMutationError;
use super::block::BlockChunk;
use crate::alignment::indices::{GraHeadRef, SemanticWordIndex1};
use crate::model::dependent_tier::gra::GrammaticalRelation;
use crate::model::dependent_tier::mor::item::Mor;

/// A host chunk numbered as before the splice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreChunk(SemanticWordIndex1);

impl PreChunk {
    /// The host chunk a caller named: the public API numbers host chunks as
    /// before the splice, the numbering the caller read them in.
    pub(super) fn named(chunk: SemanticWordIndex1) -> Self {
        Self(chunk)
    }

    /// The chunk as the public API, and its refusals, name it.
    pub(super) fn as_semantic(self) -> SemanticWordIndex1 {
        self.0
    }
}

/// What a host relation depends on, in the pre-splice numbering.
#[derive(Debug, Clone, Copy)]
pub(super) enum PreHead {
    /// The utterance's root (`0`).
    Root,
    /// A host chunk, which may lie past the host (a host defect the splice
    /// keeps, translated like any other chunk after the range).
    Chunk(PreChunk),
}

impl PreHead {
    /// The head of `relation`.
    pub(super) fn of(relation: &GrammaticalRelation) -> Self {
        match relation.head_ref() {
            GraHeadRef::Root => Self::Root,
            GraHeadRef::Word(chunk) => Self::Chunk(PreChunk(chunk)),
        }
    }
}

/// The host's `%gra` relations, admitted: relation `k` of the tier (from 1)
/// has index `k`, so a head names a chunk in the numbering the relations
/// stand in, and a relation's chunk is its position. Built only by
/// [`Self::admit`].
pub(super) struct AdmittedHost<'a> {
    relations: &'a [GrammaticalRelation],
}

impl<'a> AdmittedHost<'a> {
    /// Admit `relations`, refusing the first whose index is not its chunk
    /// ([`CoordinatedMutationError::HostIndexOutOfOrder`]).
    pub(super) fn admit(
        relations: &'a [GrammaticalRelation],
    ) -> Result<Self, CoordinatedMutationError> {
        match numbered(relations).find(|(chunk, relation)| relation.index != chunk.0.as_usize()) {
            Some((chunk, relation)) => Err(CoordinatedMutationError::HostIndexOutOfOrder {
                chunk: chunk.0,
                index: relation.index,
            }),
            None => Ok(Self { relations }),
        }
    }

    /// Every relation with its chunk, in order.
    pub(super) fn chunks(&self) -> impl Iterator<Item = (PreChunk, &'a GrammaticalRelation)> {
        numbered(self.relations)
    }

    /// The relation of `chunk`, or `None` past the host.
    pub(super) fn relation(&self, chunk: PreChunk) -> Option<&'a GrammaticalRelation> {
        // A chunk is 1-based, so its position is one less and cannot wrap.
        self.relations.get(chunk.0.as_usize() - 1)
    }

    /// How many chunks (relations) the host has.
    pub(super) fn chunk_count(&self) -> usize {
        self.relations.len()
    }
}

/// `relations` with the chunk each stands at: the one place a pre-splice
/// chunk is made from a position.
fn numbered(
    relations: &[GrammaticalRelation],
) -> impl Iterator<Item = (PreChunk, &GrammaticalRelation)> {
    relations.iter().enumerate().map(|(position, relation)| {
        let chunk = SemanticWordIndex1::from_nonzero(NonZeroUsize::MIN.saturating_add(position));
        (PreChunk(chunk), relation)
    })
}

/// A host chunk outside the replaced range, as [`Geometry::locate`] found
/// it: the only thing [`Geometry::translate`] takes.
#[derive(Debug, Clone, Copy)]
pub(super) struct KeptChunk(PreChunk);

/// A chunk of the replaced range, 0-based within it.
#[derive(Debug, Clone, Copy)]
pub(super) struct ReplacedChunk(usize);

impl ReplacedChunk {
    /// Its position within the range, for the per-chunk redirect table
    /// built in the same order.
    pub(super) fn position(self) -> usize {
        self.0
    }

    /// Whether it is the range's first chunk, where the block goes.
    pub(super) fn is_first(self) -> bool {
        self.0 == 0
    }
}

/// Where a pre-splice host chunk lies relative to the replaced range.
#[derive(Debug, Clone, Copy)]
pub(super) enum Place {
    /// Before or after the range: the splice keeps it, renumbered.
    Kept(KeptChunk),
    /// Inside the range: the splice removes it.
    Replaced(ReplacedChunk),
}

/// A host chunk numbered as after the splice. Made only by
/// [`Geometry::translate`] and [`Geometry::place`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PostChunk(NonZeroUsize);

impl PostChunk {
    /// The number written into the result.
    pub(super) fn get(self) -> usize {
        self.0.get()
    }
}

/// What a relation of the result depends on, in the post-splice numbering.
#[derive(Debug, Clone, Copy)]
pub(super) enum PostHead {
    /// The utterance's root, written `0`.
    Root,
    /// A host chunk after the splice.
    Chunk(PostChunk),
}

impl PostHead {
    /// The head as `%gra` writes it.
    pub(super) fn written(self) -> usize {
        match self {
            Self::Root => 0,
            Self::Chunk(chunk) => chunk.get(),
        }
    }
}

/// Where the replaced range sits in the host and how many chunks replace
/// it: everything a renumbering needs.
#[derive(Debug, Clone, Copy)]
pub(super) struct Geometry {
    /// Host chunks before the range.
    before: usize,
    /// Chunks in the range; every item has at least one.
    old: NonZeroUsize,
    /// Chunks of the block that replaces them; a block has a root chunk.
    new: NonZeroUsize,
}

impl Geometry {
    /// The range of `old` chunks after the host's first `before`, replaced
    /// by `new` chunks.
    pub(super) fn new(before: usize, old: NonZeroUsize, new: NonZeroUsize) -> Self {
        Self { before, old, new }
    }

    /// Where `chunk` lies: kept (before or after the range), or replaced.
    pub(super) fn locate(self, chunk: PreChunk) -> Place {
        // How far past the range's first chunk it is; `None` before it.
        match chunk.0.as_usize().checked_sub(self.before + 1) {
            Some(offset) if offset < self.old.get() => Place::Replaced(ReplacedChunk(offset)),
            _ => Place::Kept(KeptChunk(chunk)),
        }
    }

    /// THE translation: a kept host chunk's post-splice number. A chunk
    /// before the range keeps its number; one after it moves by the change
    /// in the range's size.
    pub(super) fn translate(self, kept: KeptChunk) -> PostChunk {
        let chunk = kept.0.0.as_usize();
        match chunk <= self.before {
            true => PostChunk(NonZeroUsize::MIN.saturating_add(chunk - 1)),
            // After the range, `chunk > before + old`, so `chunk - old` is
            // at least `before + 1` and cannot wrap.
            false => PostChunk(self.new.saturating_add(chunk - self.old.get())),
        }
    }

    /// A block chunk's post-splice host number.
    pub(super) fn place(self, chunk: BlockChunk) -> PostChunk {
        PostChunk(chunk.as_nonzero().saturating_add(self.before))
    }

    /// The range's first chunk, as a refusal names it.
    pub(super) fn first_replaced(self) -> PreChunk {
        PreChunk(SemanticWordIndex1::from_nonzero(
            NonZeroUsize::MIN.saturating_add(self.before),
        ))
    }

    /// The range's last chunk, as a refusal names it.
    pub(super) fn last_replaced(self) -> PreChunk {
        PreChunk(SemanticWordIndex1::from_nonzero(
            self.old.saturating_add(self.before),
        ))
    }
}

/// An item of the replaced range, 0-based within it: its counterpart is the
/// block item at the same position.
#[derive(Debug, Clone, Copy)]
pub(super) struct ReplacedItem(usize);

impl ReplacedItem {
    /// `items` with the replaced item each stands for, in order.
    pub(super) fn numbered<T>(items: impl Iterator<Item = T>) -> impl Iterator<Item = (Self, T)> {
        items
            .enumerate()
            .map(|(position, item)| (Self(position), item))
    }

    /// Its counterpart among the block's items, if the block has one there.
    pub(super) fn counterpart(self, block_items: &[Mor]) -> Option<&Mor> {
        block_items.get(self.0)
    }

    /// Its position, as a refusal names it.
    pub(super) fn get(self) -> usize {
        self.0
    }
}
