//! Own synthetic source and the coordinate translation derived while building it.

use talkbank_model::{ErrorSink, ParseError, RebasedErrorSink, Span, SpanShift};

/// One synthetic document, its borrowed caller input, and its document origin.
/// The constructor records the input start from the assembled source, so model
/// and diagnostic consumers never recalculate a synthetic prefix length.
pub(crate) struct WrappedFragment<'input> {
    source: String,
    input: &'input str,
    input_start: usize,
    document_offset: usize,
}

/// Why a selected CST range cannot represent the complete caller input.
pub(crate) enum FragmentCoverageError {
    OutsideInput,
    Incomplete,
}

impl<'input> WrappedFragment<'input> {
    pub(crate) fn new(
        prefixes: &[&str],
        input: &'input str,
        suffix: &str,
        document_offset: usize,
    ) -> Self {
        let mut source = String::with_capacity(
            prefixes.iter().map(|prefix| prefix.len()).sum::<usize>() + input.len() + suffix.len(),
        );
        for prefix in prefixes {
            source.push_str(prefix);
        }
        let input_start = source.len();
        source.push_str(input);
        source.push_str(suffix);
        Self {
            source,
            input,
            input_start,
            document_offset,
        }
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn input(&self) -> &str {
        self.input
    }

    /// The node accounts for all caller text; only surrounding whitespace may
    /// remain outside it, or extend from it into a synthetic line terminator.
    pub(crate) fn require_complete_input(
        &self,
        range: std::ops::Range<usize>,
    ) -> Result<(), FragmentCoverageError> {
        let input_end = self.input_start + self.input.len();
        if !(self.input_start..input_end).contains(&range.start) {
            return Err(FragmentCoverageError::OutsideInput);
        }
        if range.end < range.start {
            return Err(FragmentCoverageError::Incomplete);
        }
        let leading = self.source.get(self.input_start..range.start);
        let trailing = self
            .source
            .get(range.end.min(input_end)..range.end.max(input_end));
        if leading.is_some_and(|text| text.trim().is_empty())
            && trailing.is_some_and(|text| text.trim().is_empty())
        {
            Ok(())
        } else {
            Err(FragmentCoverageError::Incomplete)
        }
    }

    pub(crate) fn rebase<T: SpanShift>(&self, mut value: T) -> T {
        value.shift_spans_after(0, self.document_offset as i32 - self.input_start as i32);
        value
    }

    fn input_span(&self, span: Span) -> Span {
        let prefix = self.input_start as u32;
        let end = self.input.len() as u32;
        Span::new(
            span.start.saturating_sub(prefix).min(end),
            span.end.saturating_sub(prefix).min(end),
        )
    }

    /// Translate parser diagnostics before display enhancement rewrites labels.
    pub(crate) fn error_sink<'a, S: ErrorSink>(&'a self, inner: &'a S) -> impl ErrorSink + 'a {
        FragmentErrorSink {
            fragment: self,
            inner,
        }
    }
}

struct FragmentErrorSink<'a, 'input, S> {
    fragment: &'a WrappedFragment<'input>,
    inner: &'a S,
}

impl<S: ErrorSink> ErrorSink for FragmentErrorSink<'_, '_, S> {
    fn report(&self, mut error: ParseError) {
        error.location.span = self.fragment.input_span(error.location.span);
        for label in &mut error.labels {
            label.span = self.fragment.input_span(label.span);
        }
        if let Some(context) = &mut error.context
            && context.source_text == self.fragment.source
        {
            context.span = self.fragment.input_span(context.span);
            context.source_text = self.fragment.input.to_owned();
        }
        let document = RebasedErrorSink::new(self.inner, self.fragment.document_offset as i32);
        document.report(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{
        ErrorCode, ErrorCollector, ErrorContext, ErrorLabel, Severity, SourceLocation, Span,
    };

    #[test]
    fn projects_full_source_context_and_related_labels_even_for_long_inputs() {
        let input = "word ".repeat(100);
        let fragment = WrappedFragment::new(&["@Begin\n"], &input, "\n@End", 200);
        let mut error = ParseError::new(
            ErrorCode::UnparsableContent,
            Severity::Error,
            SourceLocation::from_offsets(12, 16),
            ErrorContext::new(fragment.source(), 12..16, "word"),
            "example",
        );
        error
            .labels
            .push(ErrorLabel::new(Span::new(7, 11), "related word"));
        let errors = ErrorCollector::new();
        fragment.error_sink(&errors).report(error);
        let errors = errors.into_vec();
        assert_eq!(errors[0].location.span, Span::new(205, 209));
        assert_eq!(errors[0].labels[0].span, Span::new(200, 204));
        assert_eq!(
            errors[0]
                .context
                .as_ref()
                .map(|context| (context.source_text.as_str(), context.span)),
            Some((input.as_str(), Span::new(5, 9)))
        );
    }

    #[test]
    fn preserves_independent_context_regardless_of_its_length() {
        let fragment = WrappedFragment::new(&["prefix"], "x", "suffix", 200);
        let context = ErrorContext::new("another source", 0..7, "another");
        let error = ParseError::new(
            ErrorCode::UnparsableContent,
            Severity::Error,
            SourceLocation::from_offsets(6, 7),
            context.clone(),
            "example",
        );
        let errors = ErrorCollector::new();
        fragment.error_sink(&errors).report(error);
        let errors = errors.into_vec();
        assert_eq!(errors[0].location.span, Span::new(200, 201));
        assert_eq!(errors[0].context, Some(context));
    }
}
