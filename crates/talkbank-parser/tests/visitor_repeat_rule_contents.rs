//! Acceptance test for typed TOP-LEVEL repeat-RULE enumeration (Task 0d-A).
//!
//! The generated traversal types every top-level `repeat`/`repeat1` RULE. The
//! load-bearing one for the parser migration is `contents`
//! (`repeat1(choice(whitespaces, content_item, separator, overlap_point))`,
//! the main-tier body): `extract_contents` enumerates EVERY child to the end
//! into a required first element (`child_0`, `ContentsChild0Choice`) plus a
//! repeated tail (`child_1`, `Vec<Positioned<NodeSlot<ContentsChild1Choice>>>`),
//! classifying each as a typed element (`Whitespaces` / `ContentItem` /
//! `Separator` / `OverlapPoint`), with no child ever silently dropped. Task B3
//! (main-tier structure) consumes this (see `tree_parsing/main_tier/
//! structure/contents.rs`).
//!
//! This top-level boundary test drives the generated visitor from the document
//! root down to a real `contents` node, entirely through typed extraction (no
//! `node.kind()` hand-walk): `extract_full_document` -> the `repeat(line)` slot
//! -> `extract_line` (an `Utterance`) -> `extract_utterance` -> `extract_main_tier`
//! -> `extract_tier_body` -> the `contents` node -> `extract_contents`, then
//! asserts the content items enumerate as `NodeSlot::Present(..)` on real
//! reference data (and that a clean file yields no `Unexpected` / `Error`
//! item).

use talkbank_parser::generated_traversal::{
    AsRawNode, ContentsChild0Choice, ContentsChild1Choice, ContentsNode, FullDocumentNode,
    LineChoice, MainTierNode, NodeSlot, TierBodyNode, extract_contents, extract_full_document,
    extract_line, extract_main_tier, extract_tier_body, extract_utterance,
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

/// Return the raw node a `Present` slot wraps, or panic with context. (Test-only;
/// the clean reference file makes every descent slot `Present`.)
fn present_raw<'tree, T: AsRawNode<'tree> + std::fmt::Debug>(
    slot: &NodeSlot<'tree, T>,
    what: &str,
) -> tree_sitter::Node<'tree> {
    match slot {
        NodeSlot::Present(node) => node.raw_node(),
        other => panic!("{what} should be Present on this clean file, got {other:?}"),
    }
}

/// On a clean reference document, `extract_contents` (the top-level `contents`
/// repeat RULE) must enumerate each main-tier content child as a typed
/// element, with at least one `Present` content item and no `Unexpected` /
/// `Error` element (the file is clean), reached entirely through typed
/// extraction.
#[test]
fn extract_contents_enumerates_main_tier_items() {
    let path = repo_root().join("corpus/reference/core/basic-conversation.cha");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let tree = parse_chat(&source);
    let full_doc = full_document(&tree);

    let doc_children = extract_full_document(FullDocumentNode(full_doc));

    let mut present_items = 0usize;
    let mut content_item_variants = 0usize;
    let mut problem_items: Vec<String> = Vec::new();
    let mut contents_nodes_seen = 0usize;

    // child_3 is the `repeat(line)` slot: `Vec<Positioned<NodeSlot<LineNode>>>`.
    for elem in &doc_children.child_3.slot {
        let NodeSlot::Present(line_node) = &elem.slot else {
            continue;
        };

        // Only utterance lines carry a main tier (and thus a `contents` node).
        let line_children = extract_line(*line_node);
        let NodeSlot::Present(LineChoice::Utterance(utterance_node)) = line_children.content.slot
        else {
            continue;
        };

        // utterance -> main_tier (child_0) -> tier_body (child_4) -> contents (content_2).
        let utt = extract_utterance(utterance_node);
        let main_tier_node = present_raw(&utt.child_0.slot, "utterance.main_tier");
        let main_tier = extract_main_tier(MainTierNode(main_tier_node));
        let tier_body_node = present_raw(&main_tier.child_4.slot, "main_tier.tier_body");
        let tier_body = extract_tier_body(TierBodyNode(tier_body_node));
        let contents_node = present_raw(&tier_body.content_2.slot, "tier_body.contents");
        assert_eq!(
            contents_node.kind(),
            "contents",
            "the typed descent must land on a `contents` node"
        );
        contents_nodes_seen += 1;

        // The Task-B3 target call: enumerate the main-tier content items by TYPE.
        // `contents` splits into a required first element (`child_0`,
        // `ContentsChild0Choice`) plus a repeated tail (`child_1`,
        // `ContentsChild1Choice`): structurally identical 4-way choices
        // (`whitespaces` / `content_item` / `separator` / `overlap_point`) the
        // generator mangles into two separately-named types because `contents =
        // repeat1(..)` splits into a required-first-plus-repeated-tail shape.
        let contents_children = extract_contents(ContentsNode(contents_node));
        match &contents_children.child_0.slot {
            NodeSlot::Present(choice) => {
                present_items += 1;
                if matches!(choice, ContentsChild0Choice::ContentItem(_)) {
                    content_item_variants += 1;
                }
            }
            NodeSlot::Missing(_) | NodeSlot::Absent => {}
            NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
                problem_items.push(format!(
                    "contents child_0: {:?}",
                    contents_children.child_0.slot
                ));
            }
        }
        for (i, item) in contents_children.child_1.slot.iter().enumerate() {
            match &item.slot {
                NodeSlot::Present(choice) => {
                    present_items += 1;
                    if matches!(choice, ContentsChild1Choice::ContentItem(_)) {
                        content_item_variants += 1;
                    }
                }
                NodeSlot::Missing(_) | NodeSlot::Absent => {}
                NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
                    problem_items.push(format!("contents item {i}: {:?}", item.slot));
                }
            }
        }
    }

    assert!(
        contents_nodes_seen >= 1,
        "expected at least one utterance line with a `contents` node in the reference file"
    );
    assert!(
        present_items >= 1,
        "expected at least one Present content item enumerated by extract_contents, got {present_items}"
    );
    assert!(
        content_item_variants >= 1,
        "expected at least one ContentItem element, got {content_item_variants}"
    );
    assert!(
        problem_items.is_empty(),
        "a clean reference file's contents must enumerate with no Error/Unexpected items, but got: {problem_items:?}"
    );
}
