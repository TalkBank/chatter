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

mod check_validity_parity;
mod closed_newtype_consumer_view;
mod config_path_check_parity;
mod conformance_inventory_current;
mod content_structure_public_view;
mod declared_speakers;
mod dev_equivalence;
mod direct_parser_roundtrip_corpus;
mod error_node_coverage;
mod error_words_validation;
mod gates;
mod generated;
mod generated_tests;
mod generated_traversal_conformance;
mod generated_traversal_current;
mod generated_traversal_parity;
mod golden_tiers_validation;
mod golden_words_parse;
mod golden_words_validation;
mod headers_only_validation;
mod offset_tests;
mod parse_chat_file_terminates;
mod parse_error_corpus;
mod phon_xtier_acceptance;
mod property_tests;
mod public_error_types;
mod reference_corpus_parses;
mod roundtrip_reference_corpus;
mod utterance_containment;
mod validation_error_corpus;
mod visitor_slot_repeat_members;
mod warning_corpus;
mod wor_terminator_alignment;
