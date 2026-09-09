//! `@PID` header parsing, over the generated typed traversal.
//!
//! Grammar: `pid_header: seq(pid_prefix, header_sep, free_text, newline)`.
//! `PidHeaderNode` proves the node's kind and `extract_pid_header` places its
//! children in typed slots, so there is no kind to re-check and no child to
//! hunt for by name. Until 2026-09-08 this file hand-walked the node
//! (`node.kind() != PID_HEADER`, `find_child_by_kind(node, FREE_TEXT)`, a
//! UTF-8 decode failure on a `&str` source), three branches no input reaches
//! and rule 6 of the repository's CLAUDE.md bans; a whole-workspace coverage
//! run showed them as most of the file.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#PID_Header>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{AsRawNode, PidHeaderNode, extract_pid_header};
use crate::parser::tree_parsing::parser_helpers::{present, surface_displaced};
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, PidValue};

/// Parse a `@PID` header into [`Header::Pid`].
///
/// The value is the `free_text` slot's text as written; `PidValue` owns
/// whatever validation the persistent identifier gets.
pub fn parse_pid_header(typed: PidHeaderNode<'_>, source: &str, errors: &impl ErrorSink) -> Header {
    let node = typed.raw_node();
    let children = extract_pid_header(typed);
    // The value: the `free_text` position's text, decoded, and NOT empty. A
    // line with nothing after the tab leaves the position present with
    // zero-width text (the placeholder tree-sitter inserts is elsewhere on
    // the line), and until 2026-09-09 that empty text became
    // `Header::Pid { pid: "" }`, a value nobody wrote.
    enum Held {
        Value(String),
        Empty,
        Nothing,
    }
    let held = match present(children.child_2.slot()) {
        Some(free_text) => {
            match decode_present_child(free_text.raw_node(), source, errors, "pid_value", |err| {
                format!("Failed to extract PID value as UTF-8: {}", err)
            }) {
                ParseOutcome::Parsed(pid) if pid.is_empty() => Held::Empty,
                ParseOutcome::Parsed(pid) => Held::Value(pid),
                ParseOutcome::Rejected => Held::Nothing,
            }
        }
        None => Held::Nothing,
    };
    surface_displaced(&children.unexpected, "pid_header", source, errors);
    // An empty value lowers as unknown with no report of its own: the
    // placeholder is the whole-tree pass's and the malformed header is
    // validation's, and an empty value is not a traversal failure.
    if let Held::Empty = held {
        return super::super::unknown_header(
            node,
            source,
            "@PID",
            "Expected @PID:\t<value>",
            "Missing PID value in @PID header",
        );
    }
    let Held::Value(pid) = held else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), "pid_header"),
            "Missing PID value in @PID header",
        ));
        return super::super::unknown_header(
            node,
            source,
            "@PID",
            "Expected @PID:\t<value>",
            "Missing PID value in @PID header",
        );
    };
    Header::Pid {
        pid: PidValue::new(pid),
    }
}
