//! Header parsing dispatch from tree-sitter nodes to strongly-typed `Header` values.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>

use super::fragment::{HeaderCoverage, HeaderFragment};
use crate::api::fragment::{Synthetic, WrappedFragment};
use crate::error::{ErrorCode, ParseErrors, ParseResult, Severity};
use crate::model::Header;
use crate::parser::TreeSitterParser;

/// The synthetic documents a lone header line is parsed inside: before
/// `@Begin` (index 1) and after the preamble (index 5).
pub(crate) const PRE_BEGIN_PREFIX: &str = "@UTF8\n";
pub(crate) const PRE_BEGIN_SUFFIX: &str = "\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@End\n";
pub(crate) const POST_BEGIN_PREFIX: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n";
pub(crate) const POST_BEGIN_SUFFIX: &str = "\n@End\n";
/// The header index of a line placed after `POST_BEGIN_PREFIX`.
pub(crate) const POST_BEGIN_HEADER_INDEX: usize = 5;

impl TreeSitterParser {
    /// Parse a header inside a wrapper this caller built, keeping the result
    /// in the wrapper's coordinates for the caller to rebase.
    ///
    /// # Errors
    ///
    /// A failed parse, a header not found at `header_index`, a node not
    /// covering the caller's input, or a lowering fault, each already
    /// projected into the caller's coordinates by the wrapper's sink.
    pub(crate) fn lower_wrapped_header(
        &self,
        fragment: &WrappedFragment<'_>,
        header_index: usize,
        coverage: HeaderCoverage,
    ) -> ParseResult<Synthetic<Header>> {
        let parsed = fragment.parse(self).map_err(|_| {
            ParseErrors::from(vec![
                fragment
                    .whole_input_error(
                        ErrorCode::TierValidationError,
                        Severity::Error,
                        "header",
                        "Tier validation error: tree-sitter could not parse this header line",
                    )
                    .with_suggestion(
                        "Check that the header line follows CHAT format (e.g., @Header:<TAB>value)",
                    ),
            ])
        })?;
        HeaderFragment::admit(&parsed, header_index, coverage)?.lower()
    }

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

        let pre_begin_wrapped =
            WrappedFragment::local(&[PRE_BEGIN_PREFIX], input, PRE_BEGIN_SUFFIX)?;
        let post_begin_wrapped =
            WrappedFragment::local(&[POST_BEGIN_PREFIX], input, POST_BEGIN_SUFFIX)?;

        // The lowered header is in the wrapper's coordinates; the wrapper
        // moves it into the caller's (input-relative, origin 0).
        let try_parse =
            |fragment: &WrappedFragment<'_>, header_index: usize| -> ParseResult<Header> {
                self.lower_wrapped_header(fragment, header_index, HeaderCoverage::WholeLine)
                    .and_then(|header| fragment.rebase(header))
            };

        let pre_err = match try_parse(&pre_begin_wrapped, 1) {
            Ok(header) => return Ok(header),
            Err(error) => error,
        };
        match try_parse(&post_begin_wrapped, POST_BEGIN_HEADER_INDEX) {
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
