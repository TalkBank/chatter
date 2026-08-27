// Test code is exempt from this crate's `deny`-level panic lints,
// see `docs/panic-audit/talkbank-model.md`.
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
//! Core TalkBank CHAT model plus validation/alignment APIs.
//!
//! `model` contains the strongly-typed AST/data structures, while `validation` and `alignment`
//! provide semantic checks and cross-tier consistency logic used by higher-level tools.
//!
//! # Start here
//!
//! Most downstream users will want one or more of these entry points:
//!
//! - [`ChatFile`], the root typed representation of a CHAT document
//! - [`ParseValidateOptions`], the standard parse/validate/alignment pipeline options
//! - [`WriteChat`], render typed structures back to CHAT text
//! - [`DependentTier`] and [`Utterance`], inspect the main tier and dependent tiers
//! - [`Validate`], run semantic validation on model values
//!
//! # Crate shape
//!
//! - [`model`] owns the typed CHAT AST and writer surface
//! - [`validation`] owns semantic checks and validation helpers
//! - [`alignment`] owns cross-tier alignment logic
//! - [`pipeline`] owns parse/validate option types shared with higher-level crates
//! - [`text_types`] owns typed raw/cleaned text wrappers used at public boundaries
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Examples
//!
//! ```
//! use talkbank_model::ChatFile;
//!
//! let file = ChatFile::new(vec![]);
//! assert_eq!(file.utterances().count(), 0);
//! ```

#![deny(missing_docs)]

// Self-alias so that proc macros generating `talkbank_model::SpanShift` resolve
// within this crate itself.
extern crate self as talkbank_model;

pub mod alignment;
pub mod chars;
pub mod errors;
pub mod generated;
pub mod indices;
pub mod model;
pub mod parser_api;
pub mod pipeline;
pub mod text_types;
pub mod validation;

pub use alignment::*;
pub use errors::*;
pub use indices::{UtteranceIdx, WordIdx};
pub use model::*;
pub use parser_api::*;
pub use pipeline::*;
pub use text_types::{ChatCleanedText, ChatRawText};
pub use validation::{GoverningMark, GoverningMarkKind, Validate, ValidationContext};

/// Compiles this crate's README so its examples cannot rot.
///
/// `#[cfg(doctest)]`, so the README is not duplicated into the rendered crate
/// docs; it exists only to make `cargo test --doc` typecheck every ```rust
/// block in it. Until 2026-08-27 no crate here did this and every README
/// example in the workspace was unchecked prose; see `talkbank-transform` for
/// the one that was already wrong.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
