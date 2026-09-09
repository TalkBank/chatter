//! Parsing for base (non-group) main-tier content items.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
#![deny(clippy::wildcard_enum_match_arm)]

mod internal_bullet;
mod long_feature;
mod nonvocal;
mod other_spoken;
mod overlap_point;

// Re-export overlap_point parser for use in other modules
pub(crate) use overlap_point::{parse_overlap_point, parse_overlap_point_token};

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, BaseContentItemChoice, BaseContentItemNode, ChildSlot, LongFeatureLabelNode,
    NoChild, NodeSlot, RecoveryNode, extract_base_content_item,
};
use crate::model::UtteranceContent;
use talkbank_model::ParseOutcome;

use super::super::super::freecode::parse_freecode;
use super::nonword::parse_nonword_content;
use super::report_tree_shape;
use super::word::parse_word_content;
use crate::parser::tree_parsing::parser_helpers::{
    SlotState, check_not_missing, expect_delimiter, expect_present, extract_utf8_text,
    parse_pause_node, surface_displaced,
};

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
pub(crate) fn parse_base_content(
    typed: BaseContentItemNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let children = extract_base_content_item(typed);
    let content = match children.content.slot() {
        NodeSlot::Present(choice) => match choice {
            BaseContentItemChoice::WordWithOptionalAnnotations(node) => {
                parse_word_content(node.raw_node(), source, errors)
            }
            BaseContentItemChoice::PauseToken(node) => {
                parse_pause_node(node.raw_node(), source, errors).map(UtteranceContent::Pause)
            }
            BaseContentItemChoice::NonwordWithOptionalAnnotations(node) => {
                parse_nonword_content(node.raw_node(), source, errors)
            }
            BaseContentItemChoice::Freecode(node) => {
                parse_freecode(node.raw_node(), source, errors)
            }
            BaseContentItemChoice::Bullet(node) => {
                internal_bullet::parse_internal_bullet(node.raw_node(), source, errors)
            }
            BaseContentItemChoice::UnderlineBegin(node) => {
                // Underline begin marker (U+0002 U+0001)
                ParseOutcome::parsed(UtteranceContent::UnderlineBegin(
                    talkbank_model::UnderlineMarker::from_span(span_of(node.raw_node())),
                ))
            }
            BaseContentItemChoice::UnderlineEnd(node) => {
                // Underline end marker (U+0002 U+0002)
                ParseOutcome::parsed(UtteranceContent::UnderlineEnd(
                    talkbank_model::UnderlineMarker::from_span(span_of(node.raw_node())),
                ))
            }
            BaseContentItemChoice::LongFeature(node) => {
                long_feature::parse_long_feature(node.raw_node(), source, errors)
            }
            BaseContentItemChoice::Nonvocal(node) => {
                nonvocal::parse_nonvocal(node.raw_node(), source, errors)
            }
            BaseContentItemChoice::OtherSpokenEvent(node) => {
                other_spoken::parse_other_spoken_event(node.raw_node(), source, errors)
            }
        },
        NodeSlot::Missing(missing) => {
            check_not_missing(*missing, source, errors, "base_content");
            ParseOutcome::rejected()
        }
        NodeSlot::Error(_) | NodeSlot::Absent(NoChild) => ParseOutcome::rejected(),
        NodeSlot::Unexpected(bad) => {
            // The grammar produced an alternative this parser does not know:
            // a grammar/parser mismatch, not a fault in the CHAT input.
            errors.report(
                ParseError::new(
                    ErrorCode::UnknownBaseContent,
                    Severity::Error,
                    SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
                    ErrorContext::new(source, bad.start_byte()..bad.end_byte(), ""),
                    format!("Unknown base content type '{}'", bad.kind()),
                )
                .with_suggestion("This may be a new grammar feature not yet supported"),
            );
            ParseOutcome::rejected()
        }
    };
    surface_displaced(&children.unexpected, "base_content", source, errors);
    content
}

/// A delimiter position of a marker construct (`&`, `{n=`, `}` and their
/// kin): `true` when it holds what the grammar put there or a MISSING
/// placeholder, which is the whole-tree pass's to report; `false` after an
/// ERROR (or, for a slot that can hold one, a displaced node) has been
/// reported as the marker losing its shape at `within`. A marker is built
/// only when every one of its delimiters answers `true`, as the positional
/// checks these replaced rejected on any structural defect.
pub(super) fn delimiter<'tree, T, M, U: RecoveryNode<'tree>, A>(
    slot: &NodeSlot<'tree, T, M, U, A>,
    what: &str,
    within: &str,
    source: &str,
    errors: &impl ErrorSink,
) -> bool {
    let mut intact = true;
    expect_delimiter(slot, |bad| {
        intact = false;
        report_tree_shape(
            bad,
            format!("Expected {what} in {within}, found '{}'", bad.kind()),
            source,
            errors,
        );
    });
    intact
}

/// The label a marker's `long_feature_label` position carries, or `None`
/// after its recovery state has been reported at `within`; `context` names
/// the position for the decode diagnostic.
pub(super) fn marker_label(
    slot: &ChildSlot<'_, LongFeatureLabelNode<'_>>,
    within: &str,
    context: &str,
    source: &str,
    errors: &impl ErrorSink,
) -> Option<String> {
    match expect_present(slot, within, source, errors) {
        SlotState::Present(node) => {
            Some(extract_utf8_text(node.raw_node(), source, errors, context, "").to_string())
        }
        SlotState::Absent | SlotState::Recovered => None,
    }
}

/// The span of a leaf or marker node.
pub(super) fn span_of(node: tree_sitter::Node) -> talkbank_model::Span {
    talkbank_model::Span::new(node.start_byte() as u32, node.end_byte() as u32)
}
