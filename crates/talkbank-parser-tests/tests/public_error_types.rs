// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! API-surface guard: every public fallible constructor's error type must
//! itself be publicly nameable by downstream crates.
//!
//! Regression for the 2026-07-08 finding (BUG-3): `LanguageCode::new`
//! returned `Result<_, LanguageCodeError>` but `LanguageCodeError` was not
//! re-exported, so the first real downstream consumer (batchalign3) could
//! not store it in a `thiserror` `#[source]` field and had to stringify at
//! the boundary. If this file stops compiling, a public constructor's
//! error type has become unnameable again.

use talkbank_model::model::{LanguageCode, LanguageCodeError};

/// The canonical downstream shape: a domain error carrying the upstream
/// construction error as a typed source. This is what batchalign3 wants
/// to write; it must always be possible.
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

/// The other public fallible constructors' error types, named through
/// their public paths so a rename or visibility regression fails here.
#[test]
fn other_public_constructor_error_types_are_nameable() {
    fn assert_error<E: std::error::Error>() {}
    assert_error::<talkbank_model::alignment::SemanticWordIndexError>();
    assert_error::<talkbank_model::model::XphointParseError>();
    assert_error::<talkbank_model::model::PhoalnParseError>();
}
