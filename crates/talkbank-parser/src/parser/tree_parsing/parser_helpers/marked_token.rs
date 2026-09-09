//! Grammar tokens that begin with a marker their rule guarantees.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

/// The text of a grammar token past the marker its rule begins with.
///
/// The grammar guarantees the marker (`$` on a `pos_tag`, `@` on a
/// `form_marker`, `@s` on a `word_lang_suffix`, `%x` on an `x_tier_prefix`,
/// `%` on an `unsupported_tier_prefix`), so a token without it, or one
/// that is not UTF-8, does not fit its own grammar: that is the traversal's
/// failure, reported as such (E330, naming the token as `what` calls it)
/// and read as nothing, never read around. Until 2026-09-09 each site stripped
/// its marker behind a fallback (`strip_prefix(..).unwrap_or(text)`, or
/// `unwrap_or("")`), so a bare payload read as a declared one and a token
/// that was not an `@s` suffix at all became the bare `@s` shortcut.
pub(crate) fn after_marker<'a>(
    node: Node,
    marker: &str,
    what: &str,
    source: &'a str,
    errors: &impl ErrorSink,
) -> Option<&'a str> {
    let fault = |message: String| {
        ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            message,
        )
    };
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(text) => text,
        Err(error) => {
            errors.report(
                fault(format!("{what} is not valid UTF-8: {error}")).with_suggestion(
                    "The source file may contain invalid UTF-8 sequences. Ensure the file is \
                     properly encoded as UTF-8.",
                ),
            );
            return None;
        }
    };
    let Some(rest) = text.strip_prefix(marker) else {
        errors.report(fault(format!("{what} does not begin with '{marker}'")));
        return None;
    };
    Some(rest)
}
