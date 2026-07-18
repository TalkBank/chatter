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

//! API-surface guard: every public fallible constructor's error type must
//! itself be publicly nameable by downstream crates, through a path rooted
//! at the SAME crate that exposes the constructor.
//!
//! Regression for the 2026-07-08 finding (BUG-3): `LanguageCode::new`
//! returned `Result<_, LanguageCodeError>` but `LanguageCodeError` was not
//! re-exported, so the first real downstream consumer (batchalign3) could
//! not store it in a `thiserror` `#[source]` field and had to stringify at
//! the boundary. The bug's exit criterion is a compile-test that names
//! EVERY public constructor error type; this file is that test. If it stops
//! compiling, a public constructor's error type has become unnameable from
//! its crate root again.
//!
//! Coverage is enumerated from the 2026-07-17 audit of every public
//! `-> Result<_, E>` / `impl TryFrom` across the published library crates
//! (talkbank-model, talkbank-parser, talkbank-transform). A new public
//! fallible constructor MUST add its error type here.

use talkbank_model::model::{LanguageCode, LanguageCodeError};

/// Naming `T` in a turbofish forces its path to resolve at compile time, so
/// a type that becomes `pub(crate)` or loses its re-export breaks this test.
/// No trait bound: the bug is about NAMEABILITY. The stronger property (the
/// error type impls `std::error::Error`, so it can be a `#[source]`) is
/// demonstrated separately by `language_code_error_is_nameable_and_sourceable`.
fn assert_nameable<T>() {}

/// The canonical downstream shape: a domain error carrying the upstream
/// construction error as a typed source. This is what batchalign3 wants
/// to write; it must always be possible for at least the anchor type.
#[derive(Debug, thiserror::Error)]
enum DownstreamError {
    #[error("invalid language code {lang:?}")]
    InvalidLanguageCode {
        lang: String,
        #[source]
        source: LanguageCodeError,
    },
}

#[test]
fn language_code_error_is_nameable_and_sourceable() {
    let err = LanguageCode::new("").expect_err("empty code must fail");
    let wrapped = DownstreamError::InvalidLanguageCode {
        lang: String::new(),
        source: err,
    };
    assert!(std::error::Error::source(&wrapped).is_some());
}

/// talkbank-model: every public fallible constructor's error type, named
/// through its crate-root-reachable path.
#[test]
fn talkbank_model_constructor_error_types_are_nameable() {
    assert_nameable::<talkbank_model::LanguageCodeError>();
    assert_nameable::<talkbank_model::SemanticWordIndexError>();
    assert_nameable::<talkbank_model::SourceLocationError>();
    assert_nameable::<talkbank_model::ParseErrorBuilderError>();
    assert_nameable::<talkbank_model::SylWordError>();
    assert_nameable::<talkbank_model::XphointParseError>();
    assert_nameable::<talkbank_model::PhoalnParseError>();
    // `PositionCode::try_from(char)` has error type `char` (a primitive):
    // trivially nameable, and deliberately NOT asserted as an Error impl.
    // That it returns a bare `char` rather than a domain error is a minor
    // pre-existing API smell tracked separately, not part of BUG-3.
}

/// talkbank-parser: the parser's public methods return
/// `ParseResult<T> = Result<T, ParseErrors>`. Both must be nameable from
/// `talkbank_parser`'s OWN root, so a crate that depends only on
/// talkbank-parser (not talkbank-model) can name the error type of a method
/// it calls. This is the BUG-3 defect the fix closes.
#[test]
fn talkbank_parser_constructor_error_types_are_nameable() {
    assert_nameable::<talkbank_parser::ParserInitError>();
    assert_nameable::<talkbank_parser::ParseErrors>();
    assert_nameable::<talkbank_parser::ParseResult<()>>();
}

/// talkbank-transform: every public fallible constructor / serializer error
/// type, named through its crate-root-reachable path.
#[test]
fn talkbank_transform_constructor_error_types_are_nameable() {
    assert_nameable::<talkbank_transform::JsonError>();
    assert_nameable::<talkbank_transform::ManifestError>();
    assert_nameable::<talkbank_transform::build_chat::BuildChatError>();
    assert_nameable::<talkbank_transform::xml::XmlWriteError>();
    assert_nameable::<talkbank_transform::validate::ValidationError>();
    assert_nameable::<talkbank_transform::adjudication::AdjudicationError>();
    assert_nameable::<talkbank_transform::rediarize::InvertedSpan>();
    assert_nameable::<talkbank_transform::rediarize::TurnsJsonError>();
    assert_nameable::<talkbank_transform::speaker_id::SpeakerIdError>();
    assert_nameable::<talkbank_transform::speaker_id::OverrideFileError>();
    assert_nameable::<talkbank_transform::speaker_id::SessionContextError>();
    assert_nameable::<talkbank_transform::speaker_id::BlankLabelError>();
}
