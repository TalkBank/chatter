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

mod categorize_divergences;
mod corpus_lex_tests;
mod equivalence_tests;
mod error_parity;
mod extract_fixtures;
mod fixture_utils;
mod full_corpus_parse_test;
mod golden_parse;
mod lexer_tests;
mod media_whitespace_provenance;
mod model_study;
mod parser_fixtures;
mod quick_divergence_check;
mod snapshot_tests;
mod subcategorize_main_tier;
mod unmatched_bracket_tests;
