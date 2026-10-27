//! CHAT parser, re2c lexer + chumsky parser combinators.
//!
//! Entry points in `entry_points.rs`. Chumsky parsers in `main_tier.rs`,
//! `dependent_tiers.rs`, `headers.rs`. File-level parser in `file.rs`.
//!
//! Public entry points lex their input before invoking internal grammars.
//! Raw-token combinators are not a separate admission path: they cannot be used
//! to attach a foreign body to an unrelated original word spelling.
//!
//! ```compile_fail,E0603
//! use chumsky::Parser;
//! use talkbank_model::WriteChat;
//! use talkbank_parser_re2c::{ast::ContentItem, convert::word_from_parsed, token::Token};
//! use talkbank_parser_re2c::parser::main_tier::rich_word;
//! use talkbank_parser_re2c::source_text::SourceText;
//! let source = "hello";
//! let tokens = [Token::Word {
//!     raw_text: source,
//!     prefix: None,
//!     body: "goodbye",
//!     form_marker: None,
//!     lang_suffix: None,
//!     pos_tag: None,
//! }];
//! let ContentItem::Word(parsed) = rich_word(SourceText::new(source))
//!     .parse(tokens.as_slice()).into_result().unwrap() else { panic!("word") };
//! assert_eq!(word_from_parsed(&parsed).to_chat_string(), source);
//! ```
//! Raw token-to-grammar classification is internal as well. Public token
//! inspection does not grant a second annotation-admission API.
//!
//! ```compile_fail,E0603
//! use talkbank_parser_re2c::parser::classify::token_to_parsed_annotation;
//! ```

pub(crate) mod classify;
pub(crate) mod dependent_tiers;
pub mod entry_points;
pub mod file;
mod gra;
mod header_fragment;
pub(crate) mod headers;
pub(crate) use header_fragment::HeaderFragment;
mod located;
pub(crate) mod main_tier;
pub mod word_body;

// Re-export all public entry points so existing code can use `parser::parse_*`.
pub use entry_points::*;

use crate::lexer::Lexer;
use crate::token::Token;

/// Token storage may be shorter lived than the source borrowed by each token.
pub(crate) type Tokens<'tokens, 'source> = &'tokens [Token<'source>];

/// Lex borrowed source into temporary token storage.
pub(crate) fn lex_to_tokens(
    input: &str,
    start_condition: crate::lexer::StartCondition,
) -> Vec<Token<'_>> {
    Lexer::new(input, start_condition)
        .map(|(token, _)| token)
        .collect()
}

/// Return temporary tokens together with their original borrowed source.
/// Source, tokens and lexer spans admitted together by one lexer run.
/// No caller can pair the token stream with a different source or span table.
pub(crate) use located::{LexedSource, LocatedTokens, TokenCursor};

mod word_language;
