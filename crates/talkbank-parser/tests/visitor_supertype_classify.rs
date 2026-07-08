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

//! Acceptance test for typed supertype classification (Task 0c).
//!
//! The generated traversal types every node-types.json SUPERTYPE as a
//! `<Supertype>Choice` enum (one variant per concrete subtype kind, typed leaf
//! wrapper payload) plus a self-classifying free `extract_<supertype>`
//! function that discriminates a concrete subtype node into that enum. This is
//! the supertype-descent capability Task 0b deferred ("do NOT generate a
//! HeaderChoice enum-of-subtypes -- a later task decides that"); Task 0c is
//! that task.
//!
//! The migration's header region (Task B2) replaced `parse_header_node`'s
//! `node.kind()` ladder with a typed dispatch: a `line`'s content classifies
//! (via `extract_line`) into the NESTED supertype choice
//! `LineChoice::ActivitiesHeader(LineActivitiesHeaderChoice)`, and that
//! concrete header node is then classified by the free, self-classifying
//! `extract_header` into the typed `HeaderChoice` (this task), with no banned
//! `node.kind()` match in the parser.
//!
//! This top-level boundary test drives the generated `extract_full_document` ->
//! `child_3` (the `repeat(line)` slot) on a real reference file, takes each
//! header line's concrete node via `extract_line(..).content`, and asserts
//! `extract_header(header_node).content` returns
//! `NodeSlot::Present(HeaderChoice::_)` for every header line, with the
//! `@Languages` line classifying specifically as `HeaderChoice::LanguagesHeader`.

use talkbank_parser::generated_traversal::{
    AsRawNode, FullDocumentNode, HeaderChoice, LineChoice, NodeSlot, extract_full_document,
    extract_header, extract_line,
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

/// On a clean two-speaker reference document, `extract_header` must classify
/// each header line's concrete header node into the typed `HeaderChoice`: every
/// header line yields `NodeSlot::Present(HeaderChoice::_)`, and the `@Languages`
/// line yields the specific `HeaderChoice::LanguagesHeader` variant.
#[test]
fn classify_header_types_concrete_header_nodes() {
    let path = repo_root().join("corpus/reference/core/basic-conversation.cha");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let tree = parse_chat(&source);
    let full_doc = full_document(&tree);

    let doc_children = extract_full_document(FullDocumentNode(full_doc));

    // child_3 is the `repeat(line)` slot: `Vec<Positioned<NodeSlot<LineNode>>>`.
    let mut header_lines = 0usize;
    let mut languages_header_seen = false;
    let mut non_present = Vec::new();

    for (i, elem) in doc_children.child_3.slot.iter().enumerate() {
        let line_node = match &elem.slot {
            NodeSlot::Present(node) => *node,
            other => panic!("line repeat element {i} should be Present, got {other:?}"),
        };

        // Classify the line's content by TYPE (Task 0b). Only header lines are
        // relevant here; utterance / blank / unsupported lines are skipped. The
        // header case is the NESTED supertype choice
        // `LineChoice::ActivitiesHeader(LineActivitiesHeaderChoice)`, not a bare
        // `LineChoice::Header(node)`; reach the concrete raw node via `raw_node()`.
        let line_children = extract_line(line_node);
        let header_node = match &line_children.content.slot {
            NodeSlot::Present(LineChoice::ActivitiesHeader(inner)) => inner.raw_node(),
            _ => continue,
        };
        header_lines += 1;

        // The Task-2b/0c target call: classify the concrete header node by TYPE,
        // via the free, self-classifying `extract_header`.
        match extract_header(header_node).content.slot {
            NodeSlot::Present(HeaderChoice::LanguagesHeader(_)) => languages_header_seen = true,
            NodeSlot::Present(_) => {} // some other concrete header kind: still typed.
            other => non_present.push(format!("line {i}: {other:?}")),
        }
    }

    assert!(
        header_lines >= 1,
        "expected at least one header line in the reference document, got {header_lines}"
    );
    assert!(
        non_present.is_empty(),
        "every header line's concrete node should classify as \
         NodeSlot::Present(HeaderChoice::_), but some did not: {non_present:?}"
    );
    assert!(
        languages_header_seen,
        "the @Languages line should classify as HeaderChoice::LanguagesHeader"
    );
}
