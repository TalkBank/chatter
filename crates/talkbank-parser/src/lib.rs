#![deny(missing_docs)]
// Test code is exempt from this crate's `deny`-level panic lints,
// see `docs/panic-audit/talkbank-parser.md`.
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]
//! Tree-sitter parser for TalkBank CHAT.
//!
//! Create a [`TreeSitterParser`] once, then reuse it for all parsing in that
//! scope. The parser handle owns an internal tree-sitter buffer that is reused
//! across calls, creating a new parser per call wastes that allocation.
//!
//! **Do not create a parser per file or per word.** Create one at the top of
//! your entry point (CLI main, server request handler, test function) and pass
//! `&TreeSitterParser` to everything that needs parsing.
//!
//! # Start here
//!
//! - [`TreeSitterParser`] is the main entry point for full-file parsing
//! - [`parse_dependent_tier`] is the narrow helper for dependent-tier-only parsing
//! - [`tiers`] exposes tier-focused parser APIs without requiring callers to dig
//!   through the internal parser implementation tree
//!
//! If you want the standard parse-then-validate pipeline rather than raw parsing,
//! use `talkbank-transform`'s `parse_and_validate*` helpers on top of this crate.
//!
//! # Example
//!
//! ```rust
//! use talkbank_parser::TreeSitterParser;
//!
//! let parser = TreeSitterParser::new().expect("grammar loads");
//!
//! // Reuse the same parser for multiple files:
//! let file1 = parser.parse_chat_file("@UTF8\n@Begin\n*CHI:\thello .\n@End\n")
//!     .expect("valid CHAT");
//! let file2 = parser.parse_chat_file("@UTF8\n@Begin\n*MOT:\thi .\n@End\n")
//!     .expect("valid CHAT");
//! ```
//!
//! # Thread Safety
//!
//! `TreeSitterParser` uses `RefCell` internally and is `!Send + !Sync`.
//! For multi-threaded work, create one parser per thread.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub(crate) mod error {
    pub use talkbank_model::*;
}

pub(crate) mod model {
    pub use talkbank_model::model::*;
}

#[cfg(test)]
pub(crate) mod validation {
    pub use talkbank_model::validation::*;
}

/// Auto-generated, exhaustive typed CST traversal for the CHAT grammar.
///
/// Produced by the self-contained `tree-sitter-grammar-utils` backend (the
/// `generate_typed_traversal` example): free `extract_*` functions, a closed
/// five-state `NodeSlot`, uniform per-rule `<Rule>Children` carriers, and a typed
/// `unexpected` sink. This is the single generated visitor the whole production
/// parser is driven by: every parser region dispatches CST structure through
/// these functions. (The former hand-walk `node.kind()` dispatch and the OLD
/// `GrammarTraversal` trait visitor were retired in the 2026-07 migration; this
/// module is the canonical successor, renamed from its transitional
/// `generated_traversal_typed` name once it became the sole visitor.)
///
/// Regenerate from a checkout of the `tree-sitter-grammar-utils` repo. There is
/// NO `--skip` flag (the backend models grammar extras explicitly); the wrapper
/// runs `rustfmt` on its own output, so this single command produces the
/// canonical, fmt-clean file directly with no separate `cargo fmt` step:
///
/// ```sh
/// cargo run --example generate_typed_traversal -p tree-sitter-node-types -- \
///   <CHATTER>/grammar/src/grammar.json \
///   <CHATTER>/grammar/src/node-types.json \
///   --edition 2024 \
///   --toolchain 1.96.1 \
///   > <CHATTER>/crates/talkbank-parser/src/generated_traversal.rs
/// ```
///
/// Never hand-edit the generated file (no dash-stripping, no adding allows): if
/// the output is wrong, fix the generator in `tree-sitter-grammar-utils` as a
/// GENERAL change and regenerate. The `generated_traversal_is_current` staleness
/// guard recomputes the embedded digests from the committed grammar JSON, so a
/// forgotten regeneration fails the test suite. The two suppressions below
/// (`missing_docs`, because the crate is `#![deny(missing_docs)]` and the
/// generated `pub` items carry no `///` docs; and
/// `rustdoc::broken_intra_doc_links`) are structural to any generated traversal
/// module registered in this crate.
#[allow(missing_docs, rustdoc::broken_intra_doc_links)]
pub mod generated_traversal;

/// Node type string constants from tree-sitter-talkbank grammar.
pub mod node_types;

/// Token parsing for coarsened grammar tokens (language codes, annotations, etc.).
pub mod tokens;

/// Public API modules (tier parsing).
pub mod api;
/// Internal parser implementation modules.
pub(crate) mod parser;

/// Main parser type and initialization error.
pub use parser::{ParserInitError, TreeSitterParser};
pub use talkbank_model::FragmentSemanticContext;

/// Convenience re-exports for dependent-tier parsing APIs.
pub use api::{dependent_tier::parse_dependent_tier, tiers};
