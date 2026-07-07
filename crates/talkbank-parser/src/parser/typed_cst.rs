//! Shared typed-CST seam for the production parser.
//!
//! One region-neutral primitive lives here so that every parser region (the
//! header parsers in `chat_file_parser/header_parser/dispatch/` and
//! `tree_parsing/header/`, and the utterance / dependent-tier / content parsers
//! in `chat_file_parser/` and `tree_parsing/main_tier/`) reaches ONE shared seam
//! instead of each module subtree reimplementing it:
//!
//! - [`decode_present_child`], the single helper that decodes a `Present`
//!   content child's bytes to text and reports the caller's family-specific
//!   diagnostic on a UTF-8 error. It replaces the duplicated decode logic that
//!   each header family carried (`read_simple_content`, `decode_child_text`,
//!   `read_types_field`, `decode_field_text`, and the inline `@Situation`
//!   decode).
//!
//! (The former `TypedTraversal` ZST receiver for the OLD `GrammarTraversal`
//! trait visitor was retired in the 2026-07 visitor migration once every region
//! flipped onto the NEW self-contained backend's free `extract_*` functions; the
//! parser no longer routes any structure through a trait receiver.)

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use std::str::Utf8Error;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Decode a `Present` content child's bytes to owned text, reporting the caller's
/// family-specific diagnostic on a UTF-8 error.
///
/// This is the ONE shared form of the "decode a typed content child, emit the
/// family diagnostic on a `utf8_text` `Err`" logic that the header families
/// previously reimplemented (`read_simple_content`, `decode_child_text`,
/// `read_types_field`, `decode_field_text`, and the inline `@Situation` decode).
/// The decode itself and the `Ok` / `Err` discipline are identical across
/// families; only the reported diagnostic differs, so the family supplies it:
///
/// - `context` is the [`ErrorContext`] node-kind / label string for the family
///   (e.g. the header kind, the field label, or `"id_contents"` /
///   `"situation_text"`).
/// - `make_message` builds the family's exact message from the [`Utf8Error`]. It
///   is invoked ONLY on the error path, so every call site reproduces its
///   pre-extraction message byte-for-byte (the families deliberately phrase it
///   differently: "text for `<kind>`", "text from `<label>`", a bare "text", and
///   the `@Situation`-specific wording).
///
/// Reads from the RAW node's `utf8_text`, NOT a typed wrapper's `.text()`
/// accessor (which swallows UTF-8 errors via `unwrap_or("")`), so the
/// `Ok` / `Err` split matches the pre-migration behaviour exactly. On success the
/// decoded text is returned as [`ParseOutcome::Parsed`]; on a UTF-8 error the
/// diagnostic is reported at the child's span and [`ParseOutcome::Rejected`] is
/// returned. The CHAT source handed in is already valid UTF-8 in practice, so the
/// error arm is defensive only.
pub(in crate::parser) fn decode_present_child(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
    make_message: impl FnOnce(Utf8Error) -> String,
) -> ParseOutcome<String> {
    match node.utf8_text(source.as_bytes()) {
        Ok(text) => ParseOutcome::parsed(text.to_string()),
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), context),
                make_message(err),
            ));
            ParseOutcome::rejected()
        }
    }
}
