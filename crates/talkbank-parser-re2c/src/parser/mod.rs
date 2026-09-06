//! CHAT parser, re2c lexer + chumsky parser combinators.
//!
//! Entry points in `entry_points.rs`. Chumsky parsers in `main_tier.rs`,
//! `dependent_tiers.rs`, `headers.rs`. File-level parser in `file.rs`.

pub mod classify;
pub mod dependent_tiers;
pub mod entry_points;
pub mod file;
mod header_fragment;
pub mod headers;
pub(crate) use header_fragment::HeaderFragment;
mod located;
pub mod main_tier;
pub mod word_body;

// Re-export all public entry points so existing code can use `parser::parse_*`.
pub use entry_points::*;

use crate::lexer::Lexer;
use crate::token::Token;

/// Token storage may be shorter lived than the source borrowed by each token.
pub(crate) type Tokens<'tokens, 'source> = &'tokens [Token<'source>];

/// Lex borrowed source into temporary token storage.
pub(crate) fn lex_to_tokens(input: &str, start_condition: usize) -> Vec<Token<'_>> {
    Lexer::new(input, start_condition)
        .map(|(token, _)| token)
        .collect()
}

/// Return temporary tokens together with their original borrowed source.
pub(crate) fn lex_to_tokens_and_source(
    input: &str,
    start_condition: usize,
) -> (Vec<Token<'_>>, &str) {
    (lex_to_tokens(input, start_condition), input)
}

/// Source, tokens and lexer spans admitted together by one lexer run.
/// No caller can pair the token stream with a different source or span table.
pub(crate) use located::LexedSource;
