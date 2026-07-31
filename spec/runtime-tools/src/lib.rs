//! Runtime-aware specification tooling.
//!
//! This crate contains post-generation tooling that needs the live Rust
//! parser/model crates. These tools are intentionally separate from
//! `spec/tools`, which should stay usable without pulling runtime parser/model
//! dependencies into ordinary spec generation workflows.
//!
//! Kept binaries:
//! - `extract_corpus_candidates`, find representative CHAT files for corpus curation
//! - `validate_error_specs`, validate error spec layer classification
//! - `gen_diagnostic_kind`, generate the `DiagnosticKind` registry
//!   (`crates/talkbank-model/src/errors/generated_diagnostic_kind.rs`) from
//!   each spec file's required `Kind` metadata field
//!
//! The bootstrap and mining machinery was removed (2026-03-22), the grammar
//! is stable and specs are now manually curated.

pub mod description;
