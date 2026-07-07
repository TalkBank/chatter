//! Per-kind parsing for structured headers with dedicated sub-parsers.
//!
//! Each function here is the LEVEL-1 entry for one `HeaderChoice` structured
//! variant. They all share the same pre-migration wrapper (a local
//! `ErrorCollector`, call the dedicated `tree_parsing/header/` sub-parser,
//! forward its diagnostics to `errors`, then `ParseOutcome::parsed(header)`),
//! factored into `call_sub`; each entry is a one-line call with its sub-parser.
//! The sub-parser bodies in `tree_parsing/header/` are unchanged.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use crate::error::{ErrorCollector, ErrorSink};
use crate::model::Header;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use crate::parser::tree_parsing::header::{
    parse_id_header, parse_languages_header, parse_media_header, parse_participants_header,
    parse_situation_header, parse_types_header,
};

/// Shared LEVEL-1 wrapper for the structured-header families: run the dedicated
/// `tree_parsing/header/` sub-parser under a local `ErrorCollector`, forward its
/// collected diagnostics to `errors`, and return the parsed `Header`. This is the
/// exact pre-migration wrapper behaviour, shared by all six entries below.
fn call_sub<F>(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
    sub: F,
) -> ParseOutcome<Header>
where
    F: FnOnce(Node, &str, &ErrorCollector) -> Header,
{
    let header_errors = ErrorCollector::new();
    let header = sub(header_actual, input, &header_errors);
    errors.report_all(header_errors.into_vec());
    ParseOutcome::parsed(header)
}

/// `@Languages` -> `parse_languages_header`.
pub(super) fn languages(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header_actual, input, errors, parse_languages_header)
}

/// `@Participants` -> `parse_participants_header`.
pub(super) fn participants(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header_actual, input, errors, parse_participants_header)
}

/// `@ID` -> `parse_id_header`.
pub(super) fn id(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header_actual, input, errors, parse_id_header)
}

/// `@Media` -> `parse_media_header`.
pub(super) fn media(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header_actual, input, errors, parse_media_header)
}

/// `@Situation` -> `parse_situation_header`.
pub(super) fn situation(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header_actual, input, errors, parse_situation_header)
}

/// `@Types` -> `parse_types_header`.
pub(super) fn types(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header_actual, input, errors, parse_types_header)
}
