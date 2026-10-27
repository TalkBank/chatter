//! The [`ParseError`] diagnostic type with source-backed context for `miette` rendering.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::Span;
use super::codes::ErrorCode;
use super::context::ErrorContext;
use super::source_location::{DiagnosticLocation, ErrorLabel, Severity};
use miette::Diagnostic;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::num::NonZeroUsize;
use thiserror::Error;

use super::builder::ParseErrorBuilder;

// Custom SourceCode implementation that supports line offsets for miette.
//
// Stores the raw source text and a line offset. When miette calls `read_span`,
// we delegate to the raw text and then adjust the starting line number in the
// returned `SpanContents`. This avoids the fragile "prepend newlines" hack
// that required coordinating span adjustments across three separate code paths.
/// Source-code wrapper that preserves original file line numbering for snippets.
pub(crate) struct SourceCodeWithOffset {
    name: String,
    source: String,
    /// 1-indexed line number where `source` starts in the original file
    line_offset: NonZeroUsize,
}

impl SourceCodeWithOffset {
    /// Create a source wrapper with a 1-indexed starting line offset.
    pub(super) fn new(
        name: impl Into<String>,
        source: impl Into<String>,
        line_offset: NonZeroUsize,
    ) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            line_offset,
        }
    }
}

impl miette::SourceCode for SourceCodeWithOffset {
    /// Read a span while adjusting line numbers back to original-file coordinates.
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        super::context::read_source_span(
            &self.name,
            &self.source,
            self.line_offset,
            span,
            context_lines_before,
            context_lines_after,
        )
    }
}

/// Structured parse/validation diagnostic with optional source-backed context.
#[derive(Clone, Debug, PartialEq, Error, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct ParseError {
    /// Typed error code identifying the rule that fired (an
    /// [`ErrorCode`] enum variant such as `ErrorCode::UnparsableContent`,
    /// rendered as `E316` in output).
    pub code: ErrorCode,
    /// Error severity
    pub severity: Severity,
    /// Source location
    pub location: DiagnosticLocation,
    /// Rich context for the error (optional - None when source not available during validation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ErrorContext>,
    /// Optional secondary labels for multi-span diagnostics
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub labels: Vec<ErrorLabel>,
    /// Human-readable error message (plain language).
    ///
    /// Downstream tools consume this text programmatically and surface it
    /// to their own users (e.g. Python bindings mapping diagnostics into
    /// their own structures), so message WORDING is part of the observable
    /// behavior of a release: rewording a message is a behavior change to
    /// note in the changelog, even though the type surface is unchanged.
    /// Match on [`code`](Self::code), never on this text.
    pub message: String,
    /// Optional suggestion for how to fix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Optional help URL (defaults to documentation_url from code)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_url: Option<String>,
}

impl ParseError {
    /// Create a new parse error with all required fields.
    ///
    /// This is the most general constructor. Prefer [`at_span`](Self::at_span) or
    /// [`from_source_span`](Self::from_source_span) for the common cases, or
    /// [`build`](Self::build) for the builder pattern.
    ///
    /// # Parameters
    ///
    /// - `code`: The error code identifying the kind of diagnostic (e.g., `ErrorCode::MissingTerminator`).
    /// - `severity`: Whether this is an error or a warning.
    /// - `location`: The source location (byte span, optional line/column).
    /// - `context`: Optional rich context for rendering source snippets. Pass `None`
    ///   when source text is not available (e.g., during post-parse validation).
    /// - `message`: Human-readable description of the problem.
    ///
    /// # Returns
    ///
    /// A fully constructed `ParseError` with `help_url` auto-populated from the error code's
    /// documentation URL, and no suggestion or secondary labels.
    pub fn new(
        code: ErrorCode,
        severity: Severity,
        location: DiagnosticLocation,
        context: impl Into<Option<ErrorContext>>,
        message: impl Into<String>,
    ) -> Self {
        let help_url = Some(code.documentation_url());
        Self {
            code,
            severity,
            location,
            context: context.into(),
            labels: Vec::new(),
            message: message.into(),
            suggestion: None,
            help_url,
        }
    }

    /// Create an error anchored to a byte span without source context.
    ///
    /// This is the preferred constructor when source text is not available for
    /// snippet rendering (e.g., during validation passes that only have the AST).
    /// The resulting error will display byte offsets but no source-code snippet.
    ///
    /// # Parameters
    ///
    /// - `code`: The error code identifying the kind of diagnostic.
    /// - `severity`: Whether this is an error or a warning.
    /// - `span`: Byte range in the source file where the problem occurred.
    /// - `message`: Human-readable description of the problem.
    pub fn at_span(
        code: ErrorCode,
        severity: Severity,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Self::at_location(code, severity, DiagnosticLocation::new(span), message)
    }

    /// Add a secondary label at a model value's location, when it has one.
    ///
    /// An unlocated value gets no label: a label needs a span to underline,
    /// and inventing one would point at the wrong place.
    pub fn with_label_at(self, location: crate::Location, message: impl Into<String>) -> Self {
        match location.span() {
            Some(span) => self.with_label(ErrorLabel::new(span, message)),
            None => self,
        }
    }

    /// Create a diagnostic at a location that may be unlocated, without context.
    ///
    /// The location is anything that converts into a [`DiagnosticLocation`]: a
    /// model value's [`Location`](crate::Location), a `Span`, or the
    /// diagnostic location itself.
    pub fn at_location(
        code: ErrorCode,
        severity: Severity,
        location: impl Into<DiagnosticLocation>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            code,
            severity,
            location.into(),
            Option::<ErrorContext>::None,
            message,
        )
    }

    /// Create an error anchored to a byte span with source-backed context.
    ///
    /// This is the preferred constructor during parsing, where the source text
    /// is available for rendering rich `miette` snippets with underlined spans.
    ///
    /// # Parameters
    ///
    /// - `code`: The error code identifying the kind of diagnostic.
    /// - `severity`: Whether this is an error or a warning.
    /// - `span`: Byte range in the source file where the problem occurred.
    /// - `source_text`: The surrounding source text for snippet rendering (typically
    ///   the full line or a few lines around the error).
    /// - `offending_text`: The specific text within `source_text` that caused the
    ///   error, used as the primary label in rendered diagnostics.
    /// - `message`: Human-readable description of the problem.
    pub fn from_source_span(
        code: ErrorCode,
        severity: Severity,
        span: Span,
        source_text: impl Into<String>,
        offending_text: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            code,
            severity,
            DiagnosticLocation::new(span),
            ErrorContext::new(source_text, span, offending_text),
            message,
        )
    }

    /// Start building a new parse error with ergonomic builder pattern.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::{ParseError, ErrorCode, Severity};
    ///
    /// let error = ParseError::build(ErrorCode::InvalidMediaBullet)
    ///     .severity(Severity::Error)
    ///     .at(10, 20)
    ///     .message("Invalid media bullet format")
    ///     .suggestion("Use format: ·start_end·")
    ///     .finish();
    /// ```
    pub fn build(code: ErrorCode) -> ParseErrorBuilder {
        ParseErrorBuilder::new(code)
    }

    /// Create a simple internal error for unexpected conditions.
    ///
    /// Use this for internal parser errors that indicate bugs or
    /// unimplemented features rather than user-facing CHAT errors.
    /// The error code is always [`ErrorCode::InternalError`] with
    /// [`Severity::Error`] and no source context.
    ///
    /// # Parameters
    ///
    /// - `message`: Description of the internal problem.
    /// - `span`: Byte range where the unexpected condition occurred.
    pub fn internal(message: impl Into<String>, span: Span) -> Self {
        Self::new(
            ErrorCode::InternalError,
            Severity::Error,
            DiagnosticLocation::new(span),
            None,
            message,
        )
    }

    /// Add a suggestion for how to fix this error.
    ///
    /// The suggestion is displayed as a `help:` line in `miette` output.
    /// Returns `self` for method chaining.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add a secondary label pointing to a related source span.
    ///
    /// Secondary labels appear as additional underlined spans in `miette`
    /// output, providing context such as "first declared here" or
    /// "expected type from this". Returns `self` for method chaining.
    pub fn with_label(mut self, label: ErrorLabel) -> Self {
        self.labels.push(label);
        self
    }

    /// Override the help URL that links to documentation for this error.
    ///
    /// By default, the URL is derived from the error code's
    /// [`documentation_url()`](ErrorCode::documentation_url). Use this method
    /// to point to a more specific page. Returns `self` for method chaining.
    pub fn with_help_url(mut self, url: impl Into<String>) -> Self {
        self.help_url = Some(url.into());
        self
    }
}

impl fmt::Display for ParseError {
    /// Render a concise diagnostic summary for logs/CLI output.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}]: {} ({})",
            self.severity, self.code, self.message, self.location
        )
    }
}

impl Diagnostic for ParseError {
    /// Expose machine-readable diagnostic code to `miette`.
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(self.code.as_str()))
    }

    /// Expose severity level to `miette`.
    fn severity(&self) -> Option<miette::Severity> {
        Some(self.severity.into())
    }

    /// Expose suggestion text as optional help message.
    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.suggestion
            .as_ref()
            .map(|s| Box::new(s) as Box<dyn std::fmt::Display>)
    }

    /// Expose documentation URL when available.
    fn url<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.help_url
            .as_ref()
            .map(|u| Box::new(u) as Box<dyn std::fmt::Display>)
    }

    /// Provide primary and secondary labeled spans for rich diagnostics.
    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        let ctx = self.context.as_ref()?;

        let mut labels = Vec::with_capacity(1 + self.labels.len());

        // Primary location, use context.span (relative to source_text)
        // not location.span (absolute file offsets).
        // No offset adjustment needed: ErrorContext handles line
        // numbering natively via read_span, so spans are used as-is.
        labels.push(miette::LabeledSpan::new_with_span(
            Some("here".to_string()),
            ctx.span,
        ));

        // Compatibility rendering of producer-supplied context. Full-source
        // rendering uses SourceDiagnostic, which admits absolute labels and
        // owns their separate display-local projection without mutating these.
        for label in &self.labels {
            labels.push(miette::LabeledSpan::new_with_span(
                Some(label.message.clone()),
                label.span,
            ));
        }

        Some(Box::new(labels.into_iter()))
    }

    /// Provide source text/snippet provider for `miette` rendering.
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        let ctx = self.context.as_ref()?;

        if ctx.source_text.is_empty() {
            return None;
        }

        // Context is publicly mutable. Borrow its current source instead of
        // retaining a stale copy after the first rendering.
        Some(ctx as &dyn miette::SourceCode)
    }
}
