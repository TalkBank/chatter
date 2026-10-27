// Test code is exempt from this crate's `deny`-level panic lints,
// see `docs/panic-audit/talkbank-parser-re2c.md`.
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
#![deny(missing_docs)]

//! re2c-based CHAT parser and lexer.
//!
//! This crate provides an independent public CHAT parser and a comparison
//! backend for the default tree-sitter parser. Neither backend is a correctness
//! oracle: differences require adjudication against the CHAT specification.
//! Valid-input meaning should agree; invalid-input diagnostics and recovery
//! may differ. Finite passing comparisons do not prove complete CHAT support.
//!
//! # Start here
//!
//! - [`Re2cParser`] implements the shared [`talkbank_model::ChatParser`] trait
//! - [`lex`] and [`lex_line`] expose the re2c lexer directly for debugging and
//!   narrow parser tests
//! - [`lexer`] exposes start-condition constants for targeted tier/body lexing
//!
//! Lower-level AST/conversion/test-helper modules remain reachable for this
//! crate's own tests and benchmarks, but they are intentionally hidden from the
//! public docs because they are backend implementation details rather than
//! first-wave supported entry points.
//!
//! # Example
//!
//! This demonstrates model production only. `NullErrorSink` discards diagnostics;
//! a returned model may contain recovery and is not proof of valid input.
//! Validity decisions must retain parse findings and run shared validation.
//!
//! ```rust
//! use talkbank_model::{ChatParser, NullErrorSink};
//! use talkbank_parser_re2c::Re2cParser;
//!
//! let parser = Re2cParser::new();
//! let outcome = parser.parse_chat_file("@UTF8\n@Begin\n*CHI:\thello .\n@End\n", 0, &NullErrorSink);
//! assert!(outcome.into_option().is_some());
//! ```

#[doc(hidden)]
pub mod ast;
#[doc(hidden)]
#[doc(hidden)]
pub mod chat_parser_impl;
#[doc(hidden)]
pub mod convert;
#[doc(hidden)]
pub mod parser;
pub mod source_text;
#[doc(hidden)]
pub mod token;

pub use chat_parser_impl::Re2cParser;

// Re-export talkbank-model for consumers.
pub use talkbank_model;

use token::LexResult;

/// Lex a line starting from a given condition. Returns `LexResult`
/// with tokens and error-checking utilities. Embedded NUL bytes are errors,
/// not terminators; lexing continues through the complete input.
///
/// Tokens and reports borrow the caller's source without copying it.
/// Arbitrary generated state numbers are not valid entry points:
/// ```compile_fail
/// let _ = talkbank_parser_re2c::lex_line("", usize::MAX);
/// ```
pub fn lex_line(input: &str, condition: lexer::StartCondition) -> LexResult<'_> {
    LexResult::lex(input, condition)
}

/// Lex a complete line from INITIAL condition.
pub fn lex(input: &str) -> LexResult<'_> {
    lex_line(input, lexer::COND_INITIAL)
}

/// Include generated lexer.
///
/// Only the lexer advances its source coordinates:
/// ```compile_fail
/// use talkbank_parser_re2c::lexer::{Lexer, COND_INITIAL};
/// let mut lexer = Lexer::new("abc", COND_INITIAL);
/// lexer.cursor = 1;
/// ```
/// Its source cannot be replaced after construction:
/// ```compile_fail
/// use talkbank_parser_re2c::lexer::{Lexer, COND_INITIAL};
/// let mut lexer = Lexer::new("abc", COND_INITIAL);
/// let _ = lexer.input;
/// ```
/// Internal DFA state cannot be injected:
/// ```compile_fail
/// use talkbank_parser_re2c::lexer::{Lexer, COND_INITIAL};
/// let mut lexer = Lexer::new("abc", COND_INITIAL);
/// lexer.condition = usize::MAX;
/// ```
pub mod lexer {
    // This DFA is generated explicitly with the pinned re2rust tool; build.rs
    // only copies the vendored artifact into OUT_DIR. The generator emits an
    // internal-error panic at a terminal state intended to be unreachable.
    // Keep the allowance around the generated include rather than hand-editing
    // generated sites. This permission is not a proof of lexer panic freedom.
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    mod input;
    use input::LexerInput;
    mod start;
    pub use start::StartCondition;

    // Generated lexer.rs will use crate::token::Token directly.
    const NONE: usize = usize::MAX;

    include!(concat!(env!("OUT_DIR"), "/lexer.rs"));

    // Re-export condition constants for start-state-based entry points.
    // Consumers can start the lexer in a specific condition to parse
    // isolated tiers (e.g., %mor body, %gra body, main tier content).
    /// Start condition for top-level line classification (`@`, `*`, `%`).
    pub const COND_INITIAL: StartCondition = StartCondition::Initial;
    /// Start condition for main-tier (`*SPK:`) content.
    pub const COND_MAIN_CONTENT: StartCondition = StartCondition::MainContent;
    /// Start condition for `%mor` tier content.
    pub const COND_MOR_CONTENT: StartCondition = StartCondition::MorContent;
    /// Start condition for `%gra` tier content.
    pub const COND_GRA_CONTENT: StartCondition = StartCondition::GraContent;
    /// Start condition for `%pho` tier content.
    pub const COND_PHO_CONTENT: StartCondition = StartCondition::PhoContent;
    /// Start condition for `%sin` tier content.
    pub const COND_SIN_CONTENT: StartCondition = StartCondition::SinContent;
    /// %wor uses MAIN_CONTENT, same word rules as main tier.
    pub const COND_WOR_CONTENT: StartCondition = StartCondition::MainContent;
    /// Start condition for generic dependent-tier text content.
    pub const COND_TIER_CONTENT: StartCondition = StartCondition::TierContent;
    /// %com: text_with_bullets_and_pics (adds inline_pic to TIER_CONTENT).
    pub const COND_COM_CONTENT: StartCondition = StartCondition::ComContent;
    /// User-defined tiers (%x*): text_with_bullets.
    pub const COND_USER_TIER_CONTENT: StartCondition = StartCondition::UserTierContent;
    /// Start condition for generic structured-header content.
    pub const COND_HEADER_CONTENT: StartCondition = StartCondition::HeaderContent;
    /// Start condition for `@ID` header content.
    pub const COND_ID_CONTENT: StartCondition = StartCondition::IdContent;
    /// Start condition for `@Types` header content.
    pub const COND_TYPES_CONTENT: StartCondition = StartCondition::TypesContent;
    /// Start condition for `@Languages` header content.
    pub const COND_LANGUAGES_CONTENT: StartCondition = StartCondition::LanguagesContent;
    /// Start condition for `@Participants` header content.
    pub const COND_PARTICIPANTS_CONTENT: StartCondition = StartCondition::ParticipantsContent;
    /// Start condition for `@Media` header content.
    pub const COND_MEDIA_CONTENT: StartCondition = StartCondition::MediaContent;
}

/// Build-time fingerprint of this crate's own complete source tree.
/// Includes parser recovery and conversion code; re2c also includes its
/// authored and vendored lexer. No sibling checkout is read at build time.
pub const SOURCE_FINGERPRINT: &str = env!("TALKBANK_PARSER_SOURCE_FINGERPRINT");

#[cfg(test)]
mod pause_provenance_tests;

#[cfg(test)]
mod header_fragment_tests;

#[cfg(test)]
mod fragment_range_tests;

#[cfg(test)]
mod header_recovery_tests;

#[cfg(test)]
mod word_language_tests;
