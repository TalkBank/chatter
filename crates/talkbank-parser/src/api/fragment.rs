//! Own synthetic source and the coordinate translation derived while building it.

use talkbank_model::{ErrorSink, FragmentSource, ParseError, ParseErrors, Span, SpanShift};

/// One synthetic document, its borrowed caller input, and its document origin.
/// The constructor records the input start from the assembled source, so model
/// and diagnostic consumers never recalculate a synthetic prefix length.
pub(crate) struct WrappedFragment<'input> {
    source: String,
    input: FragmentSource<'input>,
    input_start: usize,
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
    ) -> Result<Self, ParseErrors> {
        let admitted = FragmentSource::new(input, document_offset)
            .map_err(|error| ParseErrors::from(vec![error.into_diagnostic()]))?;
        let capacity = prefixes
            .iter()
            .try_fold(input.len(), |size, prefix| size.checked_add(prefix.len()))
            .and_then(|size| size.checked_add(suffix.len()));
        let Some(capacity) = capacity.filter(|size| *size <= u32::MAX as usize) else {
            return Err(ParseErrors::from(vec![ParseError::at_span(
                talkbank_model::ErrorCode::ParseFailed,
                talkbank_model::Severity::Error,
                Span::DUMMY,
                "Synthetic fragment source exceeds the 32-bit source coordinate space",
            )]));
        };
        let mut source = String::with_capacity(capacity);
        for prefix in prefixes {
            source.push_str(prefix);
        }
        let input_start = source.len();
        source.push_str(input);
        source.push_str(suffix);
        Ok(Self {
            source,
            input: admitted,
            input_start,
        })
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn input(&self) -> &str {
        self.input.input()
    }

    /// The node accounts for all caller text; only surrounding whitespace may
    /// remain outside it, or extend from it into a synthetic line terminator.
    pub(crate) fn require_complete_input(
        &self,
        range: std::ops::Range<usize>,
    ) -> Result<(), FragmentCoverageError> {
        let input_end = self.input_start + self.input.input().len();
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

    pub(crate) fn rebase<T: SpanShift>(&self, value: T) -> T {
        self.input.rebase_from(value, self.input_start as u32)
    }

    fn document_span(&self, span: Span) -> Span {
        if span.is_dummy() {
            return span;
        }
        let start = self.input_start as u32;
        let end = start + self.input.input().len() as u32;
        self.input.rebase_from(
            Span::new(span.start.clamp(start, end), span.end.clamp(start, end)),
            start,
        )
    }

    fn input_span(&self, span: Span) -> Span {
        let prefix = self.input_start as u32;
        let end = self.input.input().len() as u32;
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
        error.location.span = self.fragment.document_span(error.location.span);
        for label in &mut error.labels {
            label.span = self.fragment.document_span(label.span);
        }
        if let Some(context) = &mut error.context
            && context.source_text == self.fragment.source
        {
            context.span = self.fragment.input_span(context.span);
            context.source_text = self.fragment.input.input().to_owned();
        }
        self.inner.report(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{
        ErrorCode, ErrorCollector, ErrorContext, ErrorLabel, Severity, SourceLocation, Span,
    };

    #[test]
    fn wrapper_rebasing_preserves_known_zero_width_locations() {
        let fragment = WrappedFragment::new(&["prefix"], "word", "suffix", 200).unwrap();
        assert_eq!(fragment.rebase(Span::at(6)), Span::at(200));
        assert_eq!(fragment.rebase(Span::DUMMY), Span::DUMMY);
        assert_eq!(fragment.document_span(Span::at(6)), Span::at(200));
        assert_eq!(fragment.document_span(Span::DUMMY), Span::DUMMY);
    }

    #[test]
    fn projects_full_source_context_and_related_labels_even_for_long_inputs() {
        let input = "word ".repeat(100);
        let fragment = WrappedFragment::new(&["@Begin\n"], &input, "\n@End", 200).unwrap();
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
        let fragment = WrappedFragment::new(&["prefix"], "x", "suffix", 200).unwrap();
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
