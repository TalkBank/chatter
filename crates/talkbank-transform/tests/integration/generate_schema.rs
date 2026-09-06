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

//! Regenerate the canonical CHAT JSON Schema from the Rust data model.
//!
//! The schema (`schema/chat-file.schema.json`) is auto-generated from
//! `talkbank-model` types via schemars and embedded into the binary at build
//! time (`talkbank_transform::SCHEMA_JSON`). When the model's `#[doc]` comments
//! or shape change, regenerate the committed file with:
//!
//! ```bash
//! just schema-gen
//! ```
//!
//! Ordinary tests check schema currency without rewriting the file.

#[path = "generate_schema/generate.rs"]
mod generate;
