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

//! Thin proxy, delegates to `cargo run -q -p xtask -- lint-docs-sync`.
//!
//! The actual docs sync checks live in `xtask/src/docs_sync.rs` to avoid
//! compiling a full integration test binary just for a structural lint.

use std::process::Command;

#[test]
fn docs_sync_passes() {
    let status = Command::new("cargo")
        .args(["run", "-q", "-p", "xtask", "--", "lint-docs-sync"])
        .status()
        .expect("failed to run cargo xtask lint-docs-sync");
    assert!(status.success(), "cargo xtask lint-docs-sync failed");
}
