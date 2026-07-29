//! Single integration-test binary for this crate.
//!
//! Every module here was previously its own `tests/*.rs`, and so its own
//! executable. Cargo links and launches each test target separately, so
//! that shape made both link time and process-launch cost scale with the
//! NUMBER OF TEST FILES rather than with the amount of testing. One binary
//! per crate keeps `cargo test` cheap however it is invoked, with no flags
//! to remember and no way to accidentally launch a hundred programs.
//!
//! Add a test file by dropping it in this directory and declaring it below.

mod cache_tests;
mod http_provider_tests;
