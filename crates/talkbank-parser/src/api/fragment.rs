//! Own synthetic source and the coordinate translation derived while building it.

use talkbank_model::{
    ErrorSink, OffsetAdjustingErrorSink, ParseError, RebasedErrorSink, SpanShift,
};

/// One synthetic document, its borrowed caller input, and its document origin.
/// The constructor records the input start from the assembled source, so model
/// and diagnostic consumers never recalculate a synthetic prefix length.
pub(crate) struct WrappedFragment<'input> {
    source: String,
    input: &'input str,
    input_start: usize,
    document_offset: usize,
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

    pub(crate) fn rebase<T: SpanShift>(&self, mut value: T) -> T {
        value.shift_spans_after(0, self.document_offset as i32 - self.input_start as i32);
        value
    }

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
    fn report(&self, error: ParseError) {
        let document = RebasedErrorSink::new(self.inner, self.fragment.document_offset as i32);
        OffsetAdjustingErrorSink::new(&document, self.fragment.input_start, self.fragment.input)
            .report(error);
    }
}
