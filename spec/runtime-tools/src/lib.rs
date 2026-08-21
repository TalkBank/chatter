//! Runtime-aware specification tooling.
//!
//! This crate contains post-generation tooling that needs the live Rust
//! parser/model crates. These tools are intentionally separate from
//! `spec/tools`, which should stay usable without pulling runtime parser/model
//! dependencies into ordinary spec generation workflows.
//!
//! `just --list` names this crate's binaries with the question each answers.
//! A list here would be a seventh copy: an earlier revision of this doc named
//! three of the five, and its replacement opened "all FIVE of them", which is a
//! count beside the list it counts.
//!
//! The one thing worth saying here, because it is about the CRATE and not the
//! list: everything in it needs the LIVE parser, model or `ErrorCode` enum, and
//! that is the whole reason for the split from `spec/tools`. `ca_census` is the
//! least obvious member and the easiest to overlook: a per-mark attestation
//! census for Conversation Analysis notation, the one region of CHAT chatter has
//! never specified, reading meaning only from the typed AST.
//!
//! The bootstrap and mining machinery was removed (2026-03-22), the grammar
//! is stable and specs are now manually curated.

pub mod artifacts;
pub mod description;
pub mod error_spec_validation;
pub mod observations;
