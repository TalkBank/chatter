//! Admission and lowering of one complete wrapped header fragment.

use super::super::header_parser::parse_header_node;
use crate::api::fragment::{FragmentCoverageError, WrappedFragment};
use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, ParseErrors, ParseResult,
    Severity, SourceLocation,
};
use crate::model::{Header, WarningText};
use crate::node_types::*;
use crate::parser::tree_parsing::header::parse_pid_header;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Lowering owns the node and the source whose complete input it represents.
/// No caller can lower a header merely because it starts inside the input.
pub(super) struct HeaderFragment<'tree, 'source, 'input> {
    node: Node<'tree>,
    fragment: &'source WrappedFragment<'input>,
}

impl<'tree, 'source, 'input> HeaderFragment<'tree, 'source, 'input> {
    pub(super) fn admit(
        node: Node<'tree>,
        fragment: &'source WrappedFragment<'input>,
    ) -> ParseResult<Self> {
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
        Ok(Self { node, fragment })
    }

    pub(super) fn lower(self) -> ParseResult<Header> {
        let Self {
            node: header_node,
            fragment,
        } = self;
        let wrapped = fragment.source();
        // Dispatch to appropriate header parser
        // The source owner projects diagnostics into the caller's input.
        let inner_sink = ErrorCollector::new();
        let error_sink = fragment.error_sink(&inner_sink);
        let header = if header_node.is_error() {
            error_sink.report(ParseError::new(
                ErrorCode::MalformedWordContent,
                Severity::Error,
                SourceLocation::from_offsets(header_node.start_byte(), header_node.end_byte()),
                ErrorContext::new(
                    wrapped,
                    header_node.start_byte()..header_node.end_byte(),
                    "",
                ),
                format!(
                    "Malformed header at byte {}..{}",
                    header_node.start_byte(),
                    header_node.end_byte()
                ),
            ));
            let text = match header_node.utf8_text(wrapped.as_bytes()) {
                Ok(text) => text.to_string(),
                Err(_) => header_node.kind().to_string(),
            };
            Header::Unknown {
                text: WarningText::new(text),
                parse_reason: Some("Malformed header content".to_string()),
                suggested_fix: None,
            }
        } else {
            match header_node.kind() {
                // The four kinds the `header` supertype does NOT name, so
                // the generated classifier cannot route them: `@UTF8`,
                // `@Begin` and `@End` are the document's own anchors, and
                // `pid_header` is a `pre_begin_header` subtype.
                UTF8_HEADER => Header::Utf8,
                BEGIN_HEADER => Header::Begin,
                END_HEADER => Header::End,
                PID_HEADER => parse_pid_header(header_node, wrapped, &error_sink),
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
                unknown => match parse_header_node(header_node, wrapped, &error_sink) {
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
                        unknown_header_with_reason(
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

/// Build a `Header::Unknown` while preserving source text and parse reason.
fn unknown_header_with_reason(
    node: Node,
    input: &str,
    reason: impl Into<String>,
    suggested_fix: Option<&str>,
) -> Header {
    let text = match node.utf8_text(input.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => node.kind().to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(reason.into()),
        suggested_fix: suggested_fix.map(str::to_string),
    }
}
