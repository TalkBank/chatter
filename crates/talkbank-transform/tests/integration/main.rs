//! Single integration-test binary for this crate.
//!
//! Every module here was previously its own `tests/*.rs`, and so its own
//! executable. Cargo links and launches each test target separately, so
//! that shape made both link time and process-launch cost scale with the
//! NUMBER OF TEST FILES rather than with the amount of testing. One binary
//! per crate keeps `cargo test` cheap no matter how it is invoked, with no
//! flags to remember and no way to accidentally launch a hundred programs.
//!
//! Add a test file by dropping it in this directory and declaring it below.

mod adjudication_tests;
mod book_library_usage_examples;
mod cache_key_properties;
// The three suites below exercise the validation runner / result cache and
// only compile with the default-on `validation-runner` feature.
#[cfg(feature = "validation-runner")]
mod cache_tests;
#[cfg(feature = "validation-runner")]
mod concurrent_tests;
mod e552_message_quality;
mod generate_schema;
mod json_roundtrip_edges;
mod json_tests;
mod num_words;
mod pipeline_tests;
mod render_parity;
mod sanitize_tests;
mod speaker_id_tests;
mod splice_catalog_tests;
mod transcript_merge_tests;
#[cfg(feature = "validation-runner")]
mod validation_runner_tests;
