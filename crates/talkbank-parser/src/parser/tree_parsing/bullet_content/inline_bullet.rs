//! Parser for structured `bullet` nodes in tier text.

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{AsRawNode, BulletNode, SourceBound};
use crate::parser::tree_parsing::media_bullet::parse_bullet_node_timestamps;
use talkbank_model::ParseOutcome;

/// Inline-tier policy excludes the all-zero pair, independently of time ordering.
struct InlineBulletTimes((u64, u64));

impl InlineBulletTimes {
    fn admit(times: (u64, u64)) -> Option<Self> {
        if times == (0, 0) {
            None
        } else {
            Some(Self(times))
        }
    }

    fn into_pair(self) -> (u64, u64) {
        self.0
    }
}

/// Converts one structured `bullet` node to `(start_ms, end_ms)`.
pub(super) fn parse_inline_bullet<'tree>(
    typed: SourceBound<'tree, '_, BulletNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<(u64, u64)> {
    let node = typed.raw_node();
    let source = typed.source();
    let (start_ms, end_ms) = match parse_bullet_node_timestamps(typed.node(), source, errors) {
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

    let Some(times) = InlineBulletTimes::admit((start_ms, end_ms)) else {
        errors.report(ParseError::new(
            ErrorCode::InvalidMediaBullet,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "Invalid bullet: both start and end timestamps are 0",
        ));
        return ParseOutcome::rejected();
    };

    ParseOutcome::parsed(times.into_pair())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::error::ErrorCollector;
    use crate::generated_traversal::FromNodeKind;

    #[test]
    fn real_inline_bullets_preserve_the_zero_pair_policy() {
        let parser = TreeSitterParser::new().expect("grammar");
        for times in [(0, 0), (0, 1), (1, 0), (1, 2)] {
            // Each case is parsed from its own source: no substitution of the
            // timestamp text underneath an already-produced tree.
            let fixture = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../talkbank-parser-tests/tests/error_corpus/validation_errors/E360_4.cha"
            ));
            let source = fixture.replace(
                "\u{15}0_0\u{15}",
                &format!("\u{15}{}_{}\u{15}", times.0, times.1),
            );
            let parsed = parser
                .parse_source_incremental(&source, None)
                .expect("parse");
            assert!(!parsed.root_node().has_error());
            let mut pending = vec![parsed.root_node()];
            let mut witnessed = 0;
            while let Some(node) = pending.pop() {
                let mut cursor = node.walk();
                pending.extend(node.children(&mut cursor));
                let Some(_) = BulletNode::from_node(node) else {
                    continue;
                };
                let errors = ErrorCollector::new();
                let bound = parsed
                    .bind(node)
                    .expect("canonical bullet range")
                    .typed::<BulletNode>()
                    .expect("bullet kind");
                let result = parse_inline_bullet(bound, &errors).into_option();
                let diagnostics = errors.into_vec();
                if times == (0, 0) {
                    assert!(result.is_none());
                    assert_eq!(diagnostics.len(), 1);
                    assert_eq!(diagnostics[0].code, ErrorCode::InvalidMediaBullet);
                } else {
                    assert_eq!(result, Some(times));
                    assert!(diagnostics.is_empty());
                }
                // No independent source argument exists at this leaf boundary.
                witnessed += 1;
            }
            assert_eq!(witnessed, 1);
        }
    }
}
