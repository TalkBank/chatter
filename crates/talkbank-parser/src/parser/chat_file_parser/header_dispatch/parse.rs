//! Header parsing dispatch from tree-sitter nodes to strongly-typed `Header` values.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>

use super::super::header_parser::parse_header_node;
use super::finder::find_header_node_in_tree;
use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, ParseErrors, ParseResult,
    Severity, SourceLocation,
};
use crate::model::{Header, WarningText};
use crate::node_types::*;
use crate::parser::TreeSitterParser;
use crate::parser::tree_parsing::header::parse_pid_header;
use tree_sitter::Node;

use talkbank_model::ParseOutcome;

impl TreeSitterParser {
    /// Parse one header line in isolation using a minimal wrapper CHAT document.
    ///
    /// Because tree-sitter requires a complete CHAT document for context, this method
    /// wraps the input in two synthetic documents (pre-`@Begin` and post-`@Begin`
    /// positions) and attempts to parse from each. Structural headers (`@UTF8`,
    /// `@Begin`, `@End`, `@New Episode`, `@Blank`) are recognized on a fast path
    /// without wrapping.
    ///
    /// # Parameters
    ///
    /// - `input`: A single CHAT header line, e.g., `@Languages:\teng`,
    ///   `@Participants:\tCHI Target_Child`, or `@Date:\t01-JAN-2020`.
    ///
    /// # Returns
    ///
    /// A strongly-typed `Header` enum variant corresponding to the parsed header.
    ///
    /// # Errors
    ///
    /// Returns `ParseErrors` when:
    /// - Tree-sitter fails to produce a parse tree for either wrapper.
    /// - The header node falls outside the input byte range (detected as a wrapper
    ///   artifact rather than the user's header).
    /// - The header CST node is malformed or has an unrecognized kind.
    pub fn parse_header(&self, input: &str) -> ParseResult<Header> {
        // Fast path for structural headers that can't be wrapped without
        // colliding with the wrapper's own structural headers
        let trimmed = input.trim();
        match trimmed {
            "@UTF8" => return Ok(Header::Utf8),
            "@Begin" => return Ok(Header::Begin),
            "@End" => return Ok(Header::End),
            "@New Episode" => return Ok(Header::NewEpisode),
            "@Blank" => return Ok(Header::Blank),
            _ => {}
        }

        const PRE_BEGIN_PREFIX: &str = "@UTF8\n";
        const PRE_BEGIN_SUFFIX: &str = "@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@End\n";
        const POST_BEGIN_PREFIX: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n";
        const POST_BEGIN_SUFFIX: &str = "@End\n";

        let pre_begin_wrapped = format!("{}{}\n{}", PRE_BEGIN_PREFIX, input, PRE_BEGIN_SUFFIX);
        let post_begin_wrapped = format!("{}{}\n{}", POST_BEGIN_PREFIX, input, POST_BEGIN_SUFFIX);

        let try_parse = |wrapped: &str,
                         header_index: usize,
                         input_offset: usize|
         -> ParseResult<Header> {
            let tree = self
                .parser
                .borrow_mut()
                .parse(wrapped, None)
                .ok_or_else(|| {
                    let mut errors = ParseErrors::new();
                    errors.push(
                        ParseError::new(
                            ErrorCode::TierValidationError,
                            Severity::Error,
                            SourceLocation::from_offsets(0, input.len()),
                            ErrorContext::new(input, 0..input.len(), "header"),
                            "Tier validation error: tree-sitter could not parse this header line",
                        )
                        .with_suggestion("Check that the header line follows CHAT format (e.g., @Header:<TAB>value)"),
                    );
                    errors
                })?;

            let ts_root = tree.root_node();
            // Navigate source_file → full_document for multi-root grammar
            let root = if ts_root.kind() == "source_file" {
                ts_root
                    .child(0)
                    .filter(|c| c.kind() == "full_document")
                    .unwrap_or(ts_root)
            } else {
                ts_root
            };
            let header_node = find_header_node_in_tree(root, header_index)?;

            // Verify the found header node is within the input's byte range,
            // not from the wrapper prefix/suffix. Without this check, when the
            // input header parses as ERROR (e.g. @Participants before @Begin),
            // the finder returns a wrapper header at the same index.
            let input_end = input_offset + input.len();
            if header_node.start_byte() < input_offset || header_node.start_byte() >= input_end {
                let mut errors = ParseErrors::new();
                errors.push(
                    ParseError::new(
                        ErrorCode::TierValidationError,
                        Severity::Error,
                        SourceLocation::from_offsets(0, input.len()),
                        ErrorContext::new(input, 0..input.len(), "header"),
                        "Tier validation error: header node resolved outside input range (wrapper artifact)",
                    )
                    .with_suggestion("Check header formatting, the line may be malformed or in the wrong position"),
                );
                return Err(errors);
            }

            // Dispatch to appropriate header parser
            // Use OffsetAdjustingErrorSink to ensure errors are relative to input, not wrapper
            use crate::error::OffsetAdjustingErrorSink;
            let inner_sink = ErrorCollector::new();
            let error_sink = OffsetAdjustingErrorSink::new(&inner_sink, input_offset, input);
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

            let parse_errors = inner_sink.into_vec();
            if !parse_errors.is_empty() {
                let mut errors = ParseErrors::new();
                errors.errors.extend(parse_errors);
                return Err(errors);
            }

            Ok(header)
        };

        let pre_begin_attempt = try_parse(&pre_begin_wrapped, 1, PRE_BEGIN_PREFIX.len());
        if pre_begin_attempt.is_ok() {
            return pre_begin_attempt;
        }

        let post_begin_attempt = try_parse(&post_begin_wrapped, 5, POST_BEGIN_PREFIX.len());
        match (pre_begin_attempt, post_begin_attempt) {
            (Ok(header), _) => Ok(header),
            (Err(_), Ok(header)) => Ok(header),
            (Err(pre_err), Err(post_err)) => {
                if post_err.len() <= pre_err.len() {
                    Err(post_err)
                } else {
                    Err(pre_err)
                }
            }
        }
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
