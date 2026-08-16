//! Shared helpers for this crate's integration-test binaries.
//!
//! Each file directly under `tests/` is its own crate, so anything two of them
//! need has to live here and be pulled in with `mod common;`.
//!
//! Root-finding is NOT defined here. `talkbank-parser-tests` already owns it
//! (`repo_paths::workspace_root()`), and that module exists precisely because
//! this workspace accumulated fifteen root-finders in three flavours that did
//! not agree, so a moved directory broke them silently and differently. This
//! module re-exports it rather than writing a sixteenth.

#![allow(dead_code)]

pub use talkbank_parser_tests::repo_paths::workspace_root;
