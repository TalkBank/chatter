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

//! Staleness guard for the mechanical conformance inventory.
//!
//! `src/conformance/inventory.rs` is derived, byte-for-
//! byte, from `crates/talkbank-parser/src/generated_traversal.rs` (the
//! generated typed CST traversal) and `grammar/src/node-types.json` by the
//! committed generator (`conformance_inventory::generate_inventory`, runnable as
//! the `gen_conformance_inventory` example).
//!
//! This test re-derives the inventory from the current inputs and asserts it
//! equals the committed copy. A future visitor regeneration that forgets to
//! rerun the generator therefore fails the suite here instead of silently
//! drifting (the failure mode the old, uncommitted scratch script left open:
//! the inventory's own header says "DO NOT HAND-EDIT", yet every regen used to
//! require a hand-edit).
//!
//! It calls the generator's core function directly and spawns nothing: the
//! generator writes its final layout itself, so the comparison depends only on
//! the two committed inputs, never on a formatter installed on the machine.

use talkbank_parser_tests::conformance_inventory::{
    ExtractArg, ExtractReturn, Inventory, InventoryGenError, generate_inventory, inventory_path,
    node_types_json_path, typed_traversal_path,
};

#[test]
fn concrete_carriers_do_not_include_generic_source_wrappers() {
    let source = r#"
        pub struct SourceChildren<'tree, 'source, C> {
            children: C,
            parent: SourceSlice<'tree, 'source>,
        }
        pub struct ExampleChildren_2<'tree> {
            pub item: KindSlot<'tree, ExampleNode<'tree>>,
            pub trailing_extras: Vec<Extra<'tree>>,
            pub unexpected: Vec<tree_sitter::Node<'tree>>,
        }
        pub struct LeafSpan<'tree> {
            pub node: tree_sitter::Node<'tree>,
            pub range: std::ops::Range<usize>,
        }
    "#;
    let inventory = Inventory::parse(source, "[]").expect("inventory source");
    let carrier = inventory.carrier("ExampleChildren_2").expect("a carrier");
    assert_eq!(carrier.fields(), ["item"]);
    assert!(inventory.carrier("SourceChildren").is_none());
    assert!(inventory.carrier("LeafSpan").is_none());
    assert!(
        inventory
            .render()
            .expect("observed layouts")
            .contains("impl_inspect_struct!(ExampleChildren_2 { item });")
    );
}

/// A shape is declared once, generic over the axes it has, each defaulted; the
/// inventory names it `Name<'tree>`, the reading the dispatch produces. The
/// `Admitted*` alias of its other reading is a `pub type`, not a second shape.
#[test]
fn shapes_generic_over_their_axes_are_inspected_once_by_their_tree_reading() {
    let source = r#"
        pub struct RangedChildren<'tree, R: RangePhase<'tree> = Raw> {
            pub item: KindSlot<'tree, ExampleNode<'tree>, R>,
            pub trailing_extras: Vec<Extra<'tree>>,
            pub unexpected: Vec<tree_sitter::Node<'tree>>,
        }
        pub struct NarrowedChildren<'tree, K: KindProof = Broad, R: RangePhase<'tree> = Raw> {
            pub item: NarrowedKindSlot<'tree, ExampleNode<'tree>, R, K>,
            pub trailing_extras: Vec<Extra<'tree>>,
            pub unexpected: Vec<tree_sitter::Node<'tree>>,
        }
        pub type AdmittedNarrowedChildren<'tree, R = Raw> = NarrowedChildren<'tree, KindAdmitted, R>;
        pub enum NarrowedChoice<'tree, K: KindProof = Broad, R: RangePhase<'tree> = Raw> {
            Example(NarrowedChildren<'tree, K, R>),
        }
        pub struct NarrowedMissing<K: KindProof, M>(pub K::Missing<M>);
    "#;
    let inventory = Inventory::parse(source, "[]").expect("inventory source");
    assert_eq!(
        inventory
            .carrier("RangedChildren")
            .expect("ranged")
            .fields(),
        ["item"]
    );
    assert_eq!(
        inventory
            .carrier("NarrowedChildren")
            .expect("narrowed")
            .fields(),
        ["item"]
    );
    assert!(inventory.carrier("AdmittedNarrowedChildren").is_none());
    let rendered = inventory.render().expect("observed layouts");
    assert!(rendered.contains("impl_inspect_choice!(NarrowedChoice { Example });"));
    assert!(!rendered.contains("Admitted"));
    assert!(!rendered.contains("NarrowedMissing"));
}

/// A carrier or choice whose generics `Name<'tree>` cannot name is refused,
/// never skipped: a skipped shape is a position nothing inspects.
#[test]
fn a_shape_with_an_unknown_generic_is_refused() {
    let carrier = r#"
        pub struct OddChildren<'tree, T> {
            pub item: T,
            pub trailing_extras: Vec<Extra<'tree>>,
            pub unexpected: Vec<tree_sitter::Node<'tree>>,
        }
    "#;
    assert!(matches!(
        Inventory::parse(carrier, "[]"),
        Err(InventoryGenError::UnadmittedShapeGenerics { name }) if name == "OddChildren"
    ));
    let lifeless = r#"
        pub struct LifelessChildren {
            pub item: ExampleNode,
            pub trailing_extras: Vec<Extra>,
            pub unexpected: Vec<Node>,
        }
    "#;
    assert!(matches!(
        Inventory::parse(lifeless, "[]"),
        Err(InventoryGenError::UnadmittedShapeGenerics { name }) if name == "LifelessChildren"
    ));
    let undefaulted = "pub enum OddChoice<'tree, R: RangePhase<'tree>> { A(ANode<'tree>) }";
    assert!(matches!(
        Inventory::parse(undefaulted, "[]"),
        Err(InventoryGenError::UnadmittedShapeGenerics { name }) if name == "OddChoice"
    ));
}

#[test]
fn fallible_extraction_cannot_silently_skip_conformance() {
    let source = r#"
        pub fn extract_example(node: ExampleNode<'_>) -> Result<ExampleChildren<'_>, ReconstructionFault> {}
        pub fn extract_choice(node: tree_sitter::Node<'_>) -> Choice<'_> {}
    "#;
    let inventory = Inventory::parse(
        source,
        r#"[{"type":"example","named":true},{"type":"choice","named":true}]"#,
    )
    .unwrap();
    let example = inventory.dispatch_arm("example").expect("example arm");
    assert_eq!(
        example.arg(),
        &ExtractArg::TypedWrapper("ExampleNode".to_owned())
    );
    assert_eq!(example.result(), ExtractReturn::Fallible);
    let choice = inventory.dispatch_arm("choice").expect("choice arm");
    assert_eq!(choice.arg(), &ExtractArg::BareNode);
    assert_eq!(choice.result(), ExtractReturn::Direct);
    let rendered = inventory.render().expect("observed layouts");
    assert!(rendered.contains("extract_example(classify::<ExampleNode>(node)).expect("));
    assert!(rendered.contains("extract_choice(node).inspect("));
}

#[test]
fn conformance_inventory_is_current() {
    let typed_src = std::fs::read_to_string(typed_traversal_path())
        .expect("generated_traversal.rs is readable");
    let node_types_json = std::fs::read_to_string(node_types_json_path())
        .expect("grammar/src/node-types.json is readable");

    let regenerated = generate_inventory(&typed_src, &node_types_json)
        .expect("regenerating the conformance inventory succeeds");

    let committed =
        std::fs::read_to_string(inventory_path()).expect("committed inventory.rs is readable");

    assert_eq!(
        regenerated, committed,
        "src/conformance/inventory.rs is STALE: it no longer matches a \
         fresh regeneration from generated_traversal.rs + node-types.json. Regenerate it \
         with `cargo run -p talkbank-parser-tests --example gen_conformance_inventory` (never \
         hand-edit the inventory)."
    );
}
