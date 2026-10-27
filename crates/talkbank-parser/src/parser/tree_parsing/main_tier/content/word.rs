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
    AsRawNode, NoChild, SlotView, SourceBound, WordWithOptionalAnnotationsNode,
};
use crate::model::{ReplacedWord, Replacement, UtteranceContent};
use crate::parser::node_span::span_of;
use crate::parser::typed_cst::ExtractCanonical;
use talkbank_model::ParseOutcome;

use super::super::annotations::{parse_replacement, parse_scoped_annotations};
use super::super::word::convert_word_node;
use super::marker_chain::{MarkerSeed, fold_marker_chain};
use super::report_tree_shape;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;

/// A recovered replacement may fall back to its base word; a producer fault
/// may not. The latter travels outside this syntax enum as a Result error.
enum ReplacementSyntax {
    Absent,
    Recovered,
    Complete {
        replacement: Replacement,
        end: usize,
    },
}

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
        typed
            .extract_canonical()
            .and_then(|children| children.admit_ranges().map_err(Into::into)),
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let children = associated.children();

    let word = match children.word.slot.view() {
        SlotView::Present(word) => {
            convert_word_node(*word, errors).map(|lowered| lowered.into_word())
        }
        // The word position is required; `Absent` means a well-formed node of
        // another kind stood where the word should be, which no recovery
        // node marks and the whole-tree pass cannot see, so the shape fault
        // is reported here, as the positional check reported it.
        SlotView::Absent(NoChild) => {
            report_tree_shape(
                node,
                "Expected 'standalone_word' at the start of word_with_optional_annotations"
                    .to_string(),
                source,
                errors,
            );
            Ok(ParseOutcome::rejected())
        }
        SlotView::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                source,
                "word_with_optional_annotations",
            ));
            Ok(ParseOutcome::rejected())
        }
    };

    let replacement = match children.child_1.slot.as_ref() {
        Some(group) => match group.view() {
            SlotView::Present(group) => match group.replacement.slot.view() {
                SlotView::Present(replacement) => {
                    // The replacement's own end places the replaced word:
                    // the construct is the word through this `]`, and any
                    // marker after it belongs to the wrapper the fold builds.
                    let end = span_of(replacement.raw_node()).end();
                    parse_replacement(*replacement, errors).map(|outcome| match outcome {
                        ParseOutcome::Parsed(replacement) => {
                            ReplacementSyntax::Complete { replacement, end }
                        }
                        ParseOutcome::Rejected => ReplacementSyntax::Recovered,
                    })
                }
                SlotView::Error(bad) => {
                    errors.report(unexpected_node_error(
                        bad.raw_node(),
                        source,
                        "word_with_optional_annotations",
                    ));
                    Ok(ReplacementSyntax::Recovered)
                }
                SlotView::Absent(NoChild) => Ok(ReplacementSyntax::Recovered),
            },
            // An ERROR where the group should be is classified in context, as
            // the old walk's catch-all classified it (a bare `[` is an
            // incomplete annotation, not generic unparsable content).
            SlotView::Missing(never) => match never {},
            SlotView::Error(bad) => {
                errors.report(unexpected_node_error(
                    bad.raw_node(),
                    source,
                    "word_with_optional_annotations",
                ));
                Ok(ReplacementSyntax::Recovered)
            }
            SlotView::Absent(never) => match never {},
        },
        None => Ok(ReplacementSyntax::Absent),
    };

    // The markers written after the word, already resolved around the retrace
    // marker. ONE value rather than an `annotations` list beside a
    // `retrace_kind`, because those two were a partition of one ordered
    // sequence and could not say which side of the marker an annotation sat on.
    let markers = match children.annotations.slot.as_ref() {
        Some(slot) => match slot.view() {
            SlotView::Present(annotations) => parse_scoped_annotations(*annotations, errors),
            SlotView::Error(bad) => {
                errors.report(unexpected_node_error(
                    bad.raw_node(),
                    bad.source(),
                    "word_with_optional_annotations",
                ));
                Ok(Vec::new())
            }
            SlotView::Absent(never) => match never {},
        },
        None => Ok(Vec::new()),
    };
    surface_displaced(
        &children.unexpected,
        "word_with_optional_annotations",
        source,
        errors,
    );

    let Ok(ParseOutcome::Parsed(w)) = word else {
        return ParseOutcome::rejected();
    };
    let (Ok(replacement), Ok(markers)) = (replacement, markers) else {
        return ParseOutcome::rejected();
    };
    // The node is the whole chain, word through its final `]`; the fold
    // reads only its start, and each value it builds ends at its own last
    // part. A replaced word ends at the replacement's `]` (through its last
    // scoped annotation once the fold attaches one), as the re2c backend
    // places it.
    let whole = span_of(node);
    let core = match replacement {
        ReplacementSyntax::Complete { replacement, end } => {
            MarkerSeed::ReplacedWord(Box::new(ReplacedWord::new(
                talkbank_model::Span::new(whole.start(), end),
                w,
                replacement,
            )))
        }
        ReplacementSyntax::Absent | ReplacementSyntax::Recovered => MarkerSeed::Word(Box::new(w)),
    };
    ParseOutcome::parsed(fold_marker_chain(core, markers, whole))
}
