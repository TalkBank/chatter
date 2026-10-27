//! Checked CST recovery and transitional text boundaries.
//!
//! Generated slots describe grammar structure. These helpers report recovery
//! states and reject failed reads without manufacturing successful values.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{RecoveryNode, SourceBindingError};
use crate::parser::node_span::span_of;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Check if a node is a MISSING placeholder and report error if so
///
/// **Purpose:** Report MISSING nodes at raw-node boundaries not yet using typed slots.
///
/// Returns `true` if node is valid (not MISSING), `false` if MISSING (error already reported)
pub fn check_not_missing(node: Node, source: &str, errors: &impl ErrorSink, context: &str) -> bool {
    if node.is_missing() {
        errors.report(ParseError::new(
            ErrorCode::MissingRequiredElement,
            Severity::Error,
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(source, span_of(node), node.kind()),
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

/// Read a node range or report rejection; failure cannot manufacture text.
///
/// This transitional raw-node boundary checks bounds and UTF-8 boundaries.
/// It does not certify source provenance: migrated consumers use the generated
/// producer-bound SourceSlice instead of passing an independent string.
pub fn extract_utf8_text<'a>(
    node: Node,
    source: &'a str,
    errors: &impl ErrorSink,
    context: &str,
) -> ParseOutcome<&'a str> {
    match source.get(node.byte_range()) {
        Some(text) => ParseOutcome::parsed(text),
        None => {
            errors.report(crate::parser::typed_cst::cst_failure_diagnostic(
                node,
                source,
                SourceBindingError::InvalidRange,
            ).with_suggestion(format!(
                "While reading {context}, use the source that produced this tree; node ranges must be valid UTF-8 boundaries in that source."
            )));
            ParseOutcome::rejected()
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
pub(crate) fn present<T, M, E, U, A>(
    slot: &crate::generated_traversal::NodeSlot<T, M, E, U, A>,
) -> Option<&T> {
    use crate::generated_traversal::NodeSlot;
    match slot {
        NodeSlot::Present(value) => Some(value),
        NodeSlot::Missing(_)
        | NodeSlot::Error(_)
        | NodeSlot::Unexpected(_)
        | NodeSlot::Absent(_) => None,
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
/// The MISSING policy is deliberate: list walkers and MOR pipe separators
/// report their missing structural tokens locally; bracketed constructs do not.
pub(crate) fn expect_delimiter<'tree>(slot: &impl DelimiterSlot<'tree>, on_bad: impl FnOnce(Node)) {
    if let Some(bad) = slot.displaced() {
        on_bad(bad);
    }
}

/// Normalize raw and range-admitted positions without changing delimiter policy.
pub(crate) trait DelimiterSlot<'tree> {
    fn displaced(&self) -> Option<Node<'tree>>;
}

impl<'tree, T, M, E: RecoveryNode<'tree>, U: RecoveryNode<'tree>, A> DelimiterSlot<'tree>
    for crate::generated_traversal::NodeSlot<T, M, E, U, A>
{
    fn displaced(&self) -> Option<Node<'tree>> {
        use crate::generated_traversal::NodeSlot;
        match self {
            NodeSlot::Present(_) | NodeSlot::Missing(_) | NodeSlot::Absent(_) => None,
            NodeSlot::Error(bad) => Some(bad.node()),
            NodeSlot::Unexpected(bad) => Some(bad.node()),
        }
    }
}

impl<'tree, T, M, A> DelimiterSlot<'tree>
    for crate::generated_traversal::ReadableSlot<
        T,
        M,
        crate::generated_traversal::SourceSlice<'tree, '_>,
        crate::generated_traversal::Never,
        A,
    >
{
    fn displaced(&self) -> Option<Node<'tree>> {
        use crate::generated_traversal::{AsRawNode, ReadableSlot};
        match self {
            ReadableSlot::Present(_) | ReadableSlot::Missing(_) | ReadableSlot::Absent(_) => None,
            ReadableSlot::Error(bad) => Some(bad.raw_node()),
            ReadableSlot::Unexpected(never) => match *never {},
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod text_boundary_tests {
    use super::*;
    use crate::TreeSitterParser;
    use talkbank_model::ErrorCollector;

    // Raw-node compatibility boundary: a mismatched source must neither panic
    // on indexing nor manufacture an empty successful text value.
    #[test]
    fn text_read_rejects_out_of_source_ranges() {
        let source = include_str!(
            "../../../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E502_1.cha"
        );
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let errors = ErrorCollector::new();
        assert_eq!(
            extract_utf8_text(parsed.root_node(), source, &errors, "root").into_option(),
            Some(source),
        );
        assert!(errors.to_vec().is_empty());
        assert!(extract_utf8_text(parsed.root_node(), "", &errors, "root").is_none());
        let diagnostics = errors.into_vec();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, ErrorCode::InternalError);
        assert!(talkbank_model::CompletedDiagnostics::admit(diagnostics).is_err());
    }
}
