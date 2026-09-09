//! Parsing for `word_with_optional_annotations` content items.
//!
//! Combines the base word token with optional replacement and scoped
//! annotations into one `UtteranceContent` variant.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, FromNodeKind, NoChild, SlotView, WordWithOptionalAnnotationsNode,
    extract_word_with_optional_annotations,
};
use crate::model::{ReplacedWord, UtteranceContent};
use talkbank_model::ParseOutcome;
use talkbank_model::Span;
use tree_sitter::Node;

use super::super::annotations::{parse_replacement, parse_scoped_annotations};
use super::super::word::convert_word_node;
use super::marker_chain::fold_marker_chain;
use super::report_tree_shape;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{SlotState, expect_present, surface_displaced};

/// Parse a `word_with_optional_annotations` node into `UtteranceContent`,
/// over the generated typed traversal.
///
/// Grammar: `seq(field('word', standalone_word), optional(seq(whitespaces,
/// replacement)), field('annotations', optional(base_annotations)))`. The
/// word goes through the word converter, the replacement through its own
/// parser, and the markers fold onto the word through `fold_marker_chain`,
/// giving a `Word`, a `ReplacedWord`, or the annotated spelling when a
/// marker actually arrives. Until 2026-09-09 this walked the children by
/// index and `kind()` string with a catch-all for anything unnamed.
///
/// A MISSING word is reported at its position (E342, as the positional
/// check did) and builds nothing, so a placeholder never becomes a word;
/// the replacement group is a sequence, never MISSING or displaced, and
/// one that lost its shape to an ERROR is classified in context as the
/// old catch-all classified it; a MISSING replacement or annotations node
/// is reported (E342) where the old walk fed it to the sub-parser
/// unguarded.
pub(crate) fn parse_word_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let Some(typed) = WordWithOptionalAnnotationsNode::from_node(node) else {
        report_tree_shape(
            node,
            format!(
                "Expected a word_with_optional_annotations node, found '{}'",
                node.kind()
            ),
            source,
            errors,
        );
        return ParseOutcome::rejected();
    };
    let children = extract_word_with_optional_annotations(typed);

    let word = match expect_present(
        children.word.slot(),
        "word_with_optional_annotations",
        source,
        errors,
    ) {
        SlotState::Present(word) => convert_word_node(word.raw_node(), source, errors),
        // The word position is required; `Absent` means a well-formed node of
        // another kind stood where the word should be, which no recovery
        // node marks and the whole-tree pass cannot see, so the shape fault
        // is reported here, as the positional check reported it.
        SlotState::Absent => {
            report_tree_shape(
                node,
                "Expected 'standalone_word' at the start of word_with_optional_annotations"
                    .to_string(),
                source,
                errors,
            );
            ParseOutcome::rejected()
        }
        SlotState::Recovered => ParseOutcome::rejected(),
    };

    let replacement = match children.child_1.slot() {
        Some(group) => match group.view() {
            SlotView::Present(group) => match expect_present(
                group.replacement.slot(),
                "word_with_optional_annotations",
                source,
                errors,
            ) {
                SlotState::Present(replacement) => {
                    parse_replacement(replacement.raw_node(), source, errors)
                }
                SlotState::Absent | SlotState::Recovered => ParseOutcome::rejected(),
            },
            // An ERROR where the group should be is classified in context, as
            // the old walk's catch-all classified it (a bare `[` is an
            // incomplete annotation, not generic unparsable content).
            SlotView::Error(bad) => {
                errors.report(unexpected_node_error(
                    bad,
                    source,
                    "word_with_optional_annotations",
                ));
                ParseOutcome::rejected()
            }
            SlotView::Absent(NoChild) => ParseOutcome::rejected(),
        },
        None => ParseOutcome::rejected(),
    };

    // The markers written after the word, already resolved around the retrace
    // marker. ONE value rather than an `annotations` list beside a
    // `retrace_kind`, because those two were a partition of one ordered
    // sequence and could not say which side of the marker an annotation sat on.
    let markers = match children.annotations.slot() {
        Some(slot) => {
            match expect_present(slot, "word_with_optional_annotations", source, errors) {
                SlotState::Present(annotations) => {
                    parse_scoped_annotations(annotations.raw_node(), source, errors)
                }
                SlotState::Absent | SlotState::Recovered => Vec::new(),
            }
        }
        None => Vec::new(),
    };
    surface_displaced(
        &children.unexpected,
        "word_with_optional_annotations",
        source,
        errors,
    );

    let ParseOutcome::Parsed(w) = word else {
        return ParseOutcome::rejected();
    };
    // The whole construct, word through final `]`. E757's glue detection relies
    // on a wrapper's span ending at the last bracket, so the fold uses it too.
    let whole = Span::new(w.span.start, node.end_byte() as u32);
    let core = match replacement {
        ParseOutcome::Parsed(repl) => {
            UtteranceContent::ReplacedWord(Box::new(ReplacedWord::new(w, repl)))
        }
        ParseOutcome::Rejected => UtteranceContent::Word(Box::new(w)),
    };
    ParseOutcome::parsed(fold_marker_chain(core, markers, whole))
}
