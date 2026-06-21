//! Regenerate the canonical CHAT JSON Schema from the Rust data model.
//!
//! The schema (`schema/chat-file.schema.json`) is auto-generated from
//! `talkbank-model` types via schemars and embedded into the binary at build
//! time (`talkbank_transform::SCHEMA_JSON`). When the model's `#[doc]` comments
//! or shape change, regenerate the committed file with:
//!
//! ```bash
//! cargo test --test generate_schema -- --nocapture
//! ```
//!
//! (In this virtual workspace `cargo test -p talkbank-transform --test
//! generate_schema` also works.)

#[path = "generate_schema/generate.rs"]
mod generate;
