//! Own synthetic source and the coordinate translation derived while building it.
//!
//! # Three coordinate spaces, two of them typed
//!
//! A fragment parse touches three byte-offset spaces: the caller's DOCUMENT
//! (where the caller says its input sits), the caller's INPUT (offsets into
//! the string it handed over, which is where diagnostic snippets live), and
//! the SYNTHETIC source this wrapper assembles around that input so the
//! grammar sees a complete line or file. Node spans from the tree, and every
//! model value lowered from it, are synthetic until this wrapper moves them.
//!
//! [`FragmentSource`] is the typed witness for the document space, admitted
//! once at the public boundary and handed to [`WrappedFragment::new`], which
//! never re-admits it. [`Synthetic`] is the witness for the synthetic space: a
//! model or span lowered from the wrapper's parse is born inside it
//! ([`WrappedFragment::parse_file`], [`ParsedFragment::lowered`],
//! [`ParsedFragment::node_span`]) and can only leave through
//! [`WrappedFragment::rebase`] (or its clamping variants), so forgetting the
//! move, or moving twice, does not compile. Input-space coordinates arise only
//! where a snippet is cut from the caller's text: [`WrappedFragment::input_span`]
//! and the diagnostic sink, which rewrites a snippet span when the snippet is
//! the synthetic source itself.

use crate::generated_traversal::ParsedSource;
use crate::parser::TreeSitterParser;
use talkbank_model::{
    ChatFile, DependentTier, DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink,
    FragmentSource, MainTier, ParseError, ParseErrors, Severity, Span, SpanShift,
};

/// A value whose spans are in a wrapper's synthetic coordinate space.
///
/// Minted only inside this module ([`WrappedFragment::parse_file`],
/// [`ParsedFragment::lowered`], [`ParsedFragment::node_span`]); narrowed with
/// [`Self::map`], [`Self::project`], [`Self::try_map`] and [`Self::transpose`],
/// which keep the label; consumed by the wrapper's `rebase*` methods and its
/// span queries, which are the only ways out. There is deliberately no
/// accessor that lends the bare value: a consumer that needs a fact about a
/// synthetic value projects the fact and keeps the label.
#[must_use = "a synthetic value must be rebased into the caller's coordinates"]
pub(crate) struct Synthetic<T>(T);

impl<T> Synthetic<T> {
    /// Narrow or transform the value; the result is still synthetic.
    pub(crate) fn map<U>(self, f: impl FnOnce(T) -> U) -> Synthetic<U> {
        Synthetic(f(self.0))
    }

    /// Derive a second synthetic value from a borrowed view of this one.
    pub(crate) fn project<U>(&self, f: impl FnOnce(&T) -> U) -> Synthetic<U> {
        Synthetic(f(&self.0))
    }

    /// Lift a fallible narrowing; the error carries no coordinates.
    pub(crate) fn try_map<U, E>(
        self,
        f: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<Synthetic<U>, E> {
        f(self.0).map(Synthetic)
    }
}

impl<T> Synthetic<Option<T>> {
    /// `Synthetic<Option<T>>` to `Option<Synthetic<T>>`: absence has no space.
    pub(crate) fn transpose(self) -> Option<Synthetic<T>> {
        self.0.map(Synthetic)
    }
}

/// One synthetic document around an admitted caller input.
///
/// The constructor records where the input starts in the assembled source, so
/// no consumer recalculates a prefix length, and it takes the caller's
/// [`FragmentSource`] rather than an `(input, offset)` pair, so the document
/// range is admitted exactly once, at the public boundary.
pub(crate) struct WrappedFragment<'input> {
    source: String,
    input: FragmentSource<'input>,
    input_start: usize,
}

/// Wrapper-owned bytes that cannot complete a CRLF across the input boundary.
/// A caller's final CR already terminates its line; a leading synthetic LF
/// would otherwise become part of that same token and extend source locations.
struct SeparatedSuffix<'suffix>(&'suffix str);

impl<'suffix> SeparatedSuffix<'suffix> {
    fn after(input: &str, suffix: &'suffix str) -> Self {
        let suffix = if input.ends_with('\r') {
            suffix.strip_prefix('\n').unwrap_or(suffix)
        } else {
            suffix
        };
        Self(suffix)
    }
}

/// A wrapper paired with the tree produced from its own assembled source.
/// Only `WrappedFragment::parse` constructs this association.
pub(crate) struct ParsedFragment<'source, 'input> {
    parsed: ParsedSource<'source>,
    fragment: &'source WrappedFragment<'input>,
}

impl<'source, 'input> ParsedFragment<'source, 'input> {
    pub(crate) fn parsed_source(&self) -> &ParsedSource<'source> {
        &self.parsed
    }

    pub(crate) fn fragment(&self) -> &'source WrappedFragment<'input> {
        self.fragment
    }

    /// Label a value the caller lowered from this parse as synthetic.
    ///
    /// The caller asserts provenance: `value` must have been built from nodes
    /// of this tree (a header selected in it, a main tier converted from it).
    /// No type can prove that for an arbitrary `T`, so this is the one honest
    /// mint and its callers are the two lowerers in this crate; a value from
    /// another space handed in here would be moved wrongly by `rebase`.
    pub(crate) fn lowered<T>(&self, value: T) -> Synthetic<T> {
        Synthetic(value)
    }

    /// The byte range of a node of this tree, as a synthetic span.
    pub(crate) fn node_span(&self, node: tree_sitter::Node<'_>) -> Synthetic<Span> {
        Synthetic(crate::parser::node_span::span_of(node))
    }
}

/// Why a selected CST range cannot represent the complete caller input.
pub(crate) enum FragmentCoverageError {
    OutsideInput,
    Incomplete,
}

impl<'input> WrappedFragment<'input> {
    /// Assemble `prefixes`, the admitted input and `suffix` into one source.
    ///
    /// # Errors
    ///
    /// When the assembled source would exceed the 32-bit coordinate space the
    /// grammar can address; no document location is fabricated for it.
    pub(crate) fn new(
        prefixes: &[&str],
        input: FragmentSource<'input>,
        suffix: &str,
    ) -> Result<Self, ParseErrors> {
        let text = input.input();
        let SeparatedSuffix(suffix) = SeparatedSuffix::after(text, suffix);
        let capacity = prefixes
            .iter()
            .try_fold(text.len(), |size, prefix| size.checked_add(prefix.len()))
            .and_then(|size| size.checked_add(suffix.len()));
        let Some(capacity) = capacity.filter(|size| *size <= u32::MAX as usize) else {
            return Err(ParseErrors::from(vec![ParseError::at_location(
                ErrorCode::ParseFailed,
                Severity::Error,
                DiagnosticLocation::Unlocated,
                "Synthetic fragment source exceeds the 32-bit source coordinate space",
            )]));
        };
        let mut source = String::with_capacity(capacity);
        for prefix in prefixes {
            source.push_str(prefix);
        }
        let input_start = source.len();
        source.push_str(text);
        source.push_str(suffix);
        Ok(Self {
            source,
            input,
            input_start,
        })
    }

    /// [`Self::new`] for the input-relative entry points, whose caller text
    /// IS the document: admitted at origin zero.
    ///
    /// # Errors
    ///
    /// Admission refuses an input too large to parse; assembly refuses a
    /// source too large to address.
    pub(crate) fn local(
        prefixes: &[&str],
        input: &'input str,
        suffix: &str,
    ) -> Result<Self, ParseErrors> {
        let admitted = FragmentSource::new(input, 0)
            .map_err(|error| ParseErrors::from(vec![error.into_diagnostic()]))?;
        Self::new(prefixes, admitted, suffix)
    }

    pub(crate) fn parse<'source>(
        &'source self,
        parser: &TreeSitterParser,
    ) -> Result<ParsedFragment<'source, 'input>, ParseErrors> {
        Ok(ParsedFragment {
            parsed: parser.parse_source_incremental(&self.source, None)?,
            fragment: self,
        })
    }

    /// Parse the synthetic source as a whole CHAT file, streaming diagnostics
    /// through `errors` (already translated into the caller's coordinates).
    /// The file's spans are synthetic until rebased.
    pub(crate) fn parse_file(
        &self,
        parser: &TreeSitterParser,
        errors: &impl ErrorSink,
    ) -> Synthetic<ChatFile> {
        Synthetic(parser.parse_chat_file_streaming(&self.source, &self.error_sink(errors)))
    }

    /// Does the node account for the whole synthetic source, every byte of
    /// prefix, input and suffix?
    pub(crate) fn covers_whole_source(&self, span: Synthetic<Span>) -> bool {
        span.0 == Span::new(0, self.source.len())
    }

    /// The caller's whole input, in the caller's document coordinates.
    pub(crate) fn input_document_span(&self) -> Span {
        self.input.whole_span()
    }

    /// A diagnostic about the caller's whole input: placed at its document
    /// range, quoting the whole input with `found` as the offending text. The
    /// one owner of this shape; the lowerers used to build it by hand, each
    /// at `0..len`, which is right only when the document origin is zero.
    pub(crate) fn whole_input_error(
        &self,
        code: ErrorCode,
        severity: Severity,
        found: &str,
        message: impl Into<String>,
    ) -> ParseError {
        let input = self.input.input();
        ParseError::new(
            code,
            severity,
            DiagnosticLocation::new(self.input_document_span()),
            ErrorContext::new(input, Span::new(0, input.len()), found),
            message,
        )
    }

    /// The assembled synthetic text, for tests that pin the envelope shape.
    #[cfg(test)]
    pub(crate) fn synthetic_source(&self) -> &str {
        &self.source
    }

    pub(crate) fn input(&self) -> &str {
        self.input.input()
    }

    /// The node accounts for all caller text; only surrounding whitespace may
    /// remain outside it, or extend from it into a synthetic line terminator.
    pub(crate) fn require_complete_input(
        &self,
        span: Synthetic<Span>,
    ) -> Result<(), FragmentCoverageError> {
        let range = span.0.to_range();
        let input_end = self.input_start + self.input.input().len();
        if !(self.input_start..input_end).contains(&range.start) {
            return Err(FragmentCoverageError::OutsideInput);
        }
        // `span` is a `Span`, which cannot be inverted, so `range.end` is at
        // least `range.start` here without a check.
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

    /// A node may begin in the wrapper-owned prefix (the `%label:` or
    /// `@Label:` the wrapper supplied), but must still account for every
    /// non-whitespace byte of caller input.
    pub(crate) fn require_complete_labelled_input(
        &self,
        span: Synthetic<Span>,
    ) -> Result<(), FragmentCoverageError> {
        let range = span.0.to_range();
        let input_end = self.input_start + self.input.input().len();
        if range.start > range.end
            || range.end > self.source.len()
            || range.start > input_end
            || range.end < self.input_start
        {
            return Err(FragmentCoverageError::OutsideInput);
        }
        let Some((leading, trailing)) = self
            .source
            .get(self.input_start..range.start.max(self.input_start))
            .zip(self.source.get(range.end.min(input_end)..input_end))
        else {
            return Err(FragmentCoverageError::OutsideInput);
        };
        if leading.trim().is_empty() && trailing.trim().is_empty() {
            Ok(())
        } else {
            Err(FragmentCoverageError::Incomplete)
        }
    }

    /// Move a synthetic value into the caller's document coordinates. The
    /// only exit from [`Synthetic`], together with the clamping variants below.
    pub(crate) fn rebase<T: SpanShift>(&self, value: Synthetic<T>) -> Result<T, ParseErrors> {
        self.input
            .rebase_from(value.0, self.input_start)
            .map_err(|error| ParseErrors::from(vec![error.into_diagnostic()]))
    }

    /// A synthetic span clamped to the caller's input and moved into the
    /// caller's document. An empty input needs no special case: the clamp
    /// collapses the span to the input start and the move lands it at the
    /// origin, which is the admitted whole range of an empty input.
    fn document_span(&self, span: Span) -> Span {
        self.input.project_span_from(span, self.input_start)
    }

    /// A wrapper-coordinate span clamped to the caller's input, still in
    /// wrapper coordinates: the label the wrapper added lies outside it.
    fn clamp_to_input(&self, span: Span) -> Span {
        let start = self.input_start;
        let end = start + self.input.input().len();
        Span::new(span.start().clamp(start, end), span.end().clamp(start, end))
    }

    /// A synthetic span clamped into the caller's input and expressed in
    /// INPUT coordinates, for cutting a snippet out of the caller's text.
    pub(crate) fn input_span(&self, span: Synthetic<Span>) -> Span {
        self.input_span_of(span.0)
    }

    fn input_span_of(&self, span: Span) -> Span {
        let prefix = self.input_start;
        let end = self.input.input().len();
        Span::new(
            span.start().saturating_sub(prefix).min(end),
            span.end().saturating_sub(prefix).min(end),
        )
    }

    /// Project a model container that may include a wrapper-owned tier label.
    /// The tier's own span is clamped to the caller's input first (the label
    /// the wrapper added lies outside it), then the whole tier moves into the
    /// caller's coordinates; an unlocated tier stays unlocated. A known empty
    /// diagnostic is separately placed by `document_span`.
    pub(crate) fn rebase_tier(
        &self,
        tier: Synthetic<DependentTier>,
    ) -> Result<DependentTier, ParseErrors> {
        self.rebase(self.project_tier(tier))
    }

    /// Exclude the wrapper's label/newline before selecting a tier payload.
    pub(crate) fn project_tier(&self, tier: Synthetic<DependentTier>) -> Synthetic<DependentTier> {
        tier.map(|tier| tier.map_location(|span| self.clamp_to_input(span)))
    }

    /// An utterance may inherit the wrapper's final newline in container
    /// extents. Project those extents only; every payload span remains checked.
    pub(crate) fn rebase_utterance(
        &self,
        utterance: Synthetic<talkbank_model::SingleUtterance>,
    ) -> Result<talkbank_model::SingleUtterance, ParseErrors> {
        self.rebase(utterance.map(|single| {
            single.map_utterance(|mut utterance| {
                utterance.main = self.project_main(utterance.main);
                utterance.dependent_tiers = utterance
                    .dependent_tiers
                    .into_iter()
                    .map(|mut entry| {
                        entry.tier = entry.tier.map_location(|span| self.clamp_to_input(span));
                        entry
                    })
                    .collect();
                utterance
            })
        }))
    }

    fn project_main(&self, mut main: MainTier) -> MainTier {
        main.location = main.location.map(|span| self.clamp_to_input(span));
        main.content.location = main.content.location.map(|span| self.clamp_to_input(span));
        main
    }

    /// Project a main tier parsed inside a wrapper: its own extent and its
    /// content's extent are clamped to the caller's input (the synthetic line
    /// terminator lies outside it), then the whole tier moves; unlocated
    /// stays unlocated.
    pub(crate) fn rebase_main_tier(
        &self,
        main: Synthetic<MainTier>,
    ) -> Result<MainTier, ParseErrors> {
        self.rebase(main.map(|main| self.project_main(main)))
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
        error.location = error
            .location
            .rebased(|span| self.fragment.document_span(span));
        for label in &mut error.labels {
            label.span = self.fragment.document_span(label.span);
        }
        if let Some(context) = &mut error.context
            && context.source_text == self.fragment.source
        {
            context.span = self.fragment.input_span_of(context.span);
            context.source_text = self.fragment.input.input().to_owned();
        }
        self.inner.report(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{ErrorCollector, ErrorLabel};

    fn wrapped<'input>(
        prefixes: &[&str],
        input: &'input str,
        suffix: &str,
        origin: usize,
    ) -> WrappedFragment<'input> {
        WrappedFragment::new(
            prefixes,
            FragmentSource::new(input, origin).expect("admitted"),
            suffix,
        )
        .expect("assembled")
    }

    /// A point at the first input byte and an unlocated value both survive
    /// the move; an empty input's diagnostics land at its admitted origin
    /// without a special case.
    #[test]
    fn wrapper_rebasing_preserves_known_zero_width_locations() {
        let fragment = wrapped(&["prefix"], "word", "suffix", 200);
        assert_eq!(
            fragment
                .rebase(Synthetic(Span::at(6)))
                .expect("contained point"),
            Span::at(200)
        );
        assert_eq!(
            fragment
                .rebase(Synthetic(talkbank_model::Location::Unlocated))
                .expect("unlocated value"),
            talkbank_model::Location::Unlocated
        );
        assert_eq!(fragment.document_span(Span::at(6)), Span::at(200));
        let empty = wrapped(&["prefix"], "", "suffix", 200);
        assert_eq!(empty.document_span(Span::new(0, 12)), Span::at(200));
        assert_eq!(empty.input_document_span(), Span::at(200));
    }

    #[test]
    fn projects_full_source_context_and_related_labels_even_for_long_inputs() {
        let input = "word ".repeat(100);
        let fragment = wrapped(&["@Begin\n"], &input, "\n@End", 200);
        let mut error = ParseError::new(
            ErrorCode::UnparsableContent,
            Severity::Error,
            DiagnosticLocation::from_offsets(12, 16),
            ErrorContext::new(&fragment.source, Span::new(12, 16), "word"),
            "example",
        );
        error
            .labels
            .push(ErrorLabel::new(Span::new(7, 11), "related word"));
        let errors = ErrorCollector::new();
        fragment.error_sink(&errors).report(error);
        let errors = errors.into_vec();
        assert_eq!(
            errors[0].location.span().expect("located diagnostic"),
            Span::new(205, 209)
        );
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
        let fragment = wrapped(&["prefix"], "x", "suffix", 200);
        let context = ErrorContext::new("another source", Span::new(0, 7), "another");
        let error = ParseError::new(
            ErrorCode::UnparsableContent,
            Severity::Error,
            DiagnosticLocation::from_offsets(6, 7),
            context.clone(),
            "example",
        );
        let errors = ErrorCollector::new();
        fragment.error_sink(&errors).report(error);
        let errors = errors.into_vec();
        assert_eq!(
            errors[0].location.span().expect("located diagnostic"),
            Span::new(200, 201)
        );
        assert_eq!(errors[0].context, Some(context));
    }

    /// The whole-input diagnostic is placed in the caller's document, whatever
    /// the wrapper's prefix, and quotes the caller's text, not the wrapper's.
    #[test]
    fn whole_input_errors_are_placed_at_the_callers_document_range() {
        let fragment = wrapped(&["@Begin\n"], "abc", "\n@End", 200);
        let error = fragment.whole_input_error(
            ErrorCode::ParseFailed,
            Severity::Error,
            "header",
            "example",
        );
        assert_eq!(
            error.location.span().expect("located diagnostic"),
            Span::new(200, 203)
        );
        assert_eq!(
            error
                .context
                .as_ref()
                .map(|context| (context.source_text.as_str(), context.span)),
            Some(("abc", Span::new(0, 3)))
        );
    }
}
