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
use crate::generated_traversal::{Absence, RecoveryNode};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
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
pub(crate) fn present<'a, 'tree, T, M, U, A>(
    slot: &'a crate::generated_traversal::NodeSlot<'tree, T, M, U, A>,
) -> Option<&'a T> {
    use crate::generated_traversal::NodeSlot;
    match slot {
        NodeSlot::Present(value) => Some(value),
        NodeSlot::Missing(_)
        | NodeSlot::Error(_)
        | NodeSlot::Unexpected(_)
        | NodeSlot::Absent(_) => None,
    }
}

/// What a typed position held, once its recovery states have been reported.
///
/// `Recovered` is a MISSING placeholder or an ERROR or displaced node, already
/// reported through [`expect_present`]; the caller decides what a reported
/// recovery means for the construct (a `%mor` tier rejects itself on any).
#[derive(Debug)]
pub(crate) enum SlotState<'a, T> {
    /// The node the grammar expected here.
    Present(&'a T),
    /// The optional position is empty.
    Absent,
    /// A recovery state, reported.
    Recovered,
}

/// A typed position whose Present node carries something to parse, with
/// every recovery state reported in one place: a MISSING placeholder
/// through [`check_not_missing`] with `context`, an ERROR or a displaced
/// node through [`unexpected_node_error`] with the same `context`. One owner
/// for the four-arm match the `%mor` word and tier parsers had each written
/// per position (eleven copies between them).
pub(crate) fn expect_present<'a, 'tree, T, M, U, A>(
    slot: &'a crate::generated_traversal::NodeSlot<'tree, T, M, U, A>,
    context: &str,
    source: &str,
    errors: &impl ErrorSink,
) -> SlotState<'a, T>
where
    M: RecoveryNode<'tree>,
    U: RecoveryNode<'tree>,
    A: Absence,
{
    use crate::generated_traversal::NodeSlot;
    match slot {
        NodeSlot::Present(value) => SlotState::Present(value),
        NodeSlot::Absent(absent) => absent.when_absent(SlotState::Absent),
        NodeSlot::Missing(missing) => {
            check_not_missing(missing.node(), source, errors, context);
            SlotState::Recovered
        }
        NodeSlot::Error(bad) => {
            errors.report(unexpected_node_error(*bad, source, context));
            SlotState::Recovered
        }
        NodeSlot::Unexpected(bad) => {
            errors.report(unexpected_node_error(bad.node(), source, context));
            SlotState::Recovered
        }
    }
}

/// A typed position whose Present node carries nothing to parse: a separator
/// comma, the whitespace around it, a delimiter. Present and Absent need
/// nothing; a MISSING placeholder is reported as the recovery it is, through
/// [`check_not_missing`] with `context`; an ERROR or a displaced node means
/// the enclosing shape broke at that node, which `on_bad` reports in the
/// words for that position. One owner for the three-arm match that the
/// `@Participants` and `@Languages` list walkers had each written out per
/// structural slot.
pub(crate) fn expect_structure<'tree, T, M, U, A>(
    slot: &crate::generated_traversal::NodeSlot<'tree, T, M, U, A>,
    context: &str,
    source: &str,
    errors: &impl ErrorSink,
    on_bad: impl FnOnce(Node),
) where
    M: RecoveryNode<'tree>,
    U: RecoveryNode<'tree>,
{
    use crate::generated_traversal::NodeSlot;
    match slot {
        NodeSlot::Present(_) | NodeSlot::Absent(_) => {}
        NodeSlot::Missing(missing) => {
            check_not_missing(missing.node(), source, errors, context);
        }
        NodeSlot::Error(bad) => on_bad(*bad),
        NodeSlot::Unexpected(bad) => on_bad(bad.node()),
    }
}

/// A typed delimiter position (`<`, `>`, a quotation mark, a group bracket).
/// Present needs nothing and so does Absent; a MISSING placeholder is left to
/// the whole-tree recovery backstop, which already reports every MISSING
/// node once, so reporting it here again would double the diagnostic (the
/// old `kind()` walks let a MISSING delimiter through silently for the same
/// reason, if by accident); an ERROR or a displaced node is the construct
/// losing its shape at that node, which `on_bad` reports.
///
/// The difference from [`expect_structure`] is the MISSING policy, and it
/// is deliberate: the list walkers had always reported their MISSING commas
/// locally, and the bracketed constructs never had.
pub(crate) fn expect_delimiter<'tree, T, M, U: RecoveryNode<'tree>, A>(
    slot: &crate::generated_traversal::NodeSlot<'tree, T, M, U, A>,
    on_bad: impl FnOnce(Node),
) {
    use crate::generated_traversal::NodeSlot;
    match slot {
        NodeSlot::Present(_) | NodeSlot::Missing(_) | NodeSlot::Absent(_) => {}
        NodeSlot::Error(bad) => on_bad(*bad),
        NodeSlot::Unexpected(bad) => on_bad(bad.node()),
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
