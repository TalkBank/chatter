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

//! Acceptance test for `repeat(symbol)` enumeration in the generated visitor.
//!
//! The generated traversal types every top-level `repeat(...)` production as
//! `Vec<Positioned<NodeSlot<T>>>`, so the `full_document` production
//!
//! ```text
//! seq( utf8_header,
//!      repeat(pre_begin_header),   // child_1: Vec<Positioned<NodeSlot<..>>>
//!      begin_header,               // child_2
//!      repeat(line),               // child_3: Vec<Positioned<NodeSlot<LineNode>>>
//!      end_header )                // child_4
//! ```
//!
//! exposes each repeat region as a `Vec` of typed slots, so the line body is
//! visible and `begin_header` / `end_header` classify correctly.
//!
//! This is the top-level boundary test for the repeat-enumeration capability:
//! it drives the generated `extract_full_document` on real reference data and
//! asserts (1) the clean case enumerates the line repeat with every element
//! `Present`, with the anchors classified correctly, and (2) an ERROR child in
//! the line region is captured as `NodeSlot::Error` (recovery nodes
//! enumerated, not skipped; the NEW backend's recovery-aware repeat split
//! absorbs a mid-repeat ERROR as a repeat element, per Task B1).

use talkbank_parser::generated_traversal::{FullDocumentNode, NodeSlot, extract_full_document};

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

/// Locate the `full_document` node: it is the first child of the `source_file`
/// root for a complete document.
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

/// Clean multi-line reference document: the `repeat(line)` slot must be a
/// non-empty `Vec` of `Present` `line` nodes, and both header anchors present.
#[test]
fn full_document_enumerates_clean_line_repeat() {
    let path = repo_root().join("corpus/reference/edge-cases/postcodes-and-gems.cha");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let tree = parse_chat(&source);
    let full_doc = full_document(&tree);

    let children = extract_full_document(FullDocumentNode(full_doc));

    // begin_header (production index 2) and end_header (index 4) anchors must
    // classify as Present: before the fix the swallowed repeat misaligned them.
    assert!(
        matches!(children.child_2.slot, NodeSlot::Present(_)),
        "begin_header anchor should be Present, got {:?}",
        children.child_2.slot
    );
    assert!(
        matches!(children.child_4.slot, NodeSlot::Present(_)),
        "end_header anchor should be Present, got {:?}",
        children.child_4.slot
    );

    // repeat(line) (production index 3) must be a non-empty Vec whose elements
    // are all Present `line` nodes.
    assert!(
        !children.child_3.slot.is_empty(),
        "repeat(line) slot should be a non-empty Vec, got empty"
    );
    for (i, elem) in children.child_3.slot.iter().enumerate() {
        match &elem.slot {
            NodeSlot::Present(node) => {
                assert_eq!(
                    node.0.kind(),
                    "line",
                    "line repeat element {i} should be a `line` node"
                );
            }
            other => panic!("line repeat element {i} should be Present, got {other:?}"),
        }
    }
}

/// A document with a deliberately unparseable top-level line: tree-sitter emits
/// an ERROR node among the `line` children, and the generated repeat slot must
/// capture it as `NodeSlot::Error` (the whole point: recovery nodes enumerated,
/// not skipped).
#[test]
fn full_document_repeat_captures_error_line() {
    let path = repo_root().join("tests/error_corpus/parse_errors/E345_unmatched_scoped_begin.cha");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let tree = parse_chat(&source);
    let full_doc = full_document(&tree);

    let children = extract_full_document(FullDocumentNode(full_doc));

    // The line repeat must contain at least one ERROR slot (the unparsable
    // `*CHI:\thello <world .` line) AND at least one Present `line` slot (the
    // surrounding clean lines), proving the loop enumerates both rather than
    // skipping the whole region.
    let error_count = children
        .child_3
        .slot
        .iter()
        .filter(|s| matches!(s.slot, NodeSlot::Error(_)))
        .count();
    let present_lines = children
        .child_3
        .slot
        .iter()
        .filter(|s| matches!(&s.slot, NodeSlot::Present(n) if n.0.kind() == "line"))
        .count();

    assert!(
        error_count >= 1,
        "the unparsable line must surface as a NodeSlot::Error in the repeat, \
         got {error_count} error slots in {:?}",
        children.child_3.slot
    );
    assert!(
        present_lines >= 1,
        "the clean surrounding lines must still be Present `line` slots, \
         got {present_lines} present lines"
    );
}
