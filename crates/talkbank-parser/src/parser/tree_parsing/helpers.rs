//! Cross-cutting helper functions for CST error interpretation.
//!
//! These helpers translate generic tree-sitter recovery artifacts (`ERROR`,
//! `MISSING`) into TalkBank-specific parser diagnostics with actionable messages.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{AsRawNode, SourceSlice};
use tree_sitter::Node;

/// A node range admitted against its retained source. This proves readable
/// UTF-8, not identity with the tree's original input or validity of recovery.
pub(crate) struct ReadableRecovery<'tree, 'source> {
    node: Node<'tree>,
    source: &'source str,
    text: &'source str,
}

impl<'tree, 'source> ReadableRecovery<'tree, 'source> {
    pub(crate) fn from_bound(bound: SourceSlice<'tree, 'source>) -> Self {
        Self {
            node: bound.raw_node(),
            source: bound.source(),
            text: bound.text(),
        }
    }

    pub(crate) fn admit(node: Node<'tree>, source: &'source str) -> Option<Self> {
        let text = source.get(node.byte_range())?;
        Some(Self { node, source, text })
    }

    pub(crate) fn node(&self) -> Node<'tree> {
        self.node
    }
    pub(crate) fn source(&self) -> &'source str {
        self.source
    }
    pub(crate) fn text(&self) -> &'source str {
        self.text
    }

    /// Build a diagnostic located in the source but displaying this recovery
    /// fragment. Neither the node span nor the context text can be supplied
    /// independently of the admitted readable range.
    pub(crate) fn fragment_diagnostic(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
    ) -> ParseError {
        ParseError::new(
            code,
            Severity::Error,
            SourceLocation::from_offsets(self.node.start_byte(), self.node.end_byte()),
            ErrorContext::new(self.text, 0..self.text.len(), self.text),
            message,
        )
    }
}

/// Analyze a producer-bound recovery slice without re-admitting its text.
pub(crate) fn analyze_bound_error_node(bound: SourceSlice<'_, '_>, context: &str) -> ParseError {
    analyze_readable_error(ReadableRecovery::from_bound(bound), context)
}

pub(crate) fn analyze_error_node(node: Node, source: &str, context: &str) -> ParseError {
    let recovery = match ReadableRecovery::admit(node, source) {
        Some(recovery) => recovery,
        None => {
            return crate::parser::typed_cst::cst_failure_diagnostic(
                node,
                source,
                crate::generated_traversal::SourceBindingError::InvalidRange,
            );
        }
    };
    analyze_readable_error(recovery, context)
}

fn analyze_readable_error(recovery: ReadableRecovery<'_, '_>, context: &str) -> ParseError {
    let node = recovery.node();
    let source = recovery.source();
    let error_text = recovery.text();
    let is_whitespace_only = error_text.chars().all(|c| c.is_whitespace());

    // Special case: Empty ERROR node (grammar mismatch)
    if is_whitespace_only {
        return ParseError::new(
            ErrorCode::UnexpectedSyntax,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            format!("Unexpected syntax in {}", context),
        )
        .with_suggestion("Check for missing or malformed elements");
    }

    // Generic fallback: Show what was found
    recovery
        .fragment_diagnostic(
            ErrorCode::UnparsableContent,
            format!(
                "Unparsable content in {}: '{}'",
                context,
                if error_text.chars().count() > 30 {
                    let truncated: String = error_text.chars().take(30).collect();
                    format!("{}...", truncated)
                } else {
                    error_text.to_string()
                },
            ),
        )
        .with_suggestion("Check CHAT format specification for valid syntax in this context")
}

/// Create a standardized error for unexpected nodes encountered during parsing.
///
/// This helper detects ERROR nodes (from tree-sitter parse failures) and analyzes
/// them to provide user-friendly messages. For non-ERROR nodes, it reports the
/// unexpected node kind with context.
///
/// **CRITICAL**: This function ensures ERROR nodes are NEVER shown to users directly.
/// Instead, it analyzes the error content to provide actionable, user-friendly messages.
pub(crate) fn unexpected_node_error(node: Node, source: &str, context: &str) -> ParseError {
    // Special handling for ERROR nodes - analyze content for user-friendly message
    if node.is_error() {
        return analyze_error_node(node, source, context);
    }

    // Regular unexpected node (not an ERROR) - report the kind
    ParseError::new(
        ErrorCode::UnexpectedNodeInContext,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
        format!("Unexpected '{}' in {}", node.kind(), context),
    )
    .with_suggestion("This element is not valid in this context")
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;

    /// A recovery slice proves location, not an unclosed delimiter.
    #[test]
    fn lone_bracket_recovery_retains_location_without_guessing_syntax() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../talkbank-parser-tests/tests/error_corpus/validation_errors/E312_2.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut witnessed = false;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            if node.is_error() && source.get(node.byte_range()) == Some("[") {
                let diagnostic = analyze_error_node(node, source, "parse tree");
                assert_eq!(diagnostic.code, ErrorCode::UnparsableContent);
                assert_eq!(
                    diagnostic.location.span,
                    crate::error::Span::from_usize(node.start_byte(), node.end_byte())
                );
                let split_utf8 = "é".repeat(node.end_byte());
                for incompatible in ["", split_utf8.as_str()] {
                    assert!(ReadableRecovery::admit(node, incompatible).is_none());
                    assert_eq!(
                        analyze_error_node(node, incompatible, "parse tree").code,
                        ErrorCode::InternalError,
                    );
                }
                witnessed = true;
            }
        }
        assert!(
            witnessed,
            "the retained E312 spec must reach a lone-bracket ERROR"
        );
    }
}
