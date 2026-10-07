//! The identity of one node in one parse tree.
//!
//! Tree-sitter numbers every node with `Node::id`, unique among the nodes of a
//! tree while that tree lives. Kept as a bare `usize`, that number shared a type
//! with every index, count and byte offset in the parser, so nothing stopped a
//! tier's identity from being compared with a position or a length. As
//! [`CstNodeId`] it can only be read off a node, and only compared with another
//! identity.

use tree_sitter::Node;

/// Which node of the producing parse tree a value came from.
///
/// Built only by [`CstNodeId::of`], from the node itself, and never converted
/// back to a number: the one operation is equality (and hashing, for a set of
/// excluded nodes). Meaningful only within the tree that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CstNodeId(usize);

impl CstNodeId {
    /// The identity of `node` in its tree.
    pub(crate) fn of(node: Node<'_>) -> Self {
        Self(node.id())
    }
}
