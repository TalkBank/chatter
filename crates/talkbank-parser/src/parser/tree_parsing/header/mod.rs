//! Header parsing using tree-sitter nodes
//!
//! This module provides functions to extract structured header data from tree-sitter nodes.
//! **CRITICAL**: NO string parsing! All data is extracted from tree-sitter child nodes.
//!
//! # Philosophy
//!
//! - Extract data from tree-sitter nodes by position/field name
//! - Never parse strings - tree-sitter has already done the parsing
//! - Return typed `Header` variants for valid structures
//! - Return `Header::Unknown` (with parse_reason) when required CST parts are missing
//! - Use error recovery - stream errors and preserve as much signal as possible
//!
//! # Module Structure
//!
//! - `id/` - @ID header parsing (~260 lines)
//! - `participants.rs` - @Participants header parsing (~170 lines)
//! - `metadata/` - @Languages, @PID, @Media, @Situation, @Types, @T
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

mod id;
mod metadata;
mod participants;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::node_types::{COMMA, PARTICIPANTS_HEADER, WHITESPACES};
use tree_sitter::Node;

// Re-export all header parsing functions
pub use id::parse_id_header;
pub use metadata::{
    parse_languages_header, parse_media_header, parse_pid_header, parse_situation_header,
    parse_t_header, parse_types_header,
};
pub use participants::parse_participants_header;

/// Whether an `ERROR` recovery node consists solely of comma separators (with
/// optional whitespace): the shape tree-sitter produces for a dangling trailing
/// comma (and doubled commas) at the end of a comma-separated header list. This
/// classifies by node KIND, never by matching the text against `","`, per this
/// module's AST-first rule, so `,,` and `, ` are recognized too.
fn error_is_only_commas(error_node: Node) -> bool {
    let mut cursor = error_node.walk();
    let mut saw_comma = false;
    for child in error_node.children(&mut cursor) {
        match child.kind() {
            COMMA => saw_comma = true,
            WHITESPACES => {}
            _ => return false,
        }
    }
    saw_comma
}

/// Report any structural `ERROR`/`MISSING` node tree-sitter parked as a direct
/// child of a header node, OUTSIDE the header's `*_contents` child.
///
/// Header parsers (`parse_participants_header`, `parse_languages_header`, ...)
/// descend into the single `*_contents` child and never inspect its siblings,
/// so a recovery node parked beside `*_contents` would be silently swallowed,
/// leaving an invalid file reported as valid. This is the shared shape of every
/// comma-separated header list: a stray trailing comma cannot be absorbed into
/// `*_contents`, so tree-sitter parks it as an `(ERROR (comma))` SIBLING
/// (verified for both `@Participants` and `@Languages`). Each header parser that
/// exclusively owns its subtree calls this, so the recovery node is always
/// reported and never swallowed.
///
/// A trailing comma in `@Participants` is reported as E550, the precise
/// tier-specific CLAN CHECK 100 diagnostic; a trailing comma in any other header
/// is rejected generically; any other stray `ERROR`/`MISSING` is reported so
/// nothing is dropped.
pub(crate) fn report_header_structural_errors(
    header: Node,
    header_kind: &str,
    source: &str,
    errors: &impl ErrorSink,
) {
    let mut cursor = header.walk();
    for child in header.children(&mut cursor) {
        if !(child.is_error() || child.is_missing()) {
            continue;
        }
        let location = SourceLocation::from_offsets(child.start_byte(), child.end_byte());
        let context = ErrorContext::new(source, child.start_byte()..child.end_byte(), header_kind);

        if child.is_missing() {
            errors.report(ParseError::new(
                ErrorCode::MissingRequiredElement,
                Severity::Error,
                location,
                context,
                format!(
                    "Missing required '{}' in header (tree-sitter error recovery)",
                    child.kind()
                ),
            ));
        } else if error_is_only_commas(child) {
            // A dangling list separator. `@Participants` gets the precise CLAN
            // CHECK 100 diagnostic; other comma-list headers are rejected
            // generically (no CLAN code, but still invalid CHAT).
            let (code, message) = if header_kind == PARTICIPANTS_HEADER {
                (
                    ErrorCode::TrailingCommaInParticipants,
                    "Commas at the end of the @Participants tier are not allowed",
                )
            } else {
                (
                    ErrorCode::UnparsableHeader,
                    "Trailing comma is not allowed at the end of this header",
                )
            };
            errors.report(
                ParseError::new(code, Severity::Error, location, context, message)
                    .with_suggestion("Remove the trailing comma after the last item"),
            );
        } else {
            errors.report(ParseError::new(
                ErrorCode::UnparsableHeader,
                Severity::Error,
                location,
                context,
                "Malformed content in header",
            ));
        }
    }
}
