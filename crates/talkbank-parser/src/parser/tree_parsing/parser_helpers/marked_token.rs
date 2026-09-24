//! Grammar tokens that begin with a marker their rule guarantees.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::typed_cst::{NodeTextError, admit_node_text};
use tree_sitter::Node;

/// The text of a grammar token past the marker its rule begins with.
///
/// The grammar guarantees the marker (`$` on a `pos_tag`, `@` on a
/// `form_marker`, `@s` on a `word_lang_suffix`, `%x` on an `x_tier_prefix`,
/// `%` on an `unsupported_tier_prefix`), so a token without it, or one
/// that is not UTF-8, does not fit its own grammar: that is the traversal's
/// failure, reported as such (E330, naming the token as `what` calls it)
/// and read as nothing, never read around. Until 2026-09-09 each site stripped
/// its marker behind a fallback (`strip_prefix(..).unwrap_or(text)`, or
/// `unwrap_or("")`), so a bare payload read as a declared one and a token
/// that was not an `@s` suffix at all became the bare `@s` shortcut.
pub(crate) fn after_marker<'a>(
    node: Node,
    marker: &str,
    what: &str,
    source: &'a str,
    errors: &impl ErrorSink,
) -> Option<&'a str> {
    let fault = |message: String| {
        ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            message,
        )
    };
    let text = match admit_node_text(node, source) {
        Ok(text) => text,
        Err(NodeTextError::OutsideSource) => {
            errors.report(fault(format!(
                "{what} node range is outside the supplied source"
            )));
            return None;
        }
        Err(NodeTextError::InvalidUtf8(error)) => {
            errors.report(
                fault(format!("{what} is not valid UTF-8: {error}")).with_suggestion(
                    "The source file may contain invalid UTF-8 sequences. Ensure the file is \
                     properly encoded as UTF-8.",
                ),
            );
            return None;
        }
    };
    let Some(rest) = text.strip_prefix(marker) else {
        errors.report(fault(format!("{what} does not begin with '{marker}'")));
        return None;
    };
    Some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::error::{ErrorCollector, Span};
    use crate::generated_traversal::{FormMarkerNode, FromNodeKind};

    /// These are source-compatibility boundary witnesses, not malformed tokens
    /// produced by the grammar: the CST always comes from the retained fixture.
    #[test]
    fn real_marker_requires_readable_range_and_declared_prefix() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/words-special-forms.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut checked = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            if FormMarkerNode::from_node(node).is_none()
                || source.get(node.byte_range()) != Some("@wp")
            {
                continue;
            }
            checked += 1;
            let errors = ErrorCollector::new();
            assert_eq!(
                after_marker(node, "@", "Form marker", source, &errors),
                Some("wp")
            );
            assert!(errors.to_vec().is_empty());
            // Odd token width forces at least one boundary through a code point.
            let foreign_utf8 = "é".repeat(source.len());
            let mut wrong_marker = source.to_owned();
            wrong_marker.replace_range(node.byte_range(), "!wp");
            for (input, message) in [
                ("", "outside the supplied source"),
                (foreign_utf8.as_str(), "not valid UTF-8"),
                (wrong_marker.as_str(), "does not begin with '@'"),
            ] {
                let errors = ErrorCollector::new();
                assert!(after_marker(node, "@", "Form marker", input, &errors).is_none());
                let diagnostics = errors.into_vec();
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, ErrorCode::TreeParsingError);
                assert_eq!(
                    diagnostics[0].location.span,
                    Span::from_usize(node.start_byte(), node.end_byte())
                );
                assert!(diagnostics[0].message.contains(message));
            }
        }
        assert!(checked > 0, "fixture must supply the word-play marker");
    }
}
