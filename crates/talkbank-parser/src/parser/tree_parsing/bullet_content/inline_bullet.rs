//! Parser for structured `bullet` nodes in tier text.

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::media_bullet::parse_bullet_node_timestamps;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Converts one structured `bullet` node to `(start_ms, end_ms)`.
pub(super) fn parse_inline_bullet(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<(u64, u64)> {
    let (start_ms, end_ms) = match parse_bullet_node_timestamps(node, source, errors) {
        Ok(times) => times,
        // "could not extract timestamps" said nothing a reader could act on,
        // and its context carried an empty string where the bullet text
        // belongs. The rejection knows which of the four routes it took.
        Err(why) => {
            crate::parser::tree_parsing::media_bullet::report_bullet_rejection(
                node, source, &why, errors,
            );
            return ParseOutcome::rejected();
        }
    };

    if start_ms == 0 && end_ms == 0 {
        errors.report(ParseError::new(
            ErrorCode::InvalidMediaBullet,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "Invalid bullet: both start and end timestamps are 0",
        ));
        return ParseOutcome::rejected();
    }

    ParseOutcome::parsed((start_ms, end_ms))
}
