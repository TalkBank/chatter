//! Parsing for overlap-point tokens (`⌈`, `⌉`, `⌊`, `⌋` with optional indices).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{AsRawNode, OverlapPointNode, SourceBound};
use crate::model::{OverlapIndex, OverlapPoint, OverlapPointKind, UtteranceContent};
use crate::parser::node_span::span_of;
use talkbank_model::ParseOutcome;

/// Converts one overlap-point token node into `UtteranceContent`.
///
/// **Grammar Rule:**
/// ```text
/// overlap_point: $ => token(prec(10, choice(
///   seq('\u2308', optional(/[2-9]/)),  // ⌈ or ⌈2..⌈9
///   seq('\u2309', optional(/[2-9]/)),  // ⌉ or ⌉2..⌉9
///   seq('\u230A', optional(/[2-9]/)),  // ⌊ or ⌊2..⌊9
///   seq('\u230B', optional(/[2-9]/))   // ⌋ or ⌋2..⌋9
/// )))
/// ```
///
/// **Atomic token structure:**
/// - Overlap markers are single atomic tokens (e.g., "⌈", "⌊2", "⌉3")
/// - Index (2-9) is embedded in the token text, not a separate child node
/// - Parser extracts marker kind and optional index from token text
pub(crate) fn parse_overlap_point<'tree>(
    typed: SourceBound<'tree, '_, OverlapPointNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    match parse_overlap_point_token(typed, errors) {
        ParseOutcome::Parsed(point) => ParseOutcome::parsed(UtteranceContent::OverlapPoint(point)),
        ParseOutcome::Rejected => ParseOutcome::rejected(),
    }
}

/// The overlap point an `overlap_point` token names, wherever the token
/// sits: as a content item of its own (the caller above) or inside a word,
/// as in `butt⌈er⌉`, where the word converter used to keep a second copy of
/// the marker table with a `TopOverlapBegin` fallback for a character the
/// grammar cannot produce.
pub(crate) fn parse_overlap_point_token<'tree>(
    typed: SourceBound<'tree, '_, OverlapPointNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<OverlapPoint> {
    let node = typed.raw_node();
    let source = typed.source();
    let text = typed.text();

    let mut chars = text.chars();

    // First character: overlap marker (⌈⌉⌊⌋)
    let marker = match chars.next() {
        Some(c) => c,
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                DiagnosticLocation::new(span_of(node)),
                ErrorContext::new(source, span_of(node), text),
                "Empty overlap point token".to_string(),
            ));
            return ParseOutcome::rejected();
        }
    };

    // Map marker character to kind
    let kind = match marker {
        '\u{2308}' => OverlapPointKind::TopOverlapBegin, // ⌈
        '\u{2309}' => OverlapPointKind::TopOverlapEnd,   // ⌉
        '\u{230A}' => OverlapPointKind::BottomOverlapBegin, // ⌊
        '\u{230B}' => OverlapPointKind::BottomOverlapEnd, // ⌋
        other => {
            errors.report(ParseError::new(
                ErrorCode::UnbalancedOverlap,
                Severity::Error,
                DiagnosticLocation::new(span_of(node)),
                ErrorContext::new(source, span_of(node), text),
                format!(
                    "Unknown overlap point marker '{}' (U+{:04X})",
                    other, other as u32
                ),
            ));
            return ParseOutcome::rejected();
        }
    };

    // Second character (optional): index digit 1-9
    let index = chars
        .next()
        .and_then(|digit_char| digit_char.to_digit(10).map(OverlapIndex::new));

    let span = span_of(node);
    ParseOutcome::parsed(OverlapPoint::new(span, kind, index))
}
