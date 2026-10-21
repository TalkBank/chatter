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
use crate::parser::node_span::span_of;
use crate::parser::typed_cst::ExtractCanonical;
use talkbank_model::ParseOutcome;

use super::super::annotations::{parse_replacement, parse_scoped_annotations};
use super::super::word::convert_word_node;
use super::marker_chain::fold_marker_chain;
use super::report_tree_shape;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
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
/// Compiled admission excludes Missing at composite word, replacement and annotation slots. Error,
/// required absence and source-read refusal remain independently handled; an
/// unusable word never becomes a fabricated model value.
pub(crate) fn parse_word_content<'tree>(
    typed: SourceBound<'tree, '_, WordWithOptionalAnnotationsNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let source = typed.source();
    let node = typed.node().raw_node();
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        typed.extract_canonical(),
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
                    // The replacement's own end places the replaced word:
                    // the construct is the word through this `]`, and any
                    // marker after it belongs to the wrapper the fold builds.
                    let end = span_of(replacement.raw_node()).end();
                    match read_source_field(replacement, errors) {
                        Some(replacement) => {
                            parse_replacement(replacement, errors).map(|repl| (repl, end))
                        }
                        None => ParseOutcome::rejected(),
                    }
                }
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
            SourceSlotView::Missing(never) => match never {},
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
    let markers = match associated.field_annotations().slot().optional() {
        Some(slot) => match slot.view() {
            SourceSlotView::Present(annotations) => match read_source_field(annotations, errors) {
                Some(annotations) => parse_scoped_annotations(annotations, errors),
                None => Vec::new(),
            },
            SourceSlotView::Error(bad) => {
                errors.report(unexpected_node_error(
                    bad.raw_node(),
                    bad.source(),
                    "word_with_optional_annotations",
                ));
                Vec::new()
            }
            SourceSlotView::Absent(never) => match never {},
        },
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
    // The node is the whole chain, word through its final `]`; the fold
    // reads only its start, and each value it builds ends at its own last
    // part. A replaced word ends at the replacement's `]` (through its last
    // scoped annotation once the fold attaches one), as the re2c backend
    // places it.
    let whole = span_of(node);
    let core = match replacement {
        ParseOutcome::Parsed((repl, replacement_end)) => {
            UtteranceContent::ReplacedWord(Box::new(ReplacedWord::new(
                talkbank_model::Span::new(whole.start(), replacement_end),
                w,
                repl,
            )))
        }
        ParseOutcome::Rejected => UtteranceContent::Word(Box::new(w)),
    };
    ParseOutcome::parsed(fold_marker_chain(core, markers, whole))
}
