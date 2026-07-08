// Unit-test modules: panic-family clippy lints relaxed by policy
// (see the workspace [lints] table for the production deny).
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented,
    )
)]

fn main() {}
