//! Builder pattern for constructing [`ParseError`] instances ergonomically.

use super::Span;
use super::codes::ErrorCode;
use super::context::ErrorContext;
use super::parse_error::ParseError;
use super::source_location::{ErrorLabel, Severity, SourceLocation};
use std::sync::OnceLock;

/// Builder for constructing ParseError instances ergonomically.
///
/// Created via `ParseError::build(code)`. Provides a fluent API for
/// setting error properties without the verbose constructor.
///
/// # Required Fields
///
/// The following must be set before calling `finish()`:
/// - `message` - Human-readable error description
/// - `location` - Explicit diagnostic location
///
/// The type parameters carry the required values, or `()` before they are set.
/// Setters may be called in either order; finishing is infallible and only
/// available when both values are present.
///
/// # Optional Fields with Defaults
///
/// - `severity` - Defaults to `Severity::Error`
/// - `context` - No context by default
/// - `suggestion` - No suggestion by default
///
/// # Example
///
/// ```
/// use talkbank_model::{ParseError, ErrorCode, Severity};
///
/// // Minimal usage
/// let _error = ParseError::build(ErrorCode::InvalidMediaBullet)
///     .at(0, 1)
///     .message("Invalid format")
///     .finish();
///
/// // Full usage
/// let _error = ParseError::build(ErrorCode::MissingTerminator)
///     .severity(Severity::Error)
///     .at(0, 5)
///     .context_from_source("hello world", 0..5, "hello")
///     .message("Missing utterance terminator")
///     .suggestion("Add a period, question mark, or exclamation point")
///     .finish();
/// ```
///
/// Missing message:
/// ```compile_fail
/// use talkbank_model::{ParseError, ErrorCode};
/// ParseError::build(ErrorCode::ParseFailed).at(0, 1).finish();
/// ```
///
/// Missing location:
/// ```compile_fail
/// use talkbank_model::{ParseError, ErrorCode};
/// ParseError::build(ErrorCode::ParseFailed).message("Rejected").finish();
/// ```
#[derive(Debug)]
pub struct ParseErrorBuilder<Message = (), Location = ()> {
    code: ErrorCode,
    severity: Severity,
    location: Location,
    context: Option<ErrorContext>,
    message: Message,
    suggestion: Option<String>,
    labels: Vec<ErrorLabel>,
}

impl ParseErrorBuilder {
    /// Create a new builder with the given error code.
    pub(crate) fn new(code: ErrorCode) -> Self {
        Self {
            code,
            severity: Severity::Error,
            location: (),
            context: None,
            message: (),
            suggestion: None,
            labels: Vec::new(),
        }
    }
}

impl<Message, Location> ParseErrorBuilder<Message, Location> {
    /// Set the error severity (default: Error).
    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the error location from byte offsets.
    pub fn at(self, start: usize, end: usize) -> ParseErrorBuilder<Message, SourceLocation> {
        self.location(SourceLocation::from_offsets(start, end))
    }

    /// Set the error location from a Span.
    pub fn at_span(self, span: Span) -> ParseErrorBuilder<Message, SourceLocation> {
        self.location(SourceLocation::new(span))
    }

    /// Set the error location from a SourceLocation.
    pub fn location(self, location: SourceLocation) -> ParseErrorBuilder<Message, SourceLocation> {
        ParseErrorBuilder {
            code: self.code,
            severity: self.severity,
            location,
            context: self.context,
            message: self.message,
            suggestion: self.suggestion,
            labels: self.labels,
        }
    }

    /// Set the error context directly.
    pub fn context(mut self, context: ErrorContext) -> Self {
        self.context = Some(context);
        self
    }

    /// Set the error context from source text components.
    ///
    /// # Arguments
    ///
    /// * `source_text` - The source code containing the error
    /// * `span` - Byte range of the error within source_text
    /// * `offending_text` - The specific text that caused the error
    pub fn context_from_source(
        mut self,
        source_text: impl Into<String>,
        span: std::ops::Range<usize>,
        offending_text: impl Into<String>,
    ) -> Self {
        self.context = Some(ErrorContext::new(source_text, span, offending_text));
        self
    }

    /// Set the human-readable error message (required).
    pub fn message(self, message: impl Into<String>) -> ParseErrorBuilder<String, Location> {
        ParseErrorBuilder {
            code: self.code,
            severity: self.severity,
            location: self.location,
            context: self.context,
            message: message.into(),
            suggestion: self.suggestion,
            labels: self.labels,
        }
    }

    /// Set a suggestion for how to fix the error.
    pub fn suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add a secondary label span.
    pub fn label(mut self, label: ErrorLabel) -> Self {
        self.labels.push(label);
        self
    }
}

impl ParseErrorBuilder<String, SourceLocation> {
    /// Build the diagnostic after both required fields have been supplied.
    pub fn finish(self) -> ParseError {
        let help_url = Some(self.code.documentation_url());

        ParseError {
            code: self.code,
            severity: self.severity,
            location: self.location,
            context: self.context,
            labels: self.labels,
            message: self.message,
            suggestion: self.suggestion,
            help_url,
            source_cache: OnceLock::new(),
        }
    }
}
