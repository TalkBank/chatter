//! Allocation capacity derived from Tree-sitter child counts.

use tree_sitter::Node;

/// An allocation-capacity estimate expressed in Tree-sitter's `u32` count
/// domain.
///
/// Tree-sitter 0.27 deliberately uses `u32` for child counts and indices,
/// whereas Rust collections use `usize`. Keeping the estimate wrapped until
/// allocation prevents it from being confused with a child index and gives
/// the conversion one fail-safe boundary. On a target whose `usize` cannot
/// represent the estimate, parsing remains correct and merely skips the
/// reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChildCapacity(u32);

impl ChildCapacity {
    /// Use a node's complete child count as an upper-bound estimate.
    pub(crate) fn for_node(node: Node<'_>) -> Self {
        Self(node.child_count())
    }

    /// Wrap a caller-derived upper bound in Tree-sitter's count domain.
    pub(crate) const fn from_upper_bound(upper_bound: u32) -> Self {
        Self(upper_bound)
    }

    /// Create an empty vector, reserving the estimate when it fits `usize`.
    pub(crate) fn into_vec<T>(self) -> Vec<T> {
        let mut values = Vec::new();
        if let Ok(capacity) = usize::try_from(self.0) {
            // Capacity is only a performance hint. Allocation failure must not
            // turn otherwise recoverable parsing into a panic or abort.
            let _ = values.try_reserve_exact(capacity);
        }
        values
    }
}
