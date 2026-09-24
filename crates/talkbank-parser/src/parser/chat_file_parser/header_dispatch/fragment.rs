//! Admission and lowering of one complete wrapped header fragment.

use super::super::header_parser::{parse_header_node, parse_pre_begin_header};
use super::finder::{HeaderLookupError, SelectedHeader, find_header_node_in_tree, first_child};
use crate::api::fragment::{FragmentCoverageError, ParsedFragment, WrappedFragment};
use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, ParseErrors, ParseResult,
    Severity, SourceLocation,
};
use crate::generated_traversal::{AsRawNode, FromNodeKind, FullDocumentChild1Choice};
use crate::model::Header;
use crate::node_types::*;
use crate::parser::tree_parsing::parser_helpers::unknown_header_from_node;
use talkbank_model::ParseOutcome;

/// Lowering owns the node and the source whose complete input it represents.
/// No caller can lower a header merely because it starts inside the input.
pub(super) struct HeaderFragment<'tree, 'source, 'input> {
    header: SelectedHeader<'tree, 'source>,
    fragment: &'source WrappedFragment<'input>,
}

impl<'tree, 'source, 'input> HeaderFragment<'tree, 'source, 'input> {
    pub(super) fn admit(
        parsed: &'tree ParsedFragment<'source, 'input>,
        header_index: usize,
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
            let input = fragment.input();
            let code = match &error {
                HeaderLookupError::NotFound { .. } => ErrorCode::TierValidationError,
                HeaderLookupError::Binding(_) => ErrorCode::TreeParsingError,
            };
            ParseErrors::from(vec![
                ParseError::new(
                    code,
                    Severity::Error,
                    SourceLocation::from_offsets(0, input.len()),
                    ErrorContext::new(input, 0..input.len(), "header"),
                    error.to_string(),
                )
                .with_suggestion(
                    "Check that all header lines are well-formed and appear before utterances",
                ),
            ])
        })?;
        let node = header.source().raw_node();
        fragment.require_complete_input(node.byte_range()).map_err(|failure| {
            let input = fragment.input();
            let error = match failure {
                FragmentCoverageError::OutsideInput => ParseError::new(
                    ErrorCode::TierValidationError,
                    Severity::Error,
                    SourceLocation::from_offsets(0, input.len()),
                    ErrorContext::new(input, 0..input.len(), "header"),
                    "Tier validation error: header node resolved outside input range (wrapper artifact)",
                ).with_suggestion(
                    "Check header formatting, the line may be malformed or in the wrong position",
                ),
                FragmentCoverageError::Incomplete => ParseError::new(
                    ErrorCode::TierValidationError,
                    Severity::Error,
                    SourceLocation::from_offsets(0, input.len()),
                    ErrorContext::new(input, 0..input.len(), "header"),
                    "Expected exactly one complete header fragment",
                ),
            };
            ParseErrors::from(vec![error])
        })?;
        Ok(Self { header, fragment })
    }

    pub(super) fn lower(self) -> ParseResult<Header> {
        let Self { header, fragment } = self;
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
        let header = if let Some(choice) = FullDocumentChild1Choice::from_node(header_node) {
            parse_pre_begin_header(&choice, wrapped, &error_sink)
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
                        SourceLocation::from_offsets(
                            header_node.start_byte(),
                            header_node.end_byte(),
                        ),
                        ErrorContext::new(
                            wrapped,
                            header_node.start_byte()..header_node.end_byte(),
                            "",
                        ),
                        format!(
                            "Unknown header type '{unknown}' - will be flagged during validation"
                        ),
                    ));
                        unknown_header_from_node(
                            header_node,
                            wrapped,
                            format!("Unrecognized header type: {unknown}"),
                            None,
                        )
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

        Ok(header)
    }
}
