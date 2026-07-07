//! Shared low-level routines used by CHAT file parsing.
//!
//! This layer handles line iteration, selective top-level error recovery, and
//! conversion into `Line` values before participant synthesis and normalization.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
    TeeErrorSink,
};
use crate::model::{ChatDate, Header, Line, WarningText};
use crate::parser::TreeSitterParser;
use crate::parser::chat_file_parser::utterance_parser::classify_percent_error_text;
use crate::parser::tree_parsing::parser_helpers::collect_recovery_nodes;
use tracing::{debug, info, trace, warn};

use super::document_lowering::DocumentLowering;

/// Recover specific top-level `ERROR` nodes that still encode a valid header shape.
pub(super) fn recover_top_level_error_node(
    error_node: tree_sitter::Node,
    input: &str,
    lines: &mut Vec<Line>,
) -> bool {
    let Ok(text) = error_node.utf8_text(input.as_bytes()) else {
        return false;
    };

    let bytes = text.as_bytes();
    if bytes.starts_with(b"@Date:")
        && bytes[6..]
            .iter()
            .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
    {
        let span = Span::new(error_node.start_byte() as u32, error_node.end_byte() as u32);
        let date_value = text[6..].trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r'));
        lines.push(Line::header_with_span(
            Header::Date {
                date: ChatDate::new(date_value),
            },
            span,
        ));
        return true;
    }

    if recover_unknown_header_line(error_node, text, lines) {
        return true;
    }

    false
}

/// Convert an unknown `@Header:` line embedded in an `ERROR` node into `Header::Unknown`.
fn recover_unknown_header_line(
    error_node: tree_sitter::Node,
    text: &str,
    lines: &mut Vec<Line>,
) -> bool {
    let first_line = match text.lines().next() {
        Some(line) => line.trim_end(),
        None => return false,
    };
    if !first_line.starts_with('@') {
        return false;
    }

    let colon_index = match first_line.find(':') {
        Some(idx) => idx,
        None => return false,
    };
    if colon_index <= 1 {
        return false;
    }

    let label = &first_line[1..colon_index];
    if is_known_header_label(label) {
        return false;
    }

    let span = Span::new(error_node.start_byte() as u32, error_node.end_byte() as u32);
    lines.push(Line::header_with_span(
        Header::Unknown {
            text: WarningText::new(first_line.to_string()),
            parse_reason: Some(format!(
                "Recovered unknown header '{}' from parse error node",
                label
            )),
            suggested_fix: Some(
                "Use a standard CHAT header, or keep this as legacy metadata".to_string(),
            ),
        },
        span,
    ));
    true
}

/// Return whether `label` is a known CHAT header key.
fn is_known_header_label(label: &str) -> bool {
    matches!(
        label.to_ascii_lowercase().as_str(),
        "utf8"
            | "begin"
            | "end"
            | "new episode"
            | "languages"
            | "comment"
            | "participants"
            | "id"
            | "pid"
            | "date"
            | "media"
            | "number"
            | "recording quality"
            | "transcription"
            | "situation"
            | "types"
            | "tape location"
            | "time duration"
            | "time start"
            | "birth of"
            | "birthplace of"
            | "l1 of"
            | "font"
            | "window"
            | "color words"
            | "bck"
            | "bg"
            | "eg"
            | "g"
            | "t"
            | "location"
            | "room layout"
            | "transcriber"
            | "videos"
            | "options"
            | "warning"
            | "activities"
            | "blank"
            | "page"
    )
}

/// Report malformed/orphaned top-level dependent tiers and taint the prior utterance if present.
pub(super) fn report_top_level_dependent_tier_error(
    error_node: tree_sitter::Node,
    input: &str,
    lines: &mut [Line],
    errors: &impl ErrorSink,
) -> bool {
    let Ok(text) = error_node.utf8_text(input.as_bytes()) else {
        return false;
    };

    if !text.starts_with('%') {
        return false;
    }

    let first_line = text.lines().next().unwrap_or(text);
    let mut has_preceding_utterance = false;
    if let Some(utterance) = lines.iter_mut().rev().find_map(|line| match line {
        Line::Utterance(utt) => Some(utt),
        _ => None,
    }) {
        has_preceding_utterance = true;
        match classify_percent_error_text(first_line) {
            Some(tier) => utterance.mark_parse_taint(tier),
            None => utterance.mark_all_dependent_alignment_taint(),
        }
    }

    let (code, message, suggestion) = if !has_preceding_utterance {
        (
            ErrorCode::OrphanedDependentTier,
            format!(
                "Dependent tier appears before any main tier: {}",
                first_line.trim_end()
            ),
            "Move this dependent tier directly below its parent main tier",
        )
    } else if !first_line.contains(":\t") {
        (
            ErrorCode::MalformedTierHeader,
            format!("Malformed dependent tier header: {}", first_line.trim_end()),
            "Use dependent tier syntax %tier:\\tcontent",
        )
    } else if first_line.contains("|||") {
        (
            ErrorCode::InvalidDependentTier,
            format!("Invalid dependent tier content: {}", first_line.trim_end()),
            "Provide valid tier content for the declared dependent tier type",
        )
    } else if first_line.starts_with("%mor:")
        || first_line.starts_with("%gra:")
        || first_line.starts_with("%pho:")
        || first_line.starts_with("%sin:")
    {
        (
            ErrorCode::TierValidationError,
            format!(
                "Tier validation error: could not fully parse dependent tier '{}'",
                first_line.trim_end()
            ),
            "Fix tier-internal format, check tokenization, pipe delimiters, and required fields",
        )
    } else {
        (
            ErrorCode::InvalidDependentTier,
            format!(
                "Could not fully parse dependent tier: {}",
                first_line.trim_end()
            ),
            "Check dependent tier syntax (%tier:\\tcontent) and tier-specific format",
        )
    };

    errors.report(
        ParseError::new(
            code,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(input, error_node.start_byte()..error_node.end_byte(), text),
            message,
        )
        .with_suggestion(suggestion),
    );

    true
}

/// Parse all lines from `input` and stream diagnostics to `errors`.
pub(super) fn parse_lines(
    parser: &TreeSitterParser,
    input: &str,
    errors: &impl ErrorSink,
) -> Vec<Line> {
    parse_lines_with_old_tree(parser, input, None, errors).0
}

/// Parse lines, optionally reusing `old_tree` for incremental updates.
/// Returns `(lines, new_tree)`.
pub(super) fn parse_lines_with_old_tree(
    parser: &TreeSitterParser,
    input: &str,
    old_tree: Option<&tree_sitter::Tree>,
    errors: &impl ErrorSink,
) -> (Vec<Line>, Option<tree_sitter::Tree>) {
    debug!("Parsing CHAT file ({} bytes)", input.len());

    let tree = match parser.parser.borrow_mut().parse(input, old_tree) {
        Some(t) => t,
        None => {
            warn!("Tree-sitter parse failed for CHAT file");
            errors.report(ParseError::new(
                ErrorCode::ParseFailed,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "Tree-sitter parse failed for chat file",
            ));
            return (Vec::new(), None);
        }
    };
    let tree_to_return = tree.clone();

    trace!("Tree-sitter parse completed");
    let ts_root = tree.root_node();

    // With multi-root grammar, root is `source_file` containing `full_document`.
    // Navigate to `full_document` if present, otherwise use root directly.
    let root_node = if ts_root.kind() == "source_file" {
        ts_root
            .child(0)
            .filter(|c| c.kind() == "full_document")
            .unwrap_or(ts_root)
    } else {
        ts_root
    };

    // Check if the root node itself has errors AND is empty (e.g., empty file)
    if root_node.has_error() && root_node.child_count() == 0 {
        errors.report(
            ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len().max(1)),
                ErrorContext::new(input, 0..input.len().max(1), input),
                "Unparsable content: file is empty or contains no recognizable CHAT structure",
            )
            .with_suggestion("CHAT files must contain at minimum @UTF8, @Begin, and @End headers"),
        );
        return (Vec::new(), None);
    }

    // Track whether root is ERROR; we'll need this after the loop to report
    // an error if no valid lines were recovered.
    let root_is_error = root_node.is_error();

    // Tee the sink into a collector so the streaming loop's per-region
    // diagnostics are recorded as well as forwarded. The loop only inspects
    // recovery (ERROR/MISSING) nodes in the regions its handlers descend into;
    // recovery nodes nested elsewhere were silently dropped, so a file that
    // tree-sitter flagged as malformed could still validate clean. After the
    // loop, a whole-tree backstop surfaces any recovery node the handlers missed
    // (recovery is not validity), using the collected spans to avoid
    // double-reporting a node a richer region diagnostic already covered.
    // `ErrorCollector` allocates lazily, so a clean file pays nothing.
    let collector = ErrorCollector::new();
    let recording = TeeErrorSink::new(errors, &collector);
    let errors = &recording;

    // Visitor-driven document/line walk (Task 1 of the visitor-driven parser
    // migration). The hand-walked `match child.kind()` dispatch over
    // `full_document` children was replaced by `DocumentLowering`, which drives
    // the generated `extract_full_document` and processes each `NodeSlot` slot
    // exhaustively. The model and recovery diagnostics are preserved exactly: a
    // document-level ERROR routes through the same dependent-tier / recovery /
    // analyze path, and each present `line` is dispatched to the unchanged inner
    // hand-walk. `DocumentLowering` borrows the Tee'd sink so its emissions are
    // recorded for the backstop's span-dedup below.
    let mut lowering = DocumentLowering::new(input, errors, root_node.child_count());
    lowering.lower_document(root_node);
    let lines = lowering.into_lines();

    // When the root IS an ERROR node and the loop couldn't recover any valid
    // lines, the file is completely unparsable.  Report this so the strict caller
    // returns Err.  When the root is ERROR but children ARE valid structures
    // (e.g., missing @End), the loop recovers lines and the validation layer
    // can catch the missing header.
    if root_is_error && lines.is_empty() {
        errors.report(
            ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len().max(1)),
                ErrorContext::new(input, 0..input.len().max(1), input),
                "Unparsable content: file structure is not valid CHAT and no lines could be recovered",
            )
            .with_suggestion("CHAT files must contain @UTF8, @Begin, @Participants, @Languages, and @End headers"),
        );
    }

    // Whole-tree recovery-node backstop. Gated on `has_error()` so valid files
    // (the overwhelming majority) pay nothing. Every surviving ERROR/MISSING node
    // not already covered by a region diagnostic above is surfaced as invalidity
    // (ERROR -> E316, MISSING -> E342). The parser still produced an AST; this only
    // reports, honoring lenient recovery while enforcing "recovery is not validity".
    if root_node.has_error() {
        let reported = collector.to_vec();
        let mut candidates = Vec::new();
        collect_recovery_nodes(root_node, input, &mut candidates);
        for candidate in candidates {
            // Widen a zero-width MISSING span to one byte so it can intersect a
            // reported span that merely touches its point. A candidate already
            // covered by a (richer) region diagnostic is suppressed.
            let span = candidate.location.span;
            let probe = Span::new(span.start, span.end.max(span.start.saturating_add(1)));
            if !reported.iter().any(|e| e.location.span.overlaps(probe)) {
                errors.report(candidate);
            }
        }
    }

    info!("Parsed {} lines", lines.len());

    (lines, Some(tree_to_return))
}
