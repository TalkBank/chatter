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

//! Task 0d-B acceptance: a slot-level `repeat(seq)` MEMBER enumerates EVERY
//! period as a typed per-element `NodeSlot`, not just the first.
//!
//! `languages_contents = seq(language_code, repeat(seq(whitespaces?, comma,
//! whitespaces, language_code)))`: the FIRST code is `child_0`, and every
//! ADDITIONAL code is a `LanguagesContentsChild1Children` group enumerated in
//! the `child_1` repeat `Vec`. Before the 0d-B regen (OLD backend),
//! `LanguagesContentsChildren` had ONLY `child_0` (the repeat member was a
//! deferred single-skip); the OLD-backend port of this test (superseded here)
//! pinned the fix by asserting `child_1` compiles and enumerates.
//!
//! **NEW-backend field-index remap (verified against the real generated
//! struct, not assumed unchanged; the NEW backend does not `--skip
//! whitespaces`, so every interstitial `optional($.whitespaces)` is its own
//! position):** the repeat element `LanguagesContentsChild1Children` is
//! `{ child_0: Option<whitespaces>, child_1: comma, child_2: whitespaces,
//! child_3: language_code }`, NOT the OLD element's `{ child_0: comma,
//! child_1: language_code }`. The comma delimiter is now `child_1` and the
//! code is now `child_3`. Same values asserted, same construct pinned
//! (every additional code enumerates, every element's comma is Present);
//! only the field positions holding them shifted, per the B2 template
//! precedent (`docs` in `header/id/parse.rs` and the migration ledger).
//!
//! Driven on a real reference fixture (`content/linkers-multiple.cha`, whose
//! header is `@Languages:\teng, hrv, spa`) per the test-file policy (no ad hoc
//! `.cha` files).

use talkbank_parser_tests::generated_traversal::*;

/// Read the UTF-8 text of a leaf wrapper's raw node (the NEW backend's
/// wrappers carry no inherent `.text(source)` convenience method).
fn node_text<'s>(node: tree_sitter::Node, source: &'s str) -> &'s str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

/// Walk a tree-sitter tree, calling `callback` on every node.
fn walk_all<'tree, F>(node: tree_sitter::Node<'tree>, callback: &mut F)
where
    F: FnMut(tree_sitter::Node<'tree>),
{
    callback(node);
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_all(cursor.node(), callback);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn parse_chat(source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");
    parser.parse(source, None).expect("parse")
}

/// Resolve `corpus/reference` relative to this crate's manifest directory.
fn reference_file(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ dir")
        .parent()
        .expect("repo root")
        .join("corpus/reference")
        .join(rel)
}

#[test]
fn languages_contents_repeat_member_enumerates_every_code() {
    let path = reference_file("content/linkers-multiple.cha");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let tree = parse_chat(&source);
    let mut found = false;

    // Target the @Languages HEADER specifically. (The `@ID` first field reuses the
    // `languages_contents` rule for its single language, so a bare
    // `node.kind() == "languages_contents"` walk would also hit those single-code
    // nodes; we descend through the header to reach the real list.)
    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "languages_header" {
            let header = extract_languages_header(LanguagesHeaderNode(node));
            let NodeSlot::Present(contents) = &header.child_2.slot else {
                panic!("@Languages header must have a Present languages_contents child");
            };
            let children = extract_languages_contents(*contents);

            // The FIRST language code is the single `child_0` slot.
            let first = children
                .child_0
                .slot
                .present_or_recover()
                .ok()
                .map(|n| node_text(n.0, &source));
            assert_eq!(
                first,
                Some("eng"),
                "the first language code must be Present in child_0"
            );

            // The repeat member (`child_1`) enumerates the two ADDITIONAL codes as
            // typed `LanguagesContentsChild1Children` group elements. The pre-0d-B
            // OLD-backend generated struct had no `child_1` at all (it dropped every
            // code after the first); the NEW backend's group carries the code at
            // ITS OWN `child_3` (not `child_1`, per the field-index remap above).
            let codes: Vec<&str> = children
                .child_1
                .slot
                .iter()
                .filter_map(|element| match &element.slot {
                    NodeSlot::Present(group) => match &group.child_3.slot {
                        NodeSlot::Present(code_node) => Some(node_text(code_node.0, &source)),
                        _ => None,
                    },
                    _ => None,
                })
                .collect();
            assert_eq!(
                codes,
                vec!["hrv", "spa"],
                "every additional language code must enumerate as its own typed repeat element"
            );

            // Each element's `comma` delimiter (`child_1` of the group, NOT
            // `child_0` -- the NEW backend's `child_0` is the optional leading
            // whitespace) is Present too.
            assert!(
                children.child_1.slot.iter().all(|element| matches!(
                    &element.slot,
                    NodeSlot::Present(group) if matches!(group.child_1.slot, NodeSlot::Present(_))
                )),
                "each repeat element's comma delimiter must be Present"
            );

            found = true;
        }
    });

    assert!(
        found,
        "the reference fixture must contain a languages_header node"
    );
}
