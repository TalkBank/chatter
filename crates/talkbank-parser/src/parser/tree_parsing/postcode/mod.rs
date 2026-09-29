//! Parser for main-tier postcodes (`[+ ... ]`).
//!
//! # Related CHAT Manual Sections
//!
//! - CHAT manual, "Utterance level error coding (post-codes)" (section 18.2;
//!   it has no named anchor, `#_Toc208734963` in the 2026-09-03 build)
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{AsRawNode, PostcodeNode, SourceBound};
use crate::model::Postcode;
use talkbank_model::ParseOutcome;

/// Parse a single postcode node [+ text].
///
/// **Grammar Rule (coarsened):**
/// ```text
/// postcode: $ => token(/\[\+ [^\]\r\n]+\]/)
/// ```
///
/// The node is now a single leaf token. Extract the code by stripping
/// the `[+ ` prefix and `]` suffix, then trimming trailing whitespace.
pub fn parse_postcode_node<'tree>(
    typed: SourceBound<'tree, '_, PostcodeNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Postcode> {
    let node = typed.raw_node();
    let source = typed.source();
    let text = typed.text();

    // Strip "[+ " prefix and "]" suffix
    let code = text
        .strip_prefix("[+ ")
        .and_then(|s| s.strip_suffix(']'))
        .map(|s| s.trim_end());

    match code {
        Some(c) if !c.is_empty() => ParseOutcome::parsed(Postcode::new(c).with_span(
            talkbank_model::Span::from_usize(node.start_byte(), node.end_byte()),
        )),
        _ => {
            errors.report(ParseError::new(
                ErrorCode::InvalidPostcode,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "postcode"),
                "Postcode is missing required content".to_string(),
            ));
            ParseOutcome::rejected()
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::error::ErrorCollector;
    use crate::generated_traversal::FromNodeKind;

    #[test]
    fn real_postcodes_require_their_original_parse_owner() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/postcodes-and-freecodes.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let foreign = parser
            .parse_source_incremental(source, None)
            .expect("independent equal-bytes parse");
        let mut pending = vec![parsed.root_node()];
        let mut checked = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(postcode) = PostcodeNode::from_node(node) else {
                continue;
            };
            let errors = ErrorCollector::new();
            let bound = parsed.bind_typed(postcode).expect("original owner");
            let ParseOutcome::Parsed(value) = parse_postcode_node(bound, &errors) else {
                panic!("reference postcode must parse");
            };
            assert_eq!(
                value.span,
                talkbank_model::Span::from_usize(node.start_byte(), node.end_byte())
            );
            assert!(errors.to_vec().is_empty());
            assert!(
                foreign.bind_typed(postcode).is_err(),
                "equal text is not the same parse owner"
            );
            checked += 1;
        }
        assert!(checked > 0, "fixture must exercise postcode admission");
    }
}
