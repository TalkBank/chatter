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

//! Regenerate the mechanical conformance inventory
//! (`tests/generated_traversal_conformance/inventory.rs`) from the committed
//! typed CST traversal and tree-sitter node-types.
//!
//! This is the committed replacement for the former uncommitted scratch script.
//! Run it after every typed-visitor regeneration:
//!
//! ```bash
//! cargo run -p talkbank-parser-tests --example gen_conformance_inventory
//! ```
//!
//! By default it rewrites `inventory.rs` in place. Pass `--stdout` to print the
//! generated source instead (e.g. to diff before overwriting). The staleness
//! guard `conformance_inventory_is_current` fails the suite if the committed
//! file drifts from a fresh run of this generator, so a forgotten regeneration
//! cannot slip through.

use std::error::Error;

use talkbank_parser_tests::conformance_inventory::{
    generate_inventory, inventory_path, node_types_json_path, typed_traversal_path,
};

fn main() -> Result<(), Box<dyn Error>> {
    let to_stdout = std::env::args().skip(1).any(|arg| arg == "--stdout");

    let typed_src = std::fs::read_to_string(typed_traversal_path())?;
    let node_types_json = std::fs::read_to_string(node_types_json_path())?;

    let inventory = generate_inventory(&typed_src, &node_types_json)?;

    if to_stdout {
        print!("{inventory}");
    } else {
        let path = inventory_path();
        std::fs::write(&path, inventory.as_bytes())?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}
