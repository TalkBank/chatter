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

mod alignment_location_from_source;
mod alignment_units_from_source;
mod bracketed_contents_from_source;
mod ca_symbols_from_registry;
mod check_mapping_audit;
mod check_validity_parity;
mod closed_newtype_consumer_view;
mod config_path_check_parity;
mod conformance_inventory_current;
mod content_structure_public_view;
mod cross_utterance_from_source;
mod declared_speakers;
mod dependent_tier_recovery_codes_from_source;
mod dev_equivalence;
mod direct_parser_roundtrip_corpus;
mod empty_dependent_tier_from_source;
mod error_node_coverage;
mod error_words_validation;
mod file_alignment_from_source;
mod fragment_tier_api;
mod gates;
mod gem_headers_from_source;
mod generated;
mod generated_tests;
mod generated_traversal_conformance;
mod generated_traversal_current;
mod generated_traversal_parity;
mod golden_tiers_validation;
mod golden_words_parse;
mod golden_words_validation;
mod gra_alignment_from_source;
mod header_structure_from_source;
mod header_values_from_source;
mod headers_only_validation;
mod main_tier_methods_from_source;
mod mor_tier_from_source;
mod offset_tests;
mod overlap_groups_from_source;
mod overlap_regions_from_source;
mod parse_chat_file_terminates;
mod parse_error_corpus;
mod pho_alignment_from_source;
mod phon_xtier_acceptance;
mod pre_begin_headers_from_source;
mod property_tests;
mod public_error_types;
mod reference_corpus_parses;
mod retrace_diagnostics_from_source;
mod roundtrip_reference_corpus;
mod scoped_markers_from_source;
mod semantic_diff_from_source;
mod sin_alignment_from_source;
mod temporal_from_source;
mod terminator_presence_from_source;
mod text_tier_missing_content;
mod trailing_separator_from_source;
mod utterance_balance_from_source;
mod utterance_containment;
mod utterance_metadata_from_source;
mod validation_error_corpus;
mod visitor_slot_repeat_members;
mod walk_from_source;
mod warning_corpus;
mod wor_terminator_alignment;
mod wor_timing_from_source;
mod word_language_from_source;
mod word_structure_from_source;
mod word_validation_from_source;
