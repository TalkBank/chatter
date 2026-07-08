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

//! Staleness guard for the mechanical conformance inventory.
//!
//! `tests/generated_traversal_conformance/inventory.rs` is derived, byte-for-
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
//! It calls the generator's core function directly (only `rustfmt` is spawned,
//! the same normalization the committed file was produced with), so it does not
//! shell out to the example binary.

use talkbank_parser_tests::conformance_inventory::{
    generate_inventory, inventory_path, node_types_json_path, typed_traversal_path,
};

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
        "tests/generated_traversal_conformance/inventory.rs is STALE: it no longer matches a \
         fresh regeneration from generated_traversal.rs + node-types.json. Regenerate it \
         with `cargo run -p talkbank-parser-tests --example gen_conformance_inventory` (never \
         hand-edit the inventory)."
    );
}
