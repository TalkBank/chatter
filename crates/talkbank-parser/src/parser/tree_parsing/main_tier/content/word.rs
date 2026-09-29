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
    AsRawNode, NoChild, SourceBound, SourceSlotView, WordWithOptionalAnnotationsNode,
};
use crate::model::{ReplacedWord, UtteranceContent};
use talkbank_model::ParseOutcome;
use talkbank_model::Span;

use super::super::annotations::{parse_replacement, parse_scoped_annotations};
use super::super::word::convert_word_node;
use super::marker_chain::fold_marker_chain;
use super::report_tree_shape;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{SlotState, expect_present, surface_displaced};
use crate::parser::typed_cst::read_source_field;

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
/// Compiled grammar admission excludes Missing for the composite word,
/// replacement and annotation slots, not lexical recovery inside them. Error,
/// required absence and source-read refusal remain independently handled; an
/// unusable word never becomes a fabricated model value.
pub(crate) fn parse_word_content<'tree>(
    typed: SourceBound<'tree, '_, WordWithOptionalAnnotationsNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let source = typed.source();
    let node = typed.node().raw_node();
    let Ok(associated) = crate::parser::typed_cst::report_reconstruction(
        crate::parser::typed_cst::canonical_grammar()
            .and_then(|grammar| typed.extract_admitted(grammar)),
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let children = associated.children();

    let word = match associated.field_word().slot().view() {
        SourceSlotView::Present(word) => match read_source_field(word, errors) {
            Some(word) => convert_word_node(word, errors),
            None => ParseOutcome::rejected(),
        },
        // The word position is required; `Absent` means a well-formed node of
        // another kind stood where the word should be, which no recovery
        // node marks and the whole-tree pass cannot see, so the shape fault
        // is reported here, as the positional check reported it.
        SourceSlotView::Absent(NoChild) => {
            report_tree_shape(
                node,
                "Expected 'standalone_word' at the start of word_with_optional_annotations"
                    .to_string(),
                source,
                errors,
            );
            ParseOutcome::rejected()
        }
        SourceSlotView::Missing(never) => match never {},
        SourceSlotView::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                source,
                "word_with_optional_annotations",
            ));
            ParseOutcome::rejected()
        }
    };

    let replacement = match associated.field_child_1().slot().optional() {
        Some(group) => match group.view() {
            SourceSlotView::Present(group) => match group.field_replacement().slot().view() {
                SourceSlotView::Present(replacement) => {
                    match read_source_field(replacement, errors) {
                        Some(replacement) => parse_replacement(replacement, errors),
                        None => ParseOutcome::rejected(),
                    }
                }
                SourceSlotView::Missing(never) => match never {},
                SourceSlotView::Error(bad) => {
                    errors.report(unexpected_node_error(
                        bad.raw_node(),
                        source,
                        "word_with_optional_annotations",
                    ));
                    ParseOutcome::rejected()
                }
                SourceSlotView::Absent(NoChild) => ParseOutcome::rejected(),
            },
            // An ERROR where the group should be is classified in context, as
            // the old walk's catch-all classified it (a bare `[` is an
            // incomplete annotation, not generic unparsable content).
            SourceSlotView::Error(bad) => {
                errors.report(unexpected_node_error(
                    bad.raw_node(),
                    source,
                    "word_with_optional_annotations",
                ));
                ParseOutcome::rejected()
            }
            SourceSlotView::Absent(never) => match never {},
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
                    match crate::parser::typed_cst::report_reconstruction(
                        parse_scoped_annotations(*annotations, source, errors),
                        annotations.raw_node(),
                        source,
                        errors,
                    ) {
                        Ok(markers) => markers,
                        Err(_) => return ParseOutcome::Rejected,
                    }
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
            UtteranceContent::ReplacedWord(Box::new(ReplacedWord::new(w, repl).with_span(whole)))
        }
        ParseOutcome::Rejected => UtteranceContent::Word(Box::new(w)),
    };
    ParseOutcome::parsed(fold_marker_chain(core, markers, whole))
}
