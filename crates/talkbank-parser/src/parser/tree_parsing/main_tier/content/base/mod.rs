//! Parsing for base (non-group) main-tier content items.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
#![deny(clippy::wildcard_enum_match_arm)]

mod internal_bullet;
mod labeled_marker;
mod long_feature;
mod nonvocal;
mod other_spoken;
mod overlap_point;

// Re-export overlap_point parser for use in other modules
pub(crate) use overlap_point::{parse_overlap_point, parse_overlap_point_token};

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, BaseContentItemChoiceBoundView, BaseContentItemMissingSourceView,
    BaseContentItemNode, NoChild, SourceBound, SourceSlotView,
};
use crate::model::UtteranceContent;
use talkbank_model::ParseOutcome;

use super::super::super::freecode::parse_freecode;
use super::nonword::parse_nonword_content;
use super::word::parse_word_content;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, parse_pause_node, surface_displaced,
};
use crate::parser::typed_cst::ExtractCanonical;

/// Parse one `base_content_item` into `UtteranceContent`.
///
/// Grammar: `choice(word_with_optional_annotations, pause_token,
/// nonword_with_optional_annotations, freecode, bullet, underline_begin,
/// underline_end, long_feature, nonvocal, other_spoken_event)`.
/// `extract_base_content_item` places the one alternative in a typed
/// choice slot, and the match below is exhaustive over the GENERATED
/// `BaseContentItemChoice`, so an alternative the grammar gains is a
/// compile error here rather than a runtime E340. Until 2026-09-09 this
/// classified `child(0).kind()` through a hand-written mirror of that enum,
/// whose own doc called it interim: the mirror could not notice the grammar
/// gaining an alternative, and its two tests pinned the mirror against
/// itself.
///
/// The recovery states: a MISSING alternative is reported here as the
/// legacy positional check did (E342); an ERROR alternative is the
/// whole-tree pass's, which names it with its classified code (the mirror
/// used to call it "Unknown base content type 'ERROR'", E340, which is a
/// grammar/parser mismatch it was not); a DISPLACED node, one the generated
/// classifier could not place in this position, is the one thing E340 is
/// for, and the only way it is reached. Whatever else filled no position is
/// surfaced through `surface_displaced`.
pub(crate) fn parse_base_content<'tree>(
    typed: SourceBound<'tree, '_, BaseContentItemNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let source = typed.source();
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        typed.extract_canonical(),
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let children = associated.children();
    let content = match associated.field_content().slot().view() {
        SourceSlotView::Present(choice) => {
            let Some(choice) = crate::parser::typed_cst::read_source_field(choice, errors) else {
                surface_displaced(&children.unexpected, "base_content", source, errors);
                return ParseOutcome::rejected();
            };
            match choice.view() {
                BaseContentItemChoiceBoundView::WordWithOptionalAnnotations(node) => {
                    parse_word_content(node, errors)
                }
                BaseContentItemChoiceBoundView::PauseToken(node) => {
                    parse_pause_node(node.node(), source, errors).map(UtteranceContent::Pause)
                }
                BaseContentItemChoiceBoundView::NonwordWithOptionalAnnotations(node) => {
                    parse_nonword_content(node, errors)
                }
                BaseContentItemChoiceBoundView::Freecode(node) => parse_freecode(node, errors),
                BaseContentItemChoiceBoundView::Bullet(node) => {
                    internal_bullet::parse_internal_bullet(node, errors)
                }
                BaseContentItemChoiceBoundView::UnderlineBegin(node) => {
                    // Underline begin marker (U+0002 U+0001)
                    ParseOutcome::parsed(UtteranceContent::UnderlineBegin(
                        talkbank_model::UnderlineMarker::new(span_of(node.node().raw_node())),
                    ))
                }
                BaseContentItemChoiceBoundView::UnderlineEnd(node) => {
                    // Underline end marker (U+0002 U+0002)
                    ParseOutcome::parsed(UtteranceContent::UnderlineEnd(
                        talkbank_model::UnderlineMarker::new(span_of(node.node().raw_node())),
                    ))
                }
                BaseContentItemChoiceBoundView::LongFeature(node) => {
                    long_feature::parse_long_feature(node, errors)
                }
                BaseContentItemChoiceBoundView::Nonvocal(node) => {
                    nonvocal::parse_nonvocal(node, errors)
                }
                BaseContentItemChoiceBoundView::OtherSpokenEvent(node) => {
                    other_spoken::parse_other_spoken_event(node, errors)
                }
            }
        }
        SourceSlotView::Missing(missing) => {
            // Kind admission excludes composite placeholders. The four
            // lexical alternatives retain their existing Missing diagnostic.
            let node = match missing {
                BaseContentItemMissingSourceView::Bullet(never) => match never.0 {},
                BaseContentItemMissingSourceView::LongFeature(never) => match never.0 {},
                BaseContentItemMissingSourceView::Nonvocal(never) => match never.0 {},
                BaseContentItemMissingSourceView::NonwordWithOptionalAnnotations(never) => {
                    match never.0 {}
                }
                BaseContentItemMissingSourceView::OtherSpokenEvent(never) => match never.0 {},
                BaseContentItemMissingSourceView::WordWithOptionalAnnotations(never) => {
                    match never.0 {}
                }
                BaseContentItemMissingSourceView::Freecode(node) => node.raw_node(),
                BaseContentItemMissingSourceView::PauseToken(node) => node.raw_node(),
                BaseContentItemMissingSourceView::UnderlineBegin(node) => node.raw_node(),
                BaseContentItemMissingSourceView::UnderlineEnd(node) => node.raw_node(),
            };
            check_not_missing(node, source, errors, "base_content");
            ParseOutcome::rejected()
        }
        SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => ParseOutcome::rejected(),
        SourceSlotView::Unexpected(never) => match never {},
    };
    surface_displaced(&children.unexpected, "base_content", source, errors);
    content
}

/// The span of a leaf or marker node, for the sibling content modules.
pub(super) use crate::parser::node_span::span_of;
