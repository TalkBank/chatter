//! Header parsing dispatch from tree-sitter nodes to strongly-typed `Header` values.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>

use super::finder::find_header_node_in_tree;
use super::fragment::HeaderFragment;
use crate::api::fragment::WrappedFragment;
use crate::error::{
    ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity, SourceLocation,
};
use crate::model::Header;
use crate::parser::TreeSitterParser;

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
        const PRE_BEGIN_SUFFIX: &str = "\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@End\n";
        const POST_BEGIN_PREFIX: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n";
        const POST_BEGIN_SUFFIX: &str = "\n@End\n";

        let pre_begin_wrapped =
            WrappedFragment::new(&[PRE_BEGIN_PREFIX], input, PRE_BEGIN_SUFFIX, 0);
        let post_begin_wrapped =
            WrappedFragment::new(&[POST_BEGIN_PREFIX], input, POST_BEGIN_SUFFIX, 0);

        let try_parse = |fragment: &WrappedFragment<'_>,
                         header_index: usize|
         -> ParseResult<Header> {
            let wrapped = fragment.source();
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
            let header_node = find_header_node_in_tree(root, header_index).map_err(|failure| {
                ParseErrors::from(vec![
                    ParseError::new(
                        ErrorCode::TierValidationError,
                        Severity::Error,
                        SourceLocation::from_offsets(0, input.len()),
                        ErrorContext::new(input, 0..input.len(), "header"),
                        failure.to_string(),
                    )
                    .with_suggestion(
                        "Check that all header lines are well-formed and appear before utterances",
                    ),
                ])
            })?;

            HeaderFragment::admit(header_node, fragment)?.lower()
        };

        let pre_begin_attempt = try_parse(&pre_begin_wrapped, 1);
        if pre_begin_attempt.is_ok() {
            return pre_begin_attempt;
        }

        let post_begin_attempt = try_parse(&post_begin_wrapped, 5);
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
