//! Whole-tier recovery admission: the one gate between a dependent tier's
//! source-bound node and its typed parser.
//!
//! tree-sitter sets `has_error()` on a node when the node itself or any
//! descendant is an ERROR or a MISSING placeholder: in the runtime,
//! `ts_node_has_error` is `ts_subtree_error_cost(subtree) > 0`, a missing
//! subtree carries a nonzero cost, and a parent's cost sums its children's
//! (tree-sitter 0.27, `src/node.c`, `src/subtree.h`, `src/subtree.c`). A
//! dependent tier is therefore in exactly one of two states, and
//! [`TierAdmission::admit`] is total over them:
//!
//! - [`TierRecovery`]: the tier carries a recovery node somewhere. The
//!   dispatcher reports every ERROR and MISSING node in it with the
//!   whole-tier recovery walk and substitutes the tier's recovery
//!   disposition (an empty placeholder for `%mor`, a dropped tier for
//!   `%pho`, ...). No typed tier parser runs on it.
//! - [`HealthyTier`]: no descendant is ERROR or MISSING. This is the only
//!   value a typed tier parser accepts, so inside such a parser a slot in
//!   the `Missing` or `Error` state contradicts the admission and is a
//!   producer fault ([`CstFailure::RecoveryInsideHealthyTier`]), never a
//!   CHAT diagnostic.
//!
//! The proof is in the signature; the slot types inside the parsers still
//! admit `Missing` and `Error`, so their arms remain and report the
//! contradiction. Making those states uninhabited under the proof is a
//! generator change (a `HealthyTier`-narrowed extraction), recorded with the
//! coverage drive.

use crate::error::ErrorSink;
use crate::generated_traversal::{AsRawNode, SourceBound, SourceBoundKind, SourceSlice};
use crate::parser::typed_cst::{CstFailure, report_cst_failure};
use tree_sitter::Node;

/// Source-owned proof that a tier needs recovery, consumed by reporting.
#[must_use]
pub struct TierRecovery<'tree, 'source>(SourceSlice<'tree, 'source>);

impl<'tree, 'source> TierRecovery<'tree, 'source> {
    /// The tier's own source slice, for the whole-tier recovery walk.
    pub(crate) fn slice(self) -> SourceSlice<'tree, 'source> {
        self.0
    }
}

/// Source-owned proof that a tier carries no ERROR or MISSING node anywhere
/// beneath it. Built only by [`TierAdmission::admit`].
#[derive(Debug, Clone, Copy)]
pub struct HealthyTier<'tree, 'source, T: SourceBoundKind<'tree>>(SourceBound<'tree, 'source, T>);

impl<'tree, 'source, T: SourceBoundKind<'tree>> HealthyTier<'tree, 'source, T> {
    /// The admitted node. Typed parsers that still take a bare source-bound
    /// node (every tier but `%mor`, as of 2026-10-19) read it here; the
    /// proof is in having been handed a `HealthyTier` at all. Note that
    /// `SourceBound` is `Copy`, so the raw node a caller admitted stays in
    /// its scope too; only the parsers that take the proof close that.
    pub fn node(self) -> SourceBound<'tree, 'source, T> {
        self.0
    }
}

/// Which of the two states a dependent tier is in. Exhaustive: a caller
/// that forgets the recovery branch does not compile.
pub enum TierAdmission<'tree, 'source, T: SourceBoundKind<'tree>> {
    /// The tier carries a recovery node; report it, never parse it.
    Recovery(TierRecovery<'tree, 'source>),
    /// The tier is recovery-free; its typed parser may run.
    Healthy(HealthyTier<'tree, 'source, T>),
}

impl<'tree, 'source, T: SourceBoundKind<'tree>> TierAdmission<'tree, 'source, T> {
    /// Decide a tier's state from tree-sitter's own `has_error` flag.
    pub fn admit(tier: SourceBound<'tree, 'source, T>) -> Self {
        if tier.raw_node().has_error() {
            Self::Recovery(TierRecovery(tier.source_slice()))
        } else {
            Self::Healthy(HealthyTier(tier))
        }
    }
}

/// Report a `Missing` or `Error` slot met inside a tier admitted as
/// [`HealthyTier`]: the walk's reading of the tree contradicts the flag
/// tree-sitter set on it, which is a producer fault and not a CHAT
/// invalidity. `rule` names the grammar rule whose shape was being matched.
pub(crate) fn recovery_inside_healthy_tier(
    node: Node<'_>,
    source: &str,
    rule: &'static str,
    errors: &impl ErrorSink,
) {
    report_cst_failure(
        node,
        source,
        CstFailure::RecoveryInsideHealthyTier { rule },
        errors,
    );
}
