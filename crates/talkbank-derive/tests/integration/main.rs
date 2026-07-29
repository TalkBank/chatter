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

// Hoisted to the CRATE ROOT, which is what these tests need.
//
// The derive macros under test expand to `crate::model::...` paths. While
// each test file was its own crate root, its own `use` satisfied that; as
// modules of one binary they cannot, so the import belongs here.
use talkbank_model::model;

mod error_code_enum_tests;
mod semantic_eq_tests;
mod span_shift_tests;
mod ui_tests;
mod validation_tagged_tests;
