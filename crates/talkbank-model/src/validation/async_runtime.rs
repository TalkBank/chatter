//! Async validation runtime orchestration for integration with async runtimes.
//!
//! Validation is inherently CPU-bound (parsing, tree traversal, checks), not I/O-bound.
//! These entry points allow running validation on async runtimes like Tokio by:
//! - Offloading to blocking thread pools (tokio::task::spawn_blocking)
//! - Reusing async-compatible error sinks from the core error subsystem
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//!
//! Enable the crate's `async` feature and call these functions from a Tokio
//! runtime. Complete validation returns [`crate::validation::ValidChatFile`];
//! selecting individual rules reports findings without producing that proof.
//!
//! # Example with a caller-supplied document
//!
//! ```no_run
//! use talkbank_model::{AsyncChannelErrorSink, ChatFile, ParseError};
//! use talkbank_model::model::OwnedTranscriptName;
//! use talkbank_model::validation::{AsyncValidationError, ValidChatFile, validate_async};
//!
//! async fn validate_document(
//!     file: ChatFile,
//! ) -> (Result<ValidChatFile, AsyncValidationError>, Vec<ParseError>) {
//!     let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
//!
//!     // The task owns the sink. Its sender is dropped when the task completes.
//!     let result = validate_async(
//!         file, AsyncChannelErrorSink::new(sender), OwnedTranscriptName::Anonymous,
//!     ).await;
//!
//!     // Retain even partial findings if the result reports a task failure.
//!     let mut diagnostics = Vec::new();
//!     while let Some(diagnostic) = receiver.recv().await {
//!         diagnostics.push(diagnostic);
//!     }
//!     (result, diagnostics)
//! }
//! # fn main() {}
//! ```

use crate::ErrorSink;
use crate::model::ChatFile;
use crate::validation::RuleSelection;

/// Async wrapper errors emitted by validation task orchestration.
#[cfg(feature = "async")]
#[derive(Debug, thiserror::Error)]
pub enum AsyncValidationError {
    /// The Tokio blocking task failed to join successfully.
    #[error("failed to join async validation task: {0}")]
    Join(#[from] tokio::task::JoinError),
    /// Model validation refused the document.
    #[error(transparent)]
    Validation(#[from] crate::validation::ValidationFailure),
}

/// Validate a `ChatFile` on Tokio's blocking pool and await completion.
///
/// Since validation is CPU-bound, this runs the validation on a blocking thread
/// to avoid blocking the async runtime's event loop.
///
/// # Parameters
///
/// * `file` - The `ChatFile` to validate (consumed, success returns `ValidChatFile`)
/// * `errors` - Owned error sink (`ErrorSink + Send + 'static`); no `Clone` bound
/// * `name` - The transcript's name, for the rules about it (`@Media`)
///
/// # Returns
///
/// A `ValidChatFile` after complete validation succeeds. Semantic refusal is
/// [`AsyncValidationError::Validation`]; task failure is [`AsyncValidationError::Join`].
///
/// # Example
///
/// ```no_run
/// use talkbank_model::{ChatFile, ErrorCollector};
/// use talkbank_model::model::{OwnedFileStem, OwnedTranscriptName};
/// use talkbank_model::validation::{ValidChatFile, validate_async};
///
/// async fn validate_named(file: ChatFile) -> Result<ValidChatFile, Box<dyn std::error::Error>> {
///     let errors = ErrorCollector::new();
///     let name = OwnedTranscriptName::Named(OwnedFileStem::new("sample")?);
///     Ok(validate_async(file, errors, name).await?)
/// }
/// # fn main() {}
/// ```
#[cfg(feature = "async")]
pub async fn validate_async<S>(
    file: ChatFile,
    errors: S,
    name: crate::model::OwnedTranscriptName,
) -> Result<crate::validation::ValidChatFile, AsyncValidationError>
where
    S: ErrorSink + Send + 'static,
{
    tokio::task::spawn_blocking(move || file.validate_into(&errors, name.borrow()))
        .await
        .map_err(AsyncValidationError::from)?
        .map_err(AsyncValidationError::from)
}

/// Validate a `ChatFile` asynchronously with a custom `RuleSelection`.
///
/// This variant mirrors `validate_async` but runs an explicit rule selection
/// rather than the default one.
///
/// # Parameters
///
/// * `file` - The `ChatFile` to inspect (consumed by the async task)
/// * `rules` - Which validation rules to run
/// * `errors` - Error sink for collecting validation errors (must be Send + 'static)
/// * `name` - The transcript's name, for the rules about it (`@Media`)
///
/// # Returns
///
/// Successful task completion, not proof that the document is valid. Selected
/// rules can report findings through `errors` while this function returns `Ok(())`.
/// Use [`validate_async`] when a complete `ValidChatFile` proof is required.
///
/// # Example
///
/// ```no_run
/// use talkbank_model::{AsyncChannelErrorSink, ChatFile, ParseError};
/// use talkbank_model::model::OwnedTranscriptName;
/// use talkbank_model::validation::{validate_with_rules_async, RuleSelection};
/// use talkbank_model::validation::AsyncValidationError;
///
/// async fn collect_strict_linker_findings(
///     file: ChatFile,
/// ) -> (Result<(), AsyncValidationError>, Vec<ParseError>) {
///     let rules = RuleSelection::new().with_strict_linkers();
///     let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
///
///     let result = validate_with_rules_async(
///         file, rules, AsyncChannelErrorSink::new(sender), OwnedTranscriptName::Anonymous,
///     ).await;
///     let mut diagnostics = Vec::new();
///     while let Some(diagnostic) = receiver.recv().await {
///         diagnostics.push(diagnostic);
///     }
///     (result, diagnostics)
/// }
/// # fn main() {}
/// ```
#[cfg(feature = "async")]
pub async fn validate_with_rules_async<S>(
    file: ChatFile,
    rules: RuleSelection,
    errors: S,
    name: crate::model::OwnedTranscriptName,
) -> Result<(), AsyncValidationError>
where
    S: ErrorSink + Send + 'static,
{
    tokio::task::spawn_blocking(move || file.validate_with_rules(rules, &errors, name.borrow()))
        .await
        .map_err(AsyncValidationError::from)
}

#[cfg(all(test, feature = "async"))]
mod tests {
    use super::*;
    use crate::ErrorCollector;
    use crate::Span;
    use crate::model::{Header, Line};

    /// Builds a minimal syntactically valid test file.
    fn make_test_file() -> ChatFile {
        ChatFile::new(vec![
            Line::header_with_span(Header::Utf8, Span::DUMMY),
            Line::header_with_span(Header::Begin, Span::DUMMY),
            Line::header_with_span(Header::End, Span::DUMMY),
        ])
    }

    /// Ensures semantic refusal remains distinct from task failure.
    #[tokio::test]
    async fn test_validate_async() -> Result<(), AsyncValidationError> {
        let file = make_test_file();
        let errors = ErrorCollector::new();

        let rejected =
            validate_async(file, errors, crate::model::OwnedTranscriptName::Anonymous).await;
        assert!(matches!(rejected, Err(AsyncValidationError::Validation(_))));

        // Should complete without panicking
        Ok(())
    }

    /// Verifies validate with config async.
    #[tokio::test]
    async fn test_validate_with_rules_async() -> Result<(), AsyncValidationError> {
        let file = make_test_file();
        let config = RuleSelection::new();
        let errors = ErrorCollector::new();

        validate_with_rules_async(
            file,
            config,
            errors,
            crate::model::OwnedTranscriptName::Anonymous,
        )
        .await?;

        // Should complete without panicking
        Ok(())
    }
}
