//! Admission and lowering of one complete wrapped header fragment.

use super::super::header_parser::{parse_header_node, parse_pre_begin_header};
use super::finder::{HeaderLookupError, SelectedHeader, find_header_node_in_tree, first_child};
use crate::api::fragment::{FragmentCoverageError, ParsedFragment, Synthetic};
use crate::error::{
    DiagnosticLocation, ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError,
    ParseErrors, ParseResult, Severity,
};
use crate::generated_traversal::{AsRawNode, FullDocumentChild1Choice};
use crate::model::Header;
use crate::node_types::*;
use crate::parser::node_span::span_of;
use talkbank_model::ParseOutcome;

/// How much of the header line the caller supplied.
#[derive(Clone, Copy)]
pub(crate) enum HeaderCoverage {
    /// The caller's input is the whole header line; the node must cover it
    /// and nothing beyond surrounding whitespace.
    WholeLine,
    /// The wrapper supplied the `@Label:\t` prefix; the node may start in it
    /// but must still account for every non-whitespace byte of caller input.
    LabelSupplied,
}

/// Lowering owns the node and the source whose complete input it represents.
/// No caller can lower a header merely because it starts inside the input.
///
/// Every diagnostic this type returns or reports is in the caller's DOCUMENT
/// coordinates: lowering faults pass through the wrapper's sink, and
/// admission refusals are placed at the caller's whole input. A caller
/// reports them as they are, without a second rebase.
pub(crate) struct HeaderFragment<'tree, 'source, 'input> {
    header: SelectedHeader<'tree, 'source>,
    parsed: &'tree ParsedFragment<'source, 'input>,
}

impl<'tree, 'source, 'input> HeaderFragment<'tree, 'source, 'input> {
    pub(crate) fn admit(
        parsed: &'tree ParsedFragment<'source, 'input>,
        header_index: usize,
        coverage: HeaderCoverage,
    ) -> ParseResult<Self> {
        let fragment = parsed.fragment();
        let header = (|| -> Result<_, HeaderLookupError> {
            let root = parsed.parsed_source().root()?;
            let root = if root.raw_node().kind() == SOURCE_FILE {
                first_child(root)?
                    .filter(|child| child.raw_node().kind() == FULL_DOCUMENT)
                    .unwrap_or(root)
            } else {
                root
            };
            find_header_node_in_tree(root, header_index)
        })()
        .map_err(|error| {
            let code = match &error {
                HeaderLookupError::NotFound { .. } => ErrorCode::TierValidationError,
                HeaderLookupError::Binding(_) => ErrorCode::TreeParsingError,
            };
            ParseErrors::from(vec![
                fragment
                    .whole_input_error(code, Severity::Error, "header", error.to_string())
                    .with_suggestion(
                        "Check that all header lines are well-formed and appear before utterances",
                    ),
            ])
        })?;
        let node = header.source().raw_node();
        let span = parsed.node_span(node);
        match coverage {
            HeaderCoverage::WholeLine => fragment.require_complete_input(span),
            HeaderCoverage::LabelSupplied => fragment.require_complete_labelled_input(span),
        }
        .map_err(|failure| {
            let error = match failure {
                FragmentCoverageError::OutsideInput => fragment
                    .whole_input_error(
                        ErrorCode::TierValidationError,
                        Severity::Error,
                        "header",
                        "Tier validation error: header node resolved outside input range (wrapper artifact)",
                    )
                    .with_suggestion(
                        "Check header formatting, the line may be malformed or in the wrong position",
                    ),
                FragmentCoverageError::Incomplete => fragment.whole_input_error(
                    ErrorCode::TierValidationError,
                    Severity::Error,
                    "header",
                    "Expected exactly one complete header fragment",
                ),
            };
            ParseErrors::from(vec![error])
        })?;
        Ok(Self { header, parsed })
    }

    /// Lower the admitted header. Its spans are synthetic; the wrapper that
    /// owns `parsed` rebases them.
    pub(crate) fn lower(self) -> ParseResult<Synthetic<Header>> {
        let Self { header, parsed } = self;
        let fragment = parsed.fragment();
        let source = header.source();
        let header_node = source.raw_node();
        let wrapped = source.source();
        // Dispatch to appropriate header parser
        // The source owner projects diagnostics into the caller's input.
        let inner_sink = ErrorCollector::new();
        let error_sink = fragment.error_sink(&inner_sink);
        // SelectedHeader admits only generated named header kinds, never a
        // generic ERROR. MISSING placeholders still require normal lowering
        // and recovery diagnostics; kind admission does not certify validity.
        let header = if let Some(choice) = source.typed::<FullDocumentChild1Choice>() {
            match parse_pre_begin_header(choice, &error_sink) {
                Ok(header) => header,
                Err(fault) => {
                    crate::parser::typed_cst::report_cst_failure(
                        header_node,
                        wrapped,
                        fault,
                        &error_sink,
                    );
                    return Err(ParseErrors::from(inner_sink.to_vec()));
                }
            }
        } else {
            match header_node.kind() {
                // Document anchors are outside both header supertype choices.
                UTF8_HEADER => Header::Utf8,
                BEGIN_HEADER => Header::Begin,
                END_HEADER => Header::End,
                // EVERYTHING ELSE goes to the one exhaustive dispatcher.
                //
                // This used to be nineteen more hand-written arms plus an
                // `unknown =>` catch-all, and it covered 19 of the `header`
                // supertype's 34 subtypes. The other 15 (`@Activities`,
                // `@Location`, `@Options`, `@Time Start`, `@Transcriber`,
                // ...) parsed correctly INSIDE a document, where
                // `parse_header_node` matches `HeaderChoice` exhaustively,
                // and were REJECTED here, out of the public
                // `parse_header` / `parse_header_fragment` entry points.
                // Two dispatchers for one job, one of them a drifted subset,
                // and the tests covered only the arms that existed.
                //
                // `dispatch_header_choice` has no `_` arm, so a future
                // `header` subtype fails to compile until it is handled
                // rather than silently reaching a catch-all here.
                unknown => match parse_header_node(source, &error_sink) {
                    ParseOutcome::Parsed(header) => header,
                    // Not a `header` subtype at all: the same diagnostic and
                    // the same fallback value the catch-all produced.
                    ParseOutcome::Rejected => {
                        error_sink.report(ParseError::new(
                        ErrorCode::MalformedTierContent,
                        Severity::Error,
                        DiagnosticLocation::new(span_of(header_node)),
                        ErrorContext::new(
                            wrapped,
                            span_of(header_node),
                            "",
                        ),
                        format!(
                            "Unknown header type '{unknown}' - will be flagged during validation"
                        ),
                    ));
                        // Rejection already has diagnostics; do not construct
                        // an unknown-header model solely to discard it below.
                        drop(error_sink);
                        return Err(ParseErrors::from(inner_sink.into_vec()));
                    }
                },
            }
        };

        drop(error_sink);
        let parse_errors = inner_sink.into_vec();
        if !parse_errors.is_empty() {
            let mut errors = ParseErrors::new();
            errors.errors.extend(parse_errors);
            return Err(errors);
        }

        Ok(parsed.lowered(header))
    }
}
