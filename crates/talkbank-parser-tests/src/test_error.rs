//! Shared error type for parser-test binaries and integration tests.
//!
//! Unifies IO, parsing, serialisation, and assertion failures into a single
//! `Result` type so test functions can use `?` throughout.

use thiserror::Error;

/// Shared failure modes for parser-test binaries and integration suites.
#[derive(Debug, Error)]
pub enum TestError {
    /// File system operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// String formatting failed.
    #[error("Format error: {0}")]
    Fmt(#[from] std::fmt::Error),
    /// Required environment variable missing or invalid.
    #[error("Env var error: {0}")]
    EnvVar(#[from] std::env::VarError),
    /// CHAT parsing produced errors.
    #[error("Parse error: {0}")]
    Parse(#[from] talkbank_model::ParseErrors),
    /// Tree-sitter parser failed to initialise.
    #[error("Parser init error: {0}")]
    ParserInit(String),
    /// Snapshot serialization or deserialization failed.
    #[error("Snapshot serialization error: {0}")]
    Snapshot(#[from] serde_json::Error),
    /// General test assertion failure with message.
    #[error("Test failure: {0}")]
    Failure(String),
}

impl From<talkbank_parser::ParserInitError> for TestError {
    /// Convert parser initialization failures into `TestError`.
    fn from(err: talkbank_parser::ParserInitError) -> Self {
        TestError::ParserInit(err.to_string())
    }
}

/// Convert a [`talkbank_parser::ParseProduct`] into the pre-`ParseProduct`
/// strict-parse contract: a [`talkbank_parser::ParseProduct::Built`] that
/// carries an error-severity diagnostic is treated as a failure, the same
/// as [`talkbank_parser::ParseProduct::Unbuildable`].
///
/// Spec-generated construct/error tests (see `spec/tools/src/output/rust_test.rs`)
/// and hand-written parser-suite tests that assert a fixture parses
/// completely cleanly use this, rather than
/// [`talkbank_parser::ParseProduct::expect_built`] alone, which only
/// answers "was a model built," not "was it built without error
/// diagnostics." Kept in this test-support crate (never in
/// `talkbank-parser` itself): a convenience that silently discards a
/// built model on any diagnostic is exactly the footgun `ParseProduct`
/// exists to make impossible to reach for by accident in production code.
pub fn strict_parse(
    product: talkbank_parser::ParseProduct,
) -> Result<talkbank_model::model::ChatFile, talkbank_model::ParseErrors> {
    match product {
        talkbank_parser::ParseProduct::Built { file, diagnostics } => {
            if diagnostics
                .iter()
                .any(|d| matches!(d.severity, talkbank_model::Severity::Error))
            {
                Err(talkbank_model::ParseErrors::from(diagnostics))
            } else {
                Ok(file)
            }
        }
        talkbank_parser::ParseProduct::Unbuildable { diagnostics } => {
            Err(talkbank_model::ParseErrors::from(diagnostics))
        }
    }
}
