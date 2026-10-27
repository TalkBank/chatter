//! Parsing for replacement annotations (`[: ... ]`), over the generated
//! typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{
    AsRawNode, NoChild, NonMissingKindSlot, Ranged, ReplacementNode, SlotView, SourceBound,
    StandaloneWordNode,
};
use crate::model::{Replacement, Word};
use crate::parser::ChildCapacity;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::main_tier::word::convert_word_node;
use crate::parser::tree_parsing::parser_helpers::{expect_delimiter, surface_displaced};
use crate::parser::typed_cst::{ExtractCanonical, ReportedCstFailure};
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
/// Compiled grammar admission proves that composite word slots cannot be
/// Missing. Delimiter recovery and zero-width word refusal retain their existing
/// diagnostics; a replacement with no admitted word remains rejected.
pub(in crate::parser) fn parse_replacement<'tree>(
    typed: SourceBound<'tree, '_, ReplacementNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Replacement>, ReportedCstFailure> {
    let source = typed.source();
    let node = typed.node().raw_node();
    let extraction = typed
        .extract_canonical()
        .and_then(|children| children.admit_ranges().map_err(Into::into));
    let associated = crate::parser::typed_cst::report_admission(extraction, node, source, errors)?;
    let children = associated.children();
    expect_delimiter(&children.child_0.slot, |bad| {
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
    expect_delimiter(&children.child_1.slot, |bad| {
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

    // A capacity hint: at most one word per child of the node.
    let mut words = ChildCapacity::for_node(node).into_vec();
    let mut result = Ok(());
    if let SlotView::Present(first) = children.child_2.slot.view() {
        result = result.and(push_word(&first.child_1.slot, &mut words, errors));
    }
    for element in &children.child_3.slot {
        if let SlotView::Present(next) = element.slot.view() {
            result = result.and(push_word(&next.child_1.slot, &mut words, errors));
        }
    }

    expect_delimiter(&children.child_4.slot, |bad| {
        report(
            bad,
            source,
            errors,
            format!("Expected ']' at end of replacement, found '{}'", bad.kind()),
        );
    });
    surface_displaced(&children.unexpected, "replacement", source, errors);

    result?;
    Ok(
        match talkbank_model::model::annotation::ReplacementWords::new(words) {
            Ok(words) => ParseOutcome::parsed(Replacement::new(words)),
            Err(_) => ParseOutcome::rejected(),
        },
    )
}

/// Convert an admitted composite word slot, preserving zero-width refusal,
/// source-read failures and the existing Error/Absent recovery policy.
fn push_word<'tree, 'source>(
    slot: &NonMissingKindSlot<
        'tree,
        SourceBound<'tree, 'source, StandaloneWordNode<'tree>>,
        Ranged<'source>,
    >,
    words: &mut Vec<Word>,
    errors: &impl ErrorSink,
) -> Result<(), ReportedCstFailure> {
    match slot.view() {
        SlotView::Present(word) => {
            let source = word.source();
            let raw = word.raw_node();
            if raw.start_byte() == raw.end_byte() {
                report(raw, source, errors, "Replacement text empty".to_string());
                return Ok(());
            }
            if let ParseOutcome::Parsed(word) =
                convert_word_node(*word, errors)?.into_replacement_word(errors)
            {
                words.push(word);
            }
        }
        // An ERROR at the word position is the whole-tree pass's to name.
        SlotView::Error(_) | SlotView::Absent(NoChild) => {}
    }
    Ok(())
}

/// A `ReplacementParseError` at `node`.
fn report(node: Node, source: &str, errors: &impl ErrorSink, message: String) {
    errors.report(ParseError::new(
        ErrorCode::ReplacementParseError,
        Severity::Error,
        DiagnosticLocation::new(span_of(node)),
        ErrorContext::new(source, span_of(node), ""),
        message,
    ));
}
