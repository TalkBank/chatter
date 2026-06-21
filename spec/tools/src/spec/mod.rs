//! # Specification Parsing
//!
//! Parsers and types for loading CHAT specification Markdown files into
//! structured Rust types.
//!
//! The `spec/constructs/` directory holds valid-input specs (parsed by
//! [`markdown`] into [`MarkdownExample`] / [`MarkdownCategory`], then converted
//! to [`ConstructSpec`]). The `spec/errors/` directory holds invalid-input specs
//! (parsed by [`error`] into [`ErrorSpec`]). Generators in [`crate::output`]
//! consume these types to emit tree-sitter corpus tests, Rust validation tests,
//! and error documentation.

pub mod construct;
pub mod error;
pub mod error_corpus;
pub mod markdown;
pub mod validation_manifest;

pub use construct::ConstructSpec;
pub use error::ErrorSpec;
pub use error_corpus::ErrorCorpusSpec;
pub use markdown::{MarkdownCategory, MarkdownExample, WrapperStrategy};
