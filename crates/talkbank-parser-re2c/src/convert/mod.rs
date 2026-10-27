//! Conversion from our AST to `talkbank-model` types.
//!
//! Remaining parser-specific carriers are lowered to the shared model. Both
//! backends implement the CHAT contract; neither backend is a semantic oracle.
//! MOR and text bodies already construct shared values without parallel ASTs.
//!
//! Rules:
//! - No dummy/sentinel values. Every model field must be correct or absent.
//! - No silent drops. Every AST item must be converted or produce an error.
//! - No panics. All conversions are infallible for valid AST input.
//!
//! Raw content and token conversion must not grant parser admission to
//! caller-assembled payloads. Source membership of a lexeme alone does not
//! establish that its payload or token kind describes those bytes. These
//! helpers are internal; public lowering consumes parser-owned words, tiers,
//! headers or files, or starts with the input-based parser entry points.
//!
//! ```compile_fail,E0603
//! use talkbank_model::model::WriteChat;
//! use talkbank_parser_re2c::{ast::ContentItem, convert, source_text::SourceText};
//! let source = String::from("&*CHI:hello");
//! let forged = ContentItem::OtherSpokenEvent(talkbank_model::OtherSpokenEvent::new(
//!     talkbank_model::Location::Unlocated,
//!     "CHI",
//!     talkbank_model::Word::new(
//!         talkbank_model::Location::Unlocated,
//!         talkbank_model::non_empty_literal!("goodbye").into(),
//!     ),
//! ));
//! let talkbank_model::model::UtteranceContent::OtherSpokenEvent(model) =
//!     convert::content_item_to_model(&forged, SourceText::new(&source)) else {
//!     panic!("event conversion");
//! };
//! assert_eq!(model.to_chat_string(), source);
//! ```
//!
//! ```compile_fail,E0603
//! use talkbank_model::model::WriteChat;
//! use talkbank_parser_re2c::{convert, source_text::SourceText, token::Token};
//! let source = String::from("?");
//! let forged = Token::Period(&source);
//! let model = convert::token_to_terminator(&forged, SourceText::new(&source)).unwrap();
//! assert_eq!(model.to_chat_string(), source);
//! ```

mod headers;
mod items;
mod plain_text;
mod text_tiers;
mod tiers;
mod words;

pub use headers::*;
pub(crate) use items::*;
pub use text_tiers::*;
pub use tiers::*;
pub use words::*;

/// An optional span as a `Location`: the one place `None` becomes `Unlocated`
/// for the hull computations, so no converter spells the fallback itself.
pub(crate) fn located(span: Option<talkbank_model::Span>) -> talkbank_model::Location {
    span.map_or(
        talkbank_model::Location::Unlocated,
        talkbank_model::Location::Located,
    )
}

/// Where a slice the lexer borrowed from `source` sits, or `Unlocated` when
/// the slice is not from this source (a programming error pairing an AST with
/// the wrong source). One owner for the question; every converter used to
/// spell the `map_or` itself.
pub(crate) fn locate(
    source: crate::source_text::SourceText<'_>,
    slice: &str,
) -> talkbank_model::Location {
    located(source.span_of(slice))
}

/// The same question for a diagnostic: where to report about a slice, or an
/// unlocated diagnostic when the slice is not from `source`.
pub(crate) fn diagnostic_location(
    source: crate::source_text::SourceText<'_>,
    slice: &str,
) -> talkbank_model::DiagnosticLocation {
    source.span_of(slice).map_or(
        talkbank_model::DiagnosticLocation::Unlocated,
        talkbank_model::DiagnosticLocation::new,
    )
}
