//! Conversion from our AST to `talkbank-model` types.
//!
//! Each conversion is built from studying the actual JSON output of the
//! TreeSitterParser (via `chatter to-json`), ensuring semantic equivalence.
//!
//! Rules:
//! - No dummy/sentinel values. Every model field must be correct or absent.
//! - No silent drops. Every AST item must be converted or produce an error.
//! - No panics. All conversions are infallible for valid AST input.

mod headers;
mod items;
mod text_tiers;
mod tiers;
mod words;

pub use headers::*;
pub use items::*;
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
