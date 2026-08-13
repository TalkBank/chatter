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

// Shared harness, declared ONCE for the whole binary. Modules below
// reach it as `crate::common::...`.
mod common;

mod adjudication_tests;
mod appledouble_tests;
mod batch_tests;
mod cache_env_tests;
mod cache_tests;
mod command_execution_tests;
mod command_matrix_tests;
mod command_surface_manifest;
mod dependent_tier_continuation_tests;
mod docs_sync;
mod fix_tests;
mod force_refresh_scale_tests;
mod gra_relation_vocabulary_tests;
mod holistic_judgment_cli;
mod holistic_pipeline_batch_cli;
mod integration_tests;
mod join_retrace_tests;
mod long_tier_no_crash;
mod long_tier_robustness_tests;
mod lossless_parse_tests;
mod media_filename_match_tests;
mod merge_experimental_help_tests;
mod merge_tests;
mod nested_word_validation_tests;
mod parse_error_surfacing_tests;
mod phon_xtier_validation_tests;
mod pipeline_tests;
mod prefix_marker_position_tests;
mod rediarize_tests;
mod retrace_marker_order_tests;
mod sanity_scan_tests;
mod speaker_id_tests;
mod stack_limit_tests;
mod stateful_cli_integration;
mod update_command_tests;
mod utterance_initial_annotation_tests;
