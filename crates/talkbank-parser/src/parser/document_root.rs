//! Where the document is in a parse tree, decided once.
//!
//! The CHAT grammar is multi-root: `tree.root_node()` is a `source_file`
//! wrapper whose first child is the `full_document`. When the document rule
//! fails to complete, a missing `@End` being the common case, tree-sitter emits
//! an ERROR in its place carrying the document's children in the order the rule
//! expects, which is exactly what the recovery-aware reconstruction consumes.
//!
//! Three call sites navigated to that node independently: the file parser, the
//! LSP's incremental reparse, and a test helper whose comment said it navigated
//! "exactly as the production entry point does", which is a prose assertion that
//! two copies agree. All three ended in `.unwrap_or(root)`, so "no document
//! here" and "the document IS the root" arrived downstream as the same value.

use crate::generated_traversal::{
    FromNodeKind, FullDocumentChildren, FullDocumentNode, extract_full_document,
    extract_full_document_from_error_recovery,
};
use tree_sitter::{Node, Tree};

/// What the document turned out to be.
///
/// Carrying the reconstructed children rather than a flag is what stops the
/// question being asked twice: `Recovered` exists BECAUSE the reconstruction
/// succeeded, so nothing downstream re-tests `is_error` to decide whether to
/// try it.
// `Recovered` is about 368 bytes against `Complete`'s 64, because it carries
// the whole reconstructed `FullDocumentChildren` carrier rather than a node.
// Boxing it is the lint's cure and is wrong here: this enum is built ONCE per
// file parse by `classify`, matched immediately at all three call sites, and
// never stored in a collection, so the box would buy an allocation and a
// pointer hop per parse to avoid moving 368 bytes once. Revisit if a caller
// ever holds many of these at a time.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum DocumentRoot<'tree> {
    /// A complete `full_document`.
    Complete {
        /// The node itself, for whole-tree questions.
        node: Node<'tree>,
        /// The document, classified.
        document: FullDocumentNode<'tree>,
    },
    /// The ERROR tree-sitter emitted IN PLACE of a `full_document`, with the
    /// document's children reconstructed from it.
    Recovered {
        /// The ERROR node, for whole-tree questions.
        node: Node<'tree>,
        /// The document's children, recovered from under the ERROR.
        children: FullDocumentChildren<'tree>,
    },
    /// Neither: a `source_file` with no document child, which is what a fragment
    /// or a file of some other shape parses to. There is nothing
    /// document-shaped, and matching this node's children against
    /// `full_document`'s shape would report findings about a shape it does not
    /// have.
    NotADocument {
        /// The root, for whole-tree questions.
        node: Node<'tree>,
    },
}

impl<'tree> DocumentRoot<'tree> {
    /// Locate and classify the document in `tree`.
    ///
    /// # It descends into `source_file`'s first child WHATEVER that child is
    ///
    /// The navigations this replaced descended only when the child was a
    /// `full_document`, and fell back to the `source_file` otherwise. That
    /// fallback is why the lowering used to walk a `source_file`'s children
    /// against `full_document`'s shape.
    ///
    /// So a file whose document rule fails at its very first header, no `@UTF8`,
    /// parses to `source_file(ERROR(..))` and is now `Recovered` where it was
    /// once the `source_file` itself. That CHANGES A DIAGNOSTIC, and the change
    /// is pinned by `a_document_that_fails_at_its_first_header_reports_once`:
    /// two E316s naming tree-sitter become one E316 saying the file structure is
    /// not valid CHAT and no lines could be recovered. The reconstruction
    /// recovers nothing either way, because the ERROR's children begin with
    /// something other than `utf8_header`, so no lines are lost or gained; only
    /// the report changes, from two node-span messages that name an internal
    /// tool to one whole-file message about the file.
    #[must_use]
    pub fn classify(tree: &'tree Tree) -> Self {
        let ts_root = tree.root_node();
        // `source_file` is the multi-root wrapper; the document is its first
        // child. A tree parsed at another entry point is its own root.
        let node = match ts_root.child(0) {
            Some(child) if ts_root.kind() == "source_file" => child,
            Some(_) | None => ts_root,
        };
        Self::of_node(node)
    }

    /// Classify a node already known to be where the document should be.
    #[must_use]
    fn of_node(node: Node<'tree>) -> Self {
        if let Some(document) = FullDocumentNode::from_node(node) {
            return Self::Complete { node, document };
        }
        match extract_full_document_from_error_recovery(node) {
            Some(children) => Self::Recovered { node, children },
            None => Self::NotADocument { node },
        }
    }

    /// The node every whole-tree question is asked of: emptiness, the recovery
    /// backstop, the child count.
    #[must_use]
    pub fn node(&self) -> Node<'tree> {
        match self {
            Self::Complete { node, .. }
            | Self::Recovered { node, .. }
            | Self::NotADocument { node } => *node,
        }
    }

    /// Whether the parser had to RECOVER at the document position.
    ///
    /// Replaces a separately-computed `root_node.is_error()`, which was the same
    /// fact derived a second way 35 lines from its use, and had to agree with
    /// whether the recovery reconstruction was attempted.
    #[must_use]
    pub fn recovered_at_root(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }

    /// The document's children to lower, or nothing when there is no document.
    ///
    /// `Complete` and `Recovered` are one answer here on purpose: a document
    /// reconstructed from the ERROR standing in for one has the same type and
    /// the same content as a complete one, and which it was is already on
    /// [`Self::recovered_at_root`].
    #[must_use]
    pub fn into_children(self) -> Option<FullDocumentChildren<'tree>> {
        match self {
            Self::Complete { document, .. } => Some(extract_full_document(document)),
            Self::Recovered { children, .. } => Some(children),
            Self::NotADocument { .. } => None,
        }
    }
}
