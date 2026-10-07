//! Parsing for media bullets embedded in main-tier content.

use crate::error::ErrorSink;
use crate::generated_traversal::{AsRawNode, BulletNode, SourceBound};
use crate::model::UtteranceContent;
use crate::parser::tree_parsing::media_bullet::parse_bullet_node;
use talkbank_model::ParseOutcome;

/// Converts a structured `bullet` node into `UtteranceContent::InternalBullet`.
pub(crate) fn parse_internal_bullet<'tree>(
    typed: SourceBound<'tree, '_, BulletNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let node = typed.raw_node();
    let source = typed.source();
    let bullet = match parse_bullet_node(typed, errors) {
        Ok(bullet) => bullet,
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

    ParseOutcome::parsed(UtteranceContent::InternalBullet(bullet))
}
