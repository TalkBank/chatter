// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! Acceptance test for HIDDEN-rule inlining in the generated traversal (Task 0e).
//!
//! `id_contents` is `seq(_id_identity_fields, _id_demographic_fields,
//! _id_role_fields)`, where all three referenced rules are HIDDEN (underscore)
//! SEQ rules that tree-sitter inlines: their content appears directly in the
//! `id_contents` CST node and they never materialize as nodes (so they are absent
//! from `node-types.json`, and the generator emits no `extract__id_*` function
//! for them at all -- confirmed empirically, see `generated_traversal_parity.rs`'s
//! Test 6 doc comment). The REAL children of an `id_contents` node are the flat
//! fields `id_languages`, `pipe`, `id_corpus`, ... `id_role`, ... `pipe`.
//!
//! Before this task (OLD backend, pre-regen) the traversal generator treated
//! each hidden reference as a single child slot, so `IdContentsChildren` had
//! three phantom group slots and `extract_id_contents` classified the real
//! first child (`id_languages`) against the kind `"_id_identity_fields"`,
//! which never matches, yielding `NodeSlot::Unexpected`. The fix inline-expands
//! the hidden rules, so `extract_id_contents` now walks the FLAT field
//! positions: `child_0` expects `id_languages`, `child_1` expects `pipe`, ...
//! (NEW backend: WIDER than the pre-fix positions since every interstitial
//! `optional($.whitespaces)` is also its own position; see
//! `header/id/parse.rs`'s field-mapping doc comment for the full table.)
//!
//! This top-level parser-API boundary test drives the generated visitor on a real
//! reference @ID line: `extract_full_document` -> a `repeat(line)` element ->
//! `extract_line` -> a `LineChoice::ActivitiesHeader` whose nested choice is
//! `LineActivitiesHeaderChoice::IdHeader` -> `extract_id_header(node).child_2`
//! (the `id_contents` node) -> `extract_id_contents(id_contents)`. It asserts
//! the first two production slots (`child_0` = `id_languages`, `child_1` = the
//! first `pipe`, both REQUIRED) are `NodeSlot::Present`.

use talkbank_parser::generated_traversal::{
    FullDocumentNode, IdContentsNode, LineActivitiesHeaderChoice, LineChoice, NodeSlot,
    extract_full_document, extract_id_contents, extract_id_header, extract_line,
};

/// Parse `source` into a tree-sitter tree using the CHAT grammar.
fn parse_chat(source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set CHAT language");
    parser.parse(source, None).expect("parse CHAT source")
}

/// The repo root, two levels up from this crate's manifest dir.
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root is two levels up from the crate manifest dir")
        .to_path_buf()
}

/// Locate the `full_document` node: the first child of the `source_file` root.
fn full_document(tree: &tree_sitter::Tree) -> tree_sitter::Node<'_> {
    let root = tree.root_node();
    assert_eq!(root.kind(), "source_file", "root should be source_file");
    let full_doc = root
        .child(0)
        .expect("source_file should have a full_document child");
    assert_eq!(
        full_doc.kind(),
        "full_document",
        "first source_file child should be full_document"
    );
    full_doc
}

/// On a real reference @ID line, `extract_id_contents` must classify the REAL flat
/// children as `Present` (the hidden `_id_*` groups are inlined). Asserts
/// `child_0` (`id_languages`) and `child_1` (first `pipe`) are `NodeSlot::Present`.
#[test]
fn extract_id_contents_classifies_real_flat_children_present() {
    // An existing reference fixture whose @ID line carries a full field list
    // (`eng|corpus|SPE|||||Child|||`), so id_languages / pipe / id_speaker /
    // id_role are all present.
    let path = repo_root().join("corpus/reference/languages/eng-conversation.cha");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let tree = parse_chat(&source);
    let full_doc = full_document(&tree);

    let doc_children = extract_full_document(FullDocumentNode(full_doc));

    // Find the first @ID line and extract its id_contents node.
    let mut id_contents_seen = 0usize;
    for elem in &doc_children.child_3.slot {
        let NodeSlot::Present(line_node) = &elem.slot else {
            continue;
        };

        // The line's content: only header lines are relevant. The header case
        // is the NESTED supertype choice
        // `LineChoice::ActivitiesHeader(LineActivitiesHeaderChoice)`, so among
        // header lines select the @ID headers directly by the nested choice's
        // OWN variant (no separate `classify_header` round-trip needed: the
        // nested choice already names the concrete header kind).
        let line_children = extract_line(*line_node);
        let id_header_node = match &line_children.content.slot {
            NodeSlot::Present(LineChoice::ActivitiesHeader(
                LineActivitiesHeaderChoice::IdHeader(node),
            )) => *node,
            _ => continue,
        };

        // `extract_id_header(node).child_2` is the `id_contents` node slot.
        let id_header_children = extract_id_header(id_header_node);
        let id_contents_node = id_header_children
            .child_2
            .slot
            .raw_node()
            .expect("an @ID header must have an id_contents node");

        // The Task-0e target call: walk the inlined flat field positions.
        let id_contents = extract_id_contents(IdContentsNode(id_contents_node));
        id_contents_seen += 1;

        // child_0 (id_languages) and child_1 (first pipe) are REQUIRED slots.
        // Pre-fix they classified the real children against the phantom kinds
        // "_id_identity_fields" / "_id_demographic_fields" and were
        // `Unexpected`; post-fix they expect "id_languages" / "pipe" and are
        // `Present`.
        assert!(
            matches!(id_contents.child_0.slot, NodeSlot::Present(_)),
            "id_contents child_0 (id_languages) must be Present after hidden-rule \
             inlining (was Unexpected against the phantom `_id_identity_fields` kind), \
             got {:?}",
            id_contents.child_0.slot
        );
        assert!(
            matches!(id_contents.child_1.slot, NodeSlot::Present(_)),
            "id_contents child_1 (first pipe) must be Present after hidden-rule \
             inlining (was Unexpected against the phantom `_id_demographic_fields` kind), \
             got {:?}",
            id_contents.child_1.slot
        );
    }

    assert!(
        id_contents_seen >= 1,
        "expected at least one @ID header with an id_contents node in the reference \
         document, found {id_contents_seen}"
    );
}
