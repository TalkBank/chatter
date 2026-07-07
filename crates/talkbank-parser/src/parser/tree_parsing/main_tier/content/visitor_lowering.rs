//! Parser-side wrapper for the NEW-backend word/content typed extractors.
//!
//! This module provides `ContentLowering`, a lightweight struct whose methods
//! delegate to the NEW backend's free `extract_*` functions, so production
//! code can call them and receive `*Children` structs with `Positioned<..>`
//! members carrying `NodeSlot` variants (`Present` / `Missing` / `Error` /
//! `Unexpected` / `Absent`) at every child position.
//!
//! The helper `word_children` delegates to `extract_word_with_optional_annotations`
//! and returns the fully typed `WordWithOptionalAnnotationsChildren`, making
//! every child position explicit to callers.
//!
//! This wiring was missing from the production parser (which hand-walked
//! `node.kind()` string dispatch) until this module was added. It is still not
//! called from the production hot path (`main_tier/word/mod.rs` remains a
//! direct `node.kind()` hand-walk, see its own module doc): wiring the visitor
//! into that hot path is a separate, larger, PAUSED workstream (the OLD
//! `impl GrammarTraversal for ContentLowering<'_>` this migration replaces was
//! itself infrastructure landed ahead of that wiring), not part of the
//! chatter visitor-migration Task B3 (which only re-founds this module's
//! OLD-backend call sites on the NEW backend, preserving its current
//! reachability: production-unused, exercised only by this module's own test).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>

use crate::generated_traversal::{
    StandaloneWordChildren, StandaloneWordNode, WordWithOptionalAnnotationsChildren,
    WordWithOptionalAnnotationsNode, extract_standalone_word,
    extract_word_with_optional_annotations,
};
use tree_sitter::Node;

/// Holds the source text for future error-reporting callers.
///
/// The struct is intentionally minimal: no error sink is held here because
/// error emission begins once this wiring reaches the production hot path
/// (see the module doc), and adding an unused field now would violate YAGNI.
// dead_code: the struct and its methods are infrastructure not yet called
// from the production parser hot path (see the module doc). The allowance is
// removed once the first production call site is wired up.
#[allow(dead_code)]
pub(crate) struct ContentLowering<'a> {
    /// The full source text of the CHAT file being parsed.
    ///
    /// Reserved for future callers that need to extract node text from byte
    /// offsets; unused by the two typed-extraction pass-throughs below (they
    /// hand back typed CST wrappers, not decoded text).
    pub(crate) source: &'a str,
}

#[allow(dead_code)]
impl<'a> ContentLowering<'a> {
    /// Construct a `ContentLowering` for the given source text.
    pub(crate) fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// Return the typed children of a `word_with_optional_annotations` node.
    ///
    /// Delegates to the NEW backend's free `extract_word_with_optional_annotations`
    /// function, exposing (all `Positioned<..>`, read `.slot`):
    /// - `word`: `NodeSlot<StandaloneWordNode>` (required)
    /// - `child_1`: `Option<NodeSlot<WordWithOptionalAnnotationsChild1Children>>`
    ///   (the optional `[whitespace, replacement]` pair, NESTED because the
    ///   NEW backend models the interstitial whitespace explicitly; the OLD
    ///   backend's flat `replacement: Option<NodeSlot<ReplacementNode>>` field
    ///   is now reached by descending one level into this group)
    /// - `annotations`: `Option<NodeSlot<BaseAnnotationsNode>>` (optional)
    ///
    /// Callers read `NodeSlot` variants (`Present` / `Missing` / `Error` /
    /// `Unexpected` / `Absent`) to handle every child position explicitly,
    /// with no silent node dropping.
    pub(crate) fn word_children<'t>(
        &mut self,
        node: Node<'t>,
    ) -> WordWithOptionalAnnotationsChildren<'t> {
        extract_word_with_optional_annotations(WordWithOptionalAnnotationsNode(node))
    }

    /// Return the typed children of a `standalone_word` node.
    ///
    /// Delegates to the NEW backend's free `extract_standalone_word` function,
    /// exposing (all `Positioned<..>`, read `.slot`):
    /// - `child_0`: `Option<NodeSlot<StandaloneWordChild0Choice>>` (the leading
    ///   `word_prefix | zero` choice, NEWLY MATERIALIZED as an explicit
    ///   position by the NEW backend; the OLD backend consumed it internally
    ///   without exposing a carrier field)
    /// - `child_1`: `NodeSlot<WordBodyNode>` (word body, required)
    /// - `child_2`: `Option<NodeSlot<FormMarkerNode>>` (form marker, optional)
    /// - `child_3`: `Option<NodeSlot<WordLangSuffixNode>>` (language suffix, optional)
    /// - `child_4`: `Option<NodeSlot<PosTagNode>>` (part-of-speech tag, optional)
    ///
    /// The outer `Option` is `None` when the child is grammar-absent; the
    /// inner `NodeSlot` is `Present` and its downstream inner state, once
    /// `Some`.
    pub(crate) fn standalone_word_children<'t>(
        &mut self,
        node: Node<'t>,
    ) -> StandaloneWordChildren<'t> {
        extract_standalone_word(StandaloneWordNode(node))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::error::ErrorCollector;
    use crate::node_types::STANDALONE_WORD;

    /// Recursively find the first node with the given kind in the subtree.
    fn find_first_node_of_kind<'t>(
        node: tree_sitter::Node<'t>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'t>> {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_first_node_of_kind(child, kind) {
                return Some(found);
            }
        }
        None
    }

    impl TreeSitterParser {
        /// Test-only seam: parse a full CHAT fragment, locate the first
        /// `standalone_word` node using a depth-first search, run
        /// `extract_standalone_word` via `ContentLowering`, and apply `f`
        /// to the resulting children.
        ///
        /// Returns `None` when no `standalone_word` node is found in the
        /// tree (e.g., the input has no main-tier words).
        ///
        /// This method is gated on `#[cfg(test)]` and must not appear in
        /// production builds. It exists so focused unit tests can exercise
        /// `ContentLowering` without going through the full model-building
        /// pipeline.
        pub(crate) fn with_first_standalone_word<F, T>(
            &self,
            source: &str,
            _sink: &ErrorCollector,
            f: F,
        ) -> Option<T>
        where
            F: FnOnce(StandaloneWordChildren<'_>) -> T,
        {
            // Parse the source to a raw CST (no model building yet).
            let tree = self.parse_tree_incremental(source, None).ok()?;
            let root = tree.root_node();

            // Depth-first search for the first standalone_word node.
            // The actual path is:
            //   source_file -> full_document -> line -> utterance ->
            //   main_tier -> tier_body -> contents -> content_item ->
            //   base_content_item -> word_with_optional_annotations ->
            //   standalone_word
            // Using DFS avoids hard-coding every intermediate level.
            let sw_node = find_first_node_of_kind(root, STANDALONE_WORD)?;

            let mut lowering = ContentLowering::new(source);
            let children = lowering.standalone_word_children(sw_node);
            Some(f(children))
        }
    }

    #[test]
    fn visitor_exposes_form_marker_position_as_present() {
        // `hello@i` is the COMMON prefix-less word: an interjection with a
        // form marker (`@i`) and NO `word_prefix`. The grammar rule for
        // standalone_word is:
        //   optional(word_prefix | zero), word_body, optional(form_marker), ...
        // The leading slot is an OPTIONAL multi-type CHOICE (`word_prefix |
        // zero`), NEWLY MATERIALIZED by the NEW backend as an explicit
        // `child_0: Positioned<Option<NodeSlot<StandaloneWordChild0Choice>>>`
        // position (the OLD backend consumed it internally without exposing a
        // carrier field for it at all). The generated `extract_standalone_word`
        // kind-checks that slot before advancing: with no prefix present,
        // `child_0.slot` stays `None` and the cursor does NOT advance, so
        // `word_body` is correctly read at position 1 (Present) and
        // `form_marker` at position 2 (captured in `child_2`).
        //
        // This is the regression that proves the generator fix (chatter
        // visitor-migration Task B1's upstream fix, a105644): before the fix,
        // the extractor ate the word_body slot as the absent prefix slot, then
        // misclassified and lost the form marker, so a plain word reported no
        // word body and no form marker.
        let parser = TreeSitterParser::new().expect("grammar loads");
        let sink = ErrorCollector::new();
        let slot = parser.with_first_standalone_word(
            "@UTF8\n@Begin\n*CHI:\thello@i .\n@End\n",
            &sink,
            |sw| {
                // word_body is required: must be Present (not Absent/Unexpected).
                // form_marker is the optional child_2: must be captured (Some).
                (
                    sw.child_1.slot.present_or_recover().is_ok(),
                    sw.child_2.slot.is_some(),
                )
            },
        );
        assert_eq!(
            slot,
            Some((true, true)),
            "for a prefix-less word, word_body must be Present and form_marker captured"
        );
    }
}
