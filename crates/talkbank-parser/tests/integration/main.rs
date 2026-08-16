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

mod chat_parser_trait;
mod context_public_api;
mod debug_roundtrip;
mod dependent_tier_content_span;
mod document_entrypoint_characterization;
mod e203_invalid_form_marker_regression;
mod e207_filler_suggestion_test;
mod e208_empty_replacement_regression;
mod e245_stress_marker_regression;
mod e252_syllable_pause_at_word_start_regression;
mod error_messages_test;
mod gra_tier_characterization;
mod header_dispatch_characterization;
mod header_internals_characterization;
mod header_kind_dispatch_characterization;
mod header_separator_e758;
mod line_dispatch_characterization;
mod main_tier_structure_characterization;
mod mor_tier_characterization;
mod pho_mod_tier_characterization;
mod raw_user_tier_characterization;
mod recovery_tier_spans;
mod sin_tier_characterization;
mod test_debug_error_group;
mod test_parse_health_recovery;
mod text_tier_characterization;
mod typed_full_document_recovery_routing;
mod utterance_contents_characterization;
mod utterance_dispatch_characterization;
mod utterance_end_dispatch_characterization;
mod visitor_gem_header_internals;
mod visitor_gem_lazy_label;
mod visitor_hidden_rule_inlining;
mod visitor_line_choice;
mod visitor_repeat_rule_contents;
mod visitor_repeat_slotting;
mod visitor_simple_header_internals;
mod visitor_special_header_internals;
mod visitor_structured_id;
mod visitor_structured_languages;
mod visitor_structured_media;
mod visitor_structured_participants;
mod visitor_structured_situation_types;
mod visitor_supertype_classify;
mod wor_alignment_regression;
mod wor_tier_characterization;
