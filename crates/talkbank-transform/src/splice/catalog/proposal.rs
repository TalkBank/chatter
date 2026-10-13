//! Catalog-issued policy and edit shape, never independently writable facts.

use crate::splice::engine::SpliceEdit;

/// The catalog's reviewed policy for applying a proposal without per-site review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchSafety {
    /// One right answer, no semantic judgment.
    Mechanical,
    /// Deterministic, but requires explicit selection of its diagnostic code.
    Semantic,
    /// Several valid answers; never batch-applied.
    Ambiguous,
}

/// One named choice for a proposal requiring per-site review.
#[derive(Clone, Debug)]
pub struct NamedAlternative {
    /// Human-facing description of this choice.
    pub label: String,
    /// Candidate edits; a choice is not batch-selection or health admission.
    pub edits: Vec<SpliceEdit>,
}

/// A consuming view of a catalog proposal's policy and edit shape.
///
/// Raw parts can be used by intentional low-level transformations. Constructing
/// this view does not issue a [`CatalogFix`] or a selected application receipt.
#[derive(Clone, Debug)]
pub enum FixKind {
    /// A deterministic mechanical proposal.
    Mechanical(Vec<SpliceEdit>),
    /// A deterministic proposal requiring explicit code selection.
    Semantic(Vec<SpliceEdit>),
    /// Mutually exclusive choices; none is selected by the batch operation.
    Alternatives(Vec<NamedAlternative>),
}

/// An opaque proposal issued only by the catalog's producers.
///
/// Safety is derived from the closed variant, not stored alongside it. A
/// consuming raw view cannot be reclassified into another catalog proposal.
///
/// ```compile_fail
/// use talkbank_transform::splice::{CatalogFix, FixKind};
/// let forged = CatalogFix { kind: FixKind::Mechanical(Vec::new()) };
/// ```
#[derive(Clone, Debug)]
pub struct CatalogFix {
    kind: FixKind,
}

impl CatalogFix {
    pub(super) fn mechanical(edits: Vec<SpliceEdit>) -> Self {
        Self {
            kind: FixKind::Mechanical(edits),
        }
    }

    pub(super) fn semantic(edits: Vec<SpliceEdit>) -> Self {
        Self {
            kind: FixKind::Semantic(edits),
        }
    }

    pub(super) fn alternatives(choices: Vec<NamedAlternative>) -> Self {
        Self {
            kind: FixKind::Alternatives(choices),
        }
    }

    /// The reviewed policy derived from this proposal's variant.
    pub fn safety(&self) -> BatchSafety {
        match &self.kind {
            FixKind::Mechanical(_) => BatchSafety::Mechanical,
            FixKind::Semantic(_) => BatchSafety::Semantic,
            FixKind::Alternatives(_) => BatchSafety::Ambiguous,
        }
    }

    /// Consume the proposal into raw candidate parts, discarding batch authority.
    pub fn into_kind(self) -> FixKind {
        self.kind
    }
}
