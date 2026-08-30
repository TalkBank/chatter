#![allow(dead_code)]
//! CST Structure Assertions
//!
//! These functions verify that tree-sitter CST nodes match expected grammar structure.
//! When the grammar changes, these assertions will loudly fail instead of silently
//! producing incorrect parses.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Assert that a node has exactly the expected number of children
///
/// **Purpose:** Catch grammar changes that add/remove children
///
/// # Example
/// ```ignore
/// // Grammar: seq('%', 'mor', ':', '\t', mor_contents, '\n')
/// // Expected: 6 children (positions 0-5)
/// assert_child_count_exact(node, 6, source, errors, "mor_dependent_tier");
/// ```
pub fn assert_child_count_exact(
    node: Node,
    expected: u32,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> bool {
    let actual = node.child_count();
    if actual != expected {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!(
                "CST structure mismatch in {}: expected {} children, found {}. Grammar may have changed!",
                context, expected, actual
            ),
        ).with_suggestion(format!(
            "Check tree-sitter grammar for '{}' - structure has changed", node.kind()
        )));
        return false;
    }
    true
}

/// Assert that a node has at least the expected number of children
///
/// **Purpose:** Catch grammar changes that remove required children
pub fn assert_child_count_min(
    node: Node,
    minimum: u32,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> bool {
    let actual = node.child_count();
    if actual < minimum {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!(
                "CST structure mismatch in {}: expected at least {} children, found {}. Grammar may have changed!",
                context, minimum, actual
            ),
        ).with_suggestion(format!(
            "Check tree-sitter grammar for '{}' - structure has changed", node.kind()
        )));
        return false;
    }
    true
}

/// Assert that child at position has expected kind
///
/// **Purpose:** Catch when grammar changes reorder children or change types
///
/// # Example
/// ```ignore
/// // Grammar: seq('%', 'mor', ':', '\t', mor_contents, '\n')
/// // Position 4 should be mor_contents
/// assert_child_kind(node, 4, "mor_contents", source, errors, "mor_dependent_tier");
/// ```
pub fn assert_child_kind(
    node: Node,
    position: u32,
    expected_kind: &str,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> bool {
    if let Some(child) = node.child(position) {
        let actual_kind = child.kind();
        if actual_kind != expected_kind {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), actual_kind),
                format!(
                    "CST structure mismatch in {} at position {}: expected '{}', found '{}'. Grammar may have changed!",
                    context, position, expected_kind, actual_kind
                ),
            ).with_suggestion(format!(
                "Check tree-sitter grammar for '{}' - child at position {} has changed", node.kind(), position
            )));
            return false;
        }
        true
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!(
                "CST structure mismatch in {}: no child at position {}. Grammar may have changed!",
                context, position
            ),
        ).with_suggestion(format!(
            "Check tree-sitter grammar for '{}' - expected {} children", node.kind(), position + 1
        )));
        false
    }
}

/// Assert that child at position matches one of several expected kinds
///
/// **Purpose:** Handle cases where multiple node types are valid at a position
pub fn assert_child_kind_one_of(
    node: Node,
    position: u32,
    expected_kinds: &[&str],
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> bool {
    if let Some(child) = node.child(position) {
        let actual_kind = child.kind();
        if !expected_kinds.contains(&actual_kind) {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), actual_kind),
                format!(
                    "CST structure mismatch in {} at position {}: expected one of {:?}, found '{}'. Grammar may have changed!",
                    context, position, expected_kinds, actual_kind
                ),
            ).with_suggestion(format!(
                "Check tree-sitter grammar for '{}' - child at position {} has changed", node.kind(), position
            )));
            return false;
        }
        true
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!(
                "CST structure mismatch in {}: no child at position {}. Grammar may have changed!",
                context, position
            ),
        ).with_suggestion(format!(
            "Check tree-sitter grammar for '{}' - expected {} children", node.kind(), position + 1
        )));
        false
    }
}

/// Get child at position or report detailed error
///
/// **Purpose:** Safe child access that reports exactly what went wrong
///
/// Returns `None` if child doesn't exist, kind doesn't match, or node is MISSING (error already reported)
///
/// **CRITICAL**: This function checks for MISSING nodes (tree-sitter error recovery placeholders)
/// and reports them as errors. MISSING nodes have the expected `kind()` but zero-length span.
pub fn expect_child<'a>(
    node: Node<'a>,
    position: u32,
    expected_kind: &str,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> ParseOutcome<Node<'a>> {
    if let Some(child) = node.child(position) {
        // CRITICAL: Check for MISSING nodes first - these have the expected kind but are placeholders
        if child.is_missing() {
            errors.report(ParseError::new(
                ErrorCode::MissingRequiredElement,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), child.kind()),
                format!(
                    "Tree-sitter error recovery: MISSING '{}' node inserted at {} position {}",
                    expected_kind, context, position
                ),
            ).with_suggestion(
                "This CHAT construct appears to be invalid or malformed. Check the CHAT format specification for correct syntax."
            ).with_help_url("https://talkbank.org/0info/manuals/CHAT.html"));
            return ParseOutcome::rejected();
        }

        if assert_child_kind(node, position, expected_kind, source, errors, context) {
            ParseOutcome::parsed(child)
        } else {
            ParseOutcome::rejected()
        }
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!(
                "CST structure mismatch in {}: no child at position {}. Grammar may have changed!",
                context, position
            ),
        ).with_suggestion(format!(
            "Check tree-sitter grammar for '{}' - expected at least {} children", node.kind(), position + 1
        )));
        ParseOutcome::rejected()
    }
}

/// Get child at position without kind checking (for when kind can vary)
///
/// **Purpose:** Safe child access that just checks existence
///
/// **CRITICAL**: This function checks for MISSING nodes (tree-sitter error recovery placeholders)
/// and reports them as errors.
pub fn expect_child_at<'a>(
    node: Node<'a>,
    position: u32,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> ParseOutcome<Node<'a>> {
    if let Some(child) = node.child(position) {
        // CRITICAL: Check for MISSING nodes - these are placeholders from error recovery
        if child.is_missing() {
            errors.report(ParseError::new(
                ErrorCode::MissingRequiredElement,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), child.kind()),
                format!(
                    "Tree-sitter error recovery: MISSING '{}' node inserted at {} position {}",
                    child.kind(), context, position
                ),
            ).with_suggestion(
                "This CHAT construct appears to be invalid or malformed. Check the CHAT format specification for correct syntax."
            ).with_help_url("https://talkbank.org/0info/manuals/CHAT.html"));
            return ParseOutcome::rejected();
        }
        ParseOutcome::parsed(child)
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!(
                "CST structure mismatch in {}: no child at position {}. Grammar may have changed!",
                context, position
            ),
        ).with_suggestion(format!(
            "Check tree-sitter grammar for '{}' - expected at least {} children", node.kind(), position + 1
        )));
        ParseOutcome::rejected()
    }
}

/// Check if a node is a MISSING placeholder and report error if so
///
/// **Purpose:** Inline check for MISSING nodes when not using expect_child helpers
///
/// Returns `true` if node is valid (not MISSING), `false` if MISSING (error already reported)
pub fn check_not_missing(node: Node, source: &str, errors: &impl ErrorSink, context: &str) -> bool {
    if node.is_missing() {
        errors.report(ParseError::new(
            ErrorCode::MissingRequiredElement,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!(
                "Tree-sitter error recovery: MISSING '{}' node inserted in {}",
                node.kind(),
                context
            ),
        ).with_suggestion(
            "This CHAT construct appears to be invalid or malformed. Check the CHAT format specification for correct syntax."
        ).with_help_url("https://talkbank.org/0info/manuals/CHAT.html"));
        false
    } else {
        true
    }
}

/// Extract UTF-8 text from a node with proper error reporting
///
/// **Purpose:** Replace silent fallback extraction with proper error handling
///
/// # Arguments
/// * `node` - The CST node to extract text from
/// * `source` - The source text
/// * `errors` - Error sink for reporting UTF-8 failures
/// * `context` - Context string for error messages
/// * `fallback` - Fallback text if UTF-8 extraction fails
///
/// # Example
/// ```ignore
/// let text = extract_utf8_text(node, source, errors, "word_text", "");
/// // If UTF-8 fails, error is reported and fallback is returned
/// ```
pub fn extract_utf8_text<'a>(
    node: Node,
    source: &'a str,
    errors: &impl ErrorSink,
    context: &str,
    fallback: &'a str,
) -> &'a str {
    match node.utf8_text(source.as_bytes()) {
        Ok(text) => text,
        Err(e) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
                format!(
                    "UTF-8 decoding error in {}: {}",
                    context, e
                ),
            ).with_suggestion(
                "The source file may contain invalid UTF-8 sequences. Ensure the file is properly encoded as UTF-8."
            ));
            fallback
        }
    }
}

/// The value of a slot that is `Present`, borrowed, or `None` for every
/// recovery state.
///
/// `NodeSlot::present_or_recover` CONSUMES the slot, and slots are reached
/// through `Positioned::slot()`, which borrows. Eight call sites bridged that
/// with `.slot().clone().present_or_recover().ok()`, a clone taken solely to
/// satisfy a by-value signature and then thrown away, which also forced a
/// `T: Clone` bound onto two generic helpers that had no other use for it.
///
/// This is the borrowing form, exhaustive over all five states in ONE place, so
/// the recovery states stay spelled out (the crate bans `_` arms on this enum)
/// without spelling them out eight times.
///
/// Use `present_or_recover` where the RECOVERY variant is wanted; use this
/// where the call site only asks "is it there".
pub(crate) fn present<'a, 'tree, T>(
    slot: &'a crate::generated_traversal::NodeSlot<'tree, T>,
) -> Option<&'a T> {
    use crate::generated_traversal::NodeSlot;
    match slot {
        NodeSlot::Present(value) => Some(value),
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
            None
        }
    }
}

/// The first direct child of `node` whose kind is `kind`.
///
/// One owner for a five-line idiom that had grown three byte-identical private
/// copies (header dispatch, the `@Types` header, the `@PID` header) plus a
/// fourth inline use. None of them is reachable from the others, so each new
/// need produced another copy; that is how the third one came to exist.
///
/// This is NOT the banned `node.kind()` hand-walk. That ban is about driving
/// the PARSE by scanning kinds instead of the generated typed traversal. This
/// looks inside a node the traversal does not type: an ERROR node's recovered
/// children, or a header whose internals predate the migration.
pub(crate) fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
