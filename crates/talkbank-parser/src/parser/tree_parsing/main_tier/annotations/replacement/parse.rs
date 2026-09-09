//! Parsing for replacement annotations (`[: ... ]`), over the generated
//! typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, ChildSlot, FromNodeKind, NoChild, ReplacementNode, SeqSlot, SlotView,
    StandaloneWordNode, extract_replacement,
};
use crate::model::{Replacement, Word};
use crate::parser::ChildCapacity;
use crate::parser::tree_parsing::main_tier::content::report_tree_shape;
use crate::parser::tree_parsing::main_tier::word::convert_word_node;
use crate::parser::tree_parsing::parser_helpers::{expect_delimiter, surface_displaced};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse a replacement annotation node into `Replacement`.
///
/// Grammar: `seq(left_bracket, colon, repeat1(seq(optional(whitespaces),
/// standalone_word)), right_bracket)`. `extract_replacement` places the
/// four in typed slots, the word sequence as a first element and a repeat;
/// each word goes through the word converter. The sequence elements are
/// fixed two-position shapes whose own sinks the generator leaves empty by
/// construction, so only the node's sink is surfaced. Until 2026-09-09 this walked the children by index and
/// `kind()` string with a catch-all for anything unnamed.
///
/// The diagnostics keep their code and words: a delimiter that lost its
/// shape, a MISSING word (a placeholder tree-sitter inserted) and a
/// zero-width word are each `ReplacementParseError` as before; a MISSING
/// delimiter is the whole-tree pass's, as the old walk left it. A
/// replacement with no word is rejected.
pub(crate) fn parse_replacement(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Replacement> {
    let Some(typed) = ReplacementNode::from_node(node) else {
        report_tree_shape(
            node,
            format!("Expected a replacement node, found '{}'", node.kind()),
            source,
            errors,
        );
        return ParseOutcome::rejected();
    };
    let children = extract_replacement(typed);
    expect_delimiter(children.child_0.slot(), |bad| {
        report(
            bad,
            source,
            errors,
            format!(
                "Expected 'left_bracket' at position 0 of replacement, found '{}'",
                bad.kind()
            ),
        );
    });
    expect_delimiter(children.child_1.slot(), |bad| {
        report(
            bad,
            source,
            errors,
            format!(
                "Expected 'colon' at position 1 of replacement, found '{}'",
                bad.kind()
            ),
        );
    });

    let repeat = children.child_3.slot();
    // A capacity hint: at most one word per child of the node.
    let mut words = ChildCapacity::for_node(node).into_vec();
    if let Some(first) = sequence(children.child_2.slot()) {
        push_word(first.child_1.slot(), 0, &mut words, source, errors);
    }
    for (index, element) in repeat.iter().enumerate() {
        if let Some(next) = sequence(element.slot()) {
            push_word(next.child_1.slot(), index + 1, &mut words, source, errors);
        }
    }

    expect_delimiter(children.child_4.slot(), |bad| {
        report(
            bad,
            source,
            errors,
            format!("Expected ']' at end of replacement, found '{}'", bad.kind()),
        );
    });
    surface_displaced(&children.unexpected, "replacement", source, errors);

    if words.is_empty() {
        ParseOutcome::rejected()
    } else {
        ParseOutcome::parsed(Replacement::new(words))
    }
}

/// The element a word sequence position holds: `None` for a sequence that
/// did not match (`Absent`) or lost its shape to an ERROR, which the
/// whole-tree pass names. A sequence is never MISSING or displaced, and
/// the slot's type says so.
fn sequence<'a, 'tree, T>(slot: &'a SeqSlot<'tree, T>) -> Option<&'a T> {
    match slot.view() {
        SlotView::Present(element) => Some(element),
        SlotView::Error(_) | SlotView::Absent(NoChild) => None,
    }
}

/// Convert the word at a sequence element's word position, or report why
/// there is none: a MISSING placeholder and a zero-width word are the two
/// recovery shapes the old walk named, and they keep their words; `position`
/// counts words in the run, as the old message did.
fn push_word(
    slot: &ChildSlot<'_, StandaloneWordNode<'_>>,
    position: usize,
    words: &mut Vec<Word>,
    source: &str,
    errors: &impl ErrorSink,
) {
    match slot.view() {
        SlotView::Present(word) => {
            let raw = word.raw_node();
            if raw.start_byte() == raw.end_byte() {
                report(raw, source, errors, "Replacement text empty".to_string());
                return;
            }
            if let ParseOutcome::Parsed(word) = convert_word_node(raw, source, errors) {
                words.push(word);
            }
        }
        SlotView::Missing(missing) => report(
            missing,
            source,
            errors,
            format!(
                "Missing word in replacement at position {position} (tree-sitter inserted placeholder)"
            ),
        ),
        // An ERROR at the word position is the whole-tree pass's to name.
        SlotView::Error(_) | SlotView::Absent(NoChild) => {}
    }
}

/// A `ReplacementParseError` at `node`.
fn report(node: Node, source: &str, errors: &impl ErrorSink, message: String) {
    errors.report(ParseError::new(
        ErrorCode::ReplacementParseError,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
        message,
    ));
}
