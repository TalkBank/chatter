//! Rich source snippets and expectations attached to parse diagnostics.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::{FragmentRangeError, FragmentSource, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Source excerpt admission failed before a diagnostic context was constructed.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum SourceExcerptError {
    /// The requested range is reversed, outside the source, or splits UTF-8.
    #[error("excerpt range {start}..{end} is not a UTF-8 slice of source length {source_len}")]
    InvalidRange {
        /// Requested start byte.
        start: usize,
        /// Requested exclusive end byte.
        end: usize,
        /// Original source length in bytes.
        source_len: usize,
    },
    /// The excerpt cannot fit the model's snippet-coordinate space.
    #[error(transparent)]
    Unrepresentable(#[from] FragmentRangeError),
}

/// Rich error context for helpful error messages
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorContext {
    /// The source text containing the error
    pub source_text: String,
    /// Byte offset span highlighting the error (relative to source_text)
    #[serde(flatten)]
    pub span: Span,
    /// What was expected at this position (multiple possibilities).
    /// SmallVec avoids heap allocation for the common case of 0-2 items.
    /// An omitted wire field is the serializer's representation of no listed
    /// expectations, not evidence for an invented expected token.
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    #[schemars(with = "Vec<String>")]
    pub expected: SmallVec<[String; 2]>,
    /// What was actually found
    pub found: String,
    /// Starting line number of source_text in the original file (1-indexed)
    /// Used by miette to display correct line numbers in error output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_offset: Option<usize>,
}

impl miette::SourceCode for ErrorContext {
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        read_source_span(
            "input",
            &self.source_text,
            self.line_offset.unwrap_or(1),
            span,
            context_lines_before,
            context_lines_after,
        )
    }
}

/// Read a borrowed snippet without duplicating its mutable source/offset state.
pub(super) fn read_source_span<'a>(
    name: &str,
    source: &'a str,
    line_offset: usize,
    span: &miette::SourceSpan,
    context_lines_before: usize,
    context_lines_after: usize,
) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
    let contents = miette::SourceCode::read_span(
        source.as_bytes(),
        span,
        context_lines_before,
        context_lines_after,
    )?;
    let line = line_offset
        .saturating_sub(1)
        .checked_add(contents.line())
        .ok_or(miette::MietteError::OutOfBounds)?;
    // Handlers add one-based line numbers and walk the entire returned context.
    // miette's line_count counts separators, not a final unterminated line.
    let trailing_line = usize::from(!matches!(contents.data().last(), Some(b'\n' | b'\r')));
    let extent = contents
        .line_count()
        .checked_add(trailing_line)
        .ok_or(miette::MietteError::OutOfBounds)?
        .max(1);
    line.checked_add(extent)
        .ok_or(miette::MietteError::OutOfBounds)?;
    Ok(Box::new(miette::MietteSpanContents::new_named(
        name.to_owned(),
        contents.data(),
        *contents.span(),
        line,
        contents.column(),
        contents.line_count(),
    )))
}

impl ErrorContext {
    /// Create error context from source text and byte span
    pub fn new(source_text: impl Into<String>, span: Span, found: impl Into<String>) -> Self {
        Self {
            source_text: source_text.into(),
            span,
            expected: SmallVec::new(),
            found: found.into(),
            line_offset: None,
        }
    }

    /// Create error context from reconstructed text (when original source unavailable)
    ///
    /// This is useful when validating structures that were parsed but the original
    /// source text is not available at validation time. The reconstructed text
    /// (from WriteChat serialization) is used as both source and found text.
    pub fn from_reconstructed(reconstructed_text: impl Into<String>, span: Span) -> Self {
        let text = reconstructed_text.into();
        Self {
            source_text: text.clone(),
            span,
            expected: SmallVec::new(),
            found: text,
            line_offset: None,
        }
    }

    /// Set the list of expected values for this error context.
    pub fn with_expected(mut self, expected: impl Into<SmallVec<[String; 2]>>) -> Self {
        self.expected = expected.into();
        self
    }

    /// Set the line offset for miette formatting
    pub fn with_line_offset(mut self, line: usize) -> Self {
        self.line_offset = Some(line);
        self
    }

    /// Create error context from full source text and byte span, calculating line offset automatically
    ///
    /// This extracts a snippet around the span and calculates which line number it starts at
    ///
    /// Invalid byte ranges and unrepresentable snippet coordinates are errors,
    /// never substituted with an empty snippet. Valid zero-width ranges remain valid.
    pub fn from_source_with_span(
        full_source: &str,
        span_start: usize,
        span_end: usize,
        found: impl Into<String>,
    ) -> Result<Self, SourceExcerptError> {
        use crate::LineColumn;

        let text =
            full_source
                .get(span_start..span_end)
                .ok_or(SourceExcerptError::InvalidRange {
                    start: span_start,
                    end: span_end,
                    source_len: full_source.len(),
                })?;
        let admitted = FragmentSource::new(text, 0)?;

        // Calculate line number for the start of the span
        let line = LineColumn::of_offset(span_start, full_source).line.get();

        let source_text = admitted.input().to_owned();

        // Create context with relative span (0..length within the snippet)
        let relative_span = Span::new(0, source_text.len());

        Ok(Self {
            source_text,
            span: relative_span,
            expected: SmallVec::new(),
            found: found.into(),
            line_offset: Some(line),
        })
    }
}
