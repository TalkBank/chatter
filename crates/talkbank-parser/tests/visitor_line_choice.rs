// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Acceptance test for typed top-level `choice`-rule classification (Task 0b).
//!
//! The generated traversal types every qualifying top-level `choice` RULE as a
//! `<Rule>Choice` enum plus an `extract_<rule>` free function that classifies
//! the rule node's content into that enum (Option C). The migration's header
//! region (Task B2) needed this for `line`: its content is either a header (a
//! supertype, nested as `LineChoice::ActivitiesHeader(LineActivitiesHeaderChoice)`
//! over 34 concrete header subtype kinds), an `utterance`, a `blank_line`, or an
//! `unsupported_line`, and Task B2 replaced the hand-walked `line_child.kind()`
//! ladder with `extract_line(node).content`.
//!
//! This top-level boundary test drives the generated `extract_full_document` ->
//! `child_3` (the `repeat(line)` slot) on real reference data and asserts that
//! `extract_line(line_node).content` classifies a header line as
//! `NodeSlot::Present(LineChoice::ActivitiesHeader(_))` and an utterance line as
//! `NodeSlot::Present(LineChoice::Utterance(_))`. It also pins (at compile time)
//! the Option C coexistence: a dual-role symbol (`media_type`) keeps BOTH its
//! `MediaTypeNode` wrapper struct AND gains a `MediaTypeChoice` enum.

use talkbank_parser::generated_traversal::{
    FullDocumentNode, LineChoice, LineNode, MediaTypeChoice, MediaTypeNode, NodeSlot,
    extract_full_document, extract_line,
};

/// Compile-time proof that the dual-role rule `media_type` has BOTH the wrapper
/// struct `MediaTypeNode` (emitted because `media_type` also appears as a
/// concrete child position) AND the `MediaTypeChoice` enum (emitted because
/// `media_type` is a top-level choice rule). Had Option C reused the
/// `MediaTypeNode` name for the enum, the generated file would not compile
/// (`error[E0428]`); naming both types in this signature fails to build unless
/// both names exist and are distinct. The function is never called.
#[allow(dead_code)]
fn dual_role_names_coexist<'tree>(
    wrapper: MediaTypeNode<'tree>,
    choice: MediaTypeChoice<'tree>,
) -> (MediaTypeNode<'tree>, MediaTypeChoice<'tree>) {
    (wrapper, choice)
}

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

/// On a clean two-speaker reference document, `extract_line` must classify each
/// `line` node's content into the typed `LineChoice`: the `@Languages` /
/// `@Participants` / `@ID` / `@Comment` lines as `LineChoice::ActivitiesHeader`,
/// and the `*CHI` / `*MOT` lines as `LineChoice::Utterance`.
#[test]
fn extract_line_classifies_header_and_utterance() {
    let path = repo_root().join("corpus/reference/core/basic-conversation.cha");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let tree = parse_chat(&source);
    let full_doc = full_document(&tree);

    let doc_children = extract_full_document(FullDocumentNode(full_doc));

    // child_3 is the `repeat(line)` slot: `Vec<Positioned<NodeSlot<LineNode>>>`.
    let mut header_lines = 0usize;
    let mut utterance_lines = 0usize;
    let mut unexpected = Vec::new();

    for (i, elem) in doc_children.child_3.slot.iter().enumerate() {
        let line_node: LineNode = match &elem.slot {
            NodeSlot::Present(node) => *node,
            other => panic!("line repeat element {i} should be Present, got {other:?}"),
        };
        assert_eq!(
            line_node.0.kind(),
            "line",
            "repeat element {i} should be a `line` node"
        );

        // The Task-B2 target call: classify the line's content by TYPE.
        let line_children = extract_line(line_node);
        match &line_children.content.slot {
            NodeSlot::Present(LineChoice::ActivitiesHeader(_)) => header_lines += 1,
            NodeSlot::Present(LineChoice::Utterance(_)) => utterance_lines += 1,
            other => unexpected.push(format!("line {i}: {other:?}")),
        }
    }

    assert!(
        header_lines >= 1,
        "expected at least one header line classified as LineChoice::ActivitiesHeader, \
         got {header_lines}; unexpected slots: {unexpected:?}"
    );
    assert!(
        utterance_lines >= 1,
        "expected at least one utterance line classified as LineChoice::Utterance, \
         got {utterance_lines}; unexpected slots: {unexpected:?}"
    );
    assert!(
        unexpected.is_empty(),
        "every line in this clean reference file should classify as ActivitiesHeader or \
         Utterance, but some did not: {unexpected:?}"
    );
}
