//! Header parsing dispatch from tree-sitter nodes to strongly-typed `Header` values.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>

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
            WrappedFragment::new(&[PRE_BEGIN_PREFIX], input, PRE_BEGIN_SUFFIX, 0)?;
        let post_begin_wrapped =
            WrappedFragment::new(&[POST_BEGIN_PREFIX], input, POST_BEGIN_SUFFIX, 0)?;

        let try_parse =
            |fragment: &WrappedFragment<'_>, header_index: usize| -> ParseResult<Header> {
                let parsed = fragment.parse(self).map_err(|_| {
                    let mut errors = ParseErrors::new();
                    errors.push(
                    ParseError::new(
                        ErrorCode::TierValidationError,
                        Severity::Error,
                        SourceLocation::from_offsets(0, input.len()),
                        ErrorContext::new(input, 0..input.len(), "header"),
                        "Tier validation error: tree-sitter could not parse this header line",
                    )
                    .with_suggestion(
                        "Check that the header line follows CHAT format (e.g., @Header:<TAB>value)",
                    ),
                );
                    errors
                })?;

                HeaderFragment::admit(&parsed, header_index)?.lower()
            };

        let pre_err = match try_parse(&pre_begin_wrapped, 1) {
            Ok(header) => return Ok(header),
            Err(error) => error,
        };
        match try_parse(&post_begin_wrapped, 5) {
            Ok(header) => Ok(header),
            Err(post_err) => {
                if post_err.len() <= pre_err.len() {
                    Err(post_err)
                } else {
                    Err(pre_err)
                }
            }
        }
    }
}
