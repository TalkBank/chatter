//! Parsing for media bullets embedded in main-tier content.

use crate::error::{ErrorSink, Span};
use crate::model::UtteranceContent;
use crate::parser::tree_parsing::media_bullet::parse_bullet_node_timestamps;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Converts a structured `bullet` node into `UtteranceContent::InternalBullet`.
pub(crate) fn parse_internal_bullet(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
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

    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let bullet = crate::model::Bullet::new(start_ms, end_ms).with_span(span);
    ParseOutcome::parsed(UtteranceContent::InternalBullet(bullet))
}
