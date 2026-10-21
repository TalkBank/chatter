//! Post-processing helpers that enrich diagnostics with line/column and snippet context.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::chat_formatting::{PlainDisplayResult, process_for_plain_display_mapped};
use crate::line_map::SourceIndex;
use crate::{
    DiagnosticContext, DiagnosticLabel, DiagnosticLocation, DiagnosticPresentation,
    DiagnosticSourceError, DisplaySpan, ErrorCode, LineColumn, ParseError, SourceDiagnostic, Span,
};

/// Check if an error code requires raw source display (no CHAT formatting)
///
/// Errors about formatting characters themselves should show raw control characters
/// so users can see what's wrong. For example:
/// - E356/E357: Unmatched underline markers
/// - E360: Invalid bullet delimiters
fn requires_raw_display(code: &ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::UnmatchedUnderlineBegin
            | ErrorCode::UnmatchedUnderlineEnd
            | ErrorCode::InvalidMediaBullet
    )
}

/// Enhance errors with line/column information and source context from full source.
///
/// Builds a source-bound index once for the batch. For repeated batches on
/// the same immutable source, retain a [`SourceIndex`] and use
/// [`enhance_errors_with_index`]. Construction is O(source bytes), followed
/// by O(log lines) lookups for each diagnostic and label.
pub fn enhance_errors_with_source<'a>(
    errors: &'a [ParseError],
    full_source: &'a str,
) -> impl Iterator<Item = SourceDiagnostic> + 'a {
    let index = SourceIndex::new(full_source);
    errors
        .iter()
        .map(move |error| SourceDiagnostic::with_index(error.clone(), &index))
}

/// Enhance diagnostics using an index bound to its original immutable source.
///
/// Source and line boundaries share an owner. Every diagnostic range is checked
/// separately; range admission does not prove that the diagnostic originated in
/// this source. The caller must retain the producing snapshot.
pub fn enhance_errors_with_index<'a>(
    errors: &'a [ParseError],
    index: &'a SourceIndex<'_>,
) -> impl Iterator<Item = SourceDiagnostic> + 'a {
    errors
        .iter()
        .map(move |error| SourceDiagnostic::with_index(error.clone(), index))
}

/// Admit all absolute ranges before computing the display window.
pub(super) fn project(
    error: &ParseError,
    index: &SourceIndex<'_>,
) -> Result<DiagnosticPresentation, DiagnosticSourceError> {
    let full_source = index.source();
    let line_map = index.lines();
    let source_len = full_source.len();
    let primary = match error.location {
        DiagnosticLocation::Known { span, .. } => span,
        DiagnosticLocation::Unlocated => return Err(DiagnosticSourceError::Unlocated),
    };
    if full_source.get(primary.to_range()).is_none() {
        return Err(DiagnosticSourceError::PrimaryRange {
            span: primary,
            source_len: full_source.len(),
        });
    }
    for (index, label) in error.labels.iter().enumerate() {
        if full_source.get(label.span.to_range()).is_none() {
            return Err(DiagnosticSourceError::LabelRange {
                index,
                span: label.span,
                source_len: full_source.len(),
            });
        }
    }
    if full_source.is_empty() {
        return Err(DiagnosticSourceError::EmptySource);
    }

    let (line, column) = line_map.line_col_of(primary.start);
    let mut min_byte = primary.start;
    let mut max_byte = primary.end;
    // Zero-width positions belong to their own line (including final empty EOF).
    let mut last_line = line_map.line_of(if primary.is_empty() {
        primary.start
    } else {
        primary.end - 1
    });
    for label in &error.labels {
        min_byte = min_byte.min(label.span.start);
        max_byte = max_byte.max(label.span.end);
        last_line = last_line.max(line_map.line_of(if label.span.is_empty() {
            label.span.start
        } else {
            label.span.end - 1
        }));
    }
    let first_line = line_map.line_of(min_byte);
    let context_start = line_map.line_start(first_line);
    let mut context_end = line_map.line_end(last_line, source_len);
    // Only strip an unreferenced newline. Never shorten an admitted span.
    if context_end > max_byte && full_source.as_bytes().get(context_end - 1) == Some(&b'\n') {
        context_end -= 1;
    }
    let context_text = &full_source[context_start..context_end];
    let mapped = if requires_raw_display(&error.code) {
        PlainDisplayResult::unformatted(context_text)
    } else {
        process_for_plain_display_mapped(context_text)
    };
    let project_span = |span: Span| {
        let projected = mapped.project_range(span.start - context_start, span.end - context_start);
        DisplaySpan::from_projected(if span.is_empty() {
            projected.collapsed()
        } else {
            projected
        })
    };
    let display_span = project_span(primary);
    let labels = error
        .labels
        .iter()
        .map(|label| DiagnosticLabel {
            span: project_span(label.span),
            message: label.message.clone(),
        })
        .collect();
    let context = DiagnosticContext {
        source_text: mapped.into_text(),
        span: display_span,
        first_line: first_line + 1,
    };
    Ok(DiagnosticPresentation::new(
        primary,
        LineColumn::from_zero_based(line, column),
        context,
        labels,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiagnosticLocation, ErrorContext, ErrorLabel, Severity};

    fn render(source: &str, start: usize, end: usize, code: ErrorCode) -> SourceDiagnostic {
        let raw = ParseError::new(
            code,
            Severity::Error,
            DiagnosticLocation::from_offsets(start, end),
            ErrorContext::new("partial", crate::Span::new(0, 0), "original"),
            "Test error",
        );
        SourceDiagnostic::with_index(raw, &SourceIndex::new(source))
    }

    #[test]
    fn test_enhance_errors_with_empty_context() {
        let source = "line 1\nline 2 with error\nline 3\n";
        let raw = ParseError::build(ErrorCode::UnknownError)
            .at(14, 19)
            .message("Test error")
            .finish();
        let rendered = SourceDiagnostic::with_index(raw.clone(), &SourceIndex::new(source));
        assert_eq!(rendered.original(), &raw);
        let p = rendered.presentation().expect("source admission");
        assert_eq!((p.position().line.get(), p.position().column.get()), (2, 8));
        assert_eq!(p.context().source_text(), "line 2 with error");
        assert_eq!(p.context().first_line(), 2);
        assert_eq!(p.context().span().to_range(), 7..12);
    }

    #[test]
    fn test_enhance_errors_replaces_partial_context_with_full_line() {
        let rendered = render(
            "line 1\nline 2 with error\nline 3\n",
            14,
            19,
            ErrorCode::UnknownError,
        );
        let context = rendered.context().expect("source admission");
        assert_eq!(context.source_text(), "line 2 with error");
        assert_eq!(
            rendered
                .original()
                .context
                .as_ref()
                .expect("producer context")
                .source_text,
            "partial"
        );
        assert_eq!(
            rendered
                .original()
                .context
                .as_ref()
                .expect("producer context")
                .found,
            "original"
        );
    }

    #[test]
    fn test_enhance_errors_handles_first_line() {
        let rendered = render("error first\nsecond\n", 0, 5, ErrorCode::UnknownError);
        let p = rendered.presentation().expect("source admission");
        assert_eq!((p.position().line.get(), p.position().column.get()), (1, 1));
        assert_eq!(p.context().source_text(), "error first");
        assert_eq!(p.context().first_line(), 1);
    }

    #[test]
    fn test_enhance_errors_handles_last_line() {
        let source = "first\nlast error";
        let start = source.find("error").expect("error");
        let rendered = render(source, start, source.len(), ErrorCode::UnknownError);
        let p = rendered.presentation().expect("source admission");
        assert_eq!((p.position().line.get(), p.position().column.get()), (2, 6));
        assert_eq!(p.context().source_text(), "last error");
        assert_eq!(p.context().first_line(), 2);
    }

    #[test]
    fn test_enhance_preserves_raw_formatting_for_marker_errors() {
        let source = "hello \u{0002}\u{0001}bad\u{0002}\u{0002} world\n";
        let rendered = render(source, 6, 8, ErrorCode::UnmatchedUnderlineBegin);
        let context = rendered.context().expect("source admission");
        assert_eq!(context.source_text(), source.trim_end_matches('\n'));
        assert_eq!(context.span().to_range(), 6..8);
    }

    #[test]
    fn test_enhance_applies_formatting_for_normal_errors() {
        let source = "hello \u{0002}\u{0001}bad\u{0002}\u{0002} world\n";
        let rendered = render(source, 14, 19, ErrorCode::UnknownError);
        let context = rendered.context().expect("source admission");
        assert_eq!(context.source_text(), "hello bad world");
        assert_eq!(context.span().to_range(), 10..15);
    }

    #[test]
    fn test_enhance_multi_line_context_with_label() {
        let source = "line 1\n*CHI:\thello world .\n%wor:\thello world\nline 4\n";
        let main = source.find("*CHI:").expect("main tier");
        let wor = source.find("%wor:").expect("wor tier");
        let end = source[wor..].find('\n').expect("wor end") + wor;
        let raw = ParseError::build(ErrorCode::PhoCountMismatchTooFew)
            .at(main, wor - 1)
            .message("Alignment mismatch")
            .label(ErrorLabel::new(Span::new(wor, end), "%wor tier"))
            .finish();
        let rendered = SourceDiagnostic::with_index(raw.clone(), &SourceIndex::new(source));
        let p = rendered.presentation().expect("source admission");
        let text = p.context().source_text();
        assert!(text.contains("*CHI:") && text.contains("%wor:"));
        assert_eq!(p.context().first_line(), 2);
        let label_text = &text[p.labels()[0].span().to_range()];
        assert!(label_text.starts_with("%wor:") && label_text.ends_with("hello world"));
        assert!(
            !label_text.contains('\t'),
            "labels use formatted display bytes, not raw tabbed offsets"
        );
        assert!(p.labels()[0].span().start() > p.context().span().start());
        assert_eq!(rendered.original(), &raw, "absolute labels remain intact");
    }

    #[test]
    fn test_enhance_label_before_primary() {
        let source = "alpha\nbeta\ngamma error\ndelta\n";
        let gamma = source.find("gamma").expect("gamma");
        let raw = ParseError::build(ErrorCode::UnknownError)
            .at(gamma, gamma + 11)
            .message("Error with backward label")
            .label(ErrorLabel::new(Span::new(0, 5), "related"))
            .finish();
        let rendered = SourceDiagnostic::with_index(raw, &SourceIndex::new(source));
        let p = rendered.presentation().expect("source admission");
        assert_eq!(p.context().source_text(), "alpha\nbeta\ngamma error");
        assert_eq!(p.context().first_line(), 1);
        assert_eq!(p.labels()[0].span().to_range(), 0..5);
    }

    #[test]
    fn source_and_line_index_must_not_be_mixed() {
        let rendered = render("aaéb", 4, 5, ErrorCode::UnmatchedUnderlineBegin);
        let p = rendered.presentation().expect("source admission");
        assert_eq!((p.position().line.get(), p.position().column.get()), (1, 5));
    }
}
