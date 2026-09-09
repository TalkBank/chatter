//! Parsing for annotated angle-bracket groups (`< ... >[...]`), over the
//! generated typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Annotations>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{
    AsRawNode, ContentsChild0Choice, ContentsChild1Choice, ContentsChildren,
    GroupWithAnnotationsNode, NoChild, NodeSlot, SlotView, extract_group_with_annotations,
};
use crate::model::{BracketedContent, Group, UtteranceContent};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::super::annotations::parse_scoped_annotations;
use super::super::marker_chain::fold_marker_chain;
use super::super::report_tree_shape;
use super::contents::{contents_of, parse_group_contents};
use crate::parser::tree_parsing::parser_helpers::{expect_delimiter, surface_displaced};

/// Which group delimiter the offending whitespace touches; drives the
/// E750 message wording only.
#[derive(Clone, Copy)]
enum AngleSide {
    /// Whitespace directly after the opening `<`.
    AfterOpen,
    /// Whitespace directly before the closing `>`.
    BeforeClose,
}

/// Report E750 for a `whitespaces` CST node sitting inside the group
/// delimiters (CLAN CHECK 160). The parse continues; the diagnostic
/// alone marks the file invalid.
fn report_space_inside_angle_group(
    child: Node,
    source: &str,
    errors: &impl ErrorSink,
    side: AngleSide,
) {
    let position = match side {
        AngleSide::AfterOpen => "after '<'",
        AngleSide::BeforeClose => "before '>'",
    };
    errors.report(ParseError::new(
        ErrorCode::SpaceInsideAngleGroup,
        Severity::Error,
        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
        format!("Space is not allowed {position} in an angle-bracket group"),
    ));
}

/// Whitespace hugging either delimiter lands INSIDE `contents`, as its
/// first or last item; the grammar tolerates it so the parse recovers, but
/// it is invalid CHAT (CLAN CHECK 160). The typed choice at each edge says
/// whether it is whitespace; interior whitespace between words is legal and
/// never looked at. A `contents` of one whitespace item is reported once,
/// as after `<`.
fn report_edge_whitespace(contents: &ContentsChildren<'_>, source: &str, errors: &impl ErrorSink) {
    if let NodeSlot::Present(ContentsChild0Choice::Whitespaces(first)) = contents.child_0.slot() {
        report_space_inside_angle_group(first.raw_node(), source, errors, AngleSide::AfterOpen);
    }
    if let Some(last) = contents.child_1.slot().last()
        && let NodeSlot::Present(ContentsChild1Choice::Whitespaces(last)) = last.slot()
    {
        report_space_inside_angle_group(last.raw_node(), source, errors, AngleSide::BeforeClose);
    }
}

/// Parse one `group_with_annotations` node into group utterance content, preserving the `<...>[...]` semantics.
///
/// Grammar: `seq(less_than, field('content', contents), greater_than,
/// field('annotations', base_annotations))`. `extract_group_with_annotations`
/// places the four in typed slots; the delimiters are structure, the
/// contents go through the one shared walker, and the annotations fold onto
/// the group through `fold_marker_chain`, giving a bare `Group` or an
/// `AnnotatedGroup`. A group with no items is rejected. Until 2026-09-08
/// this walked the children by index and `node.kind()`, with a reporter per
/// position that only an ERROR at that position ever reached.
pub(crate) fn parse_group_content(
    typed: GroupWithAnnotationsNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let node = typed.raw_node();
    let children = extract_group_with_annotations(typed);

    expect_delimiter(children.child_0.slot(), |bad| {
        report_tree_shape(
            bad,
            format!(
                "Expected '<' at start of group_with_annotations, found '{}'",
                bad.kind()
            ),
            source,
            errors,
        );
    });

    let group_items = match contents_of(children.content_2.slot(), |bad| {
        report_tree_shape(
            bad,
            format!(
                "Expected 'contents' in group_with_annotations, found '{}'",
                bad.kind()
            ),
            source,
            errors,
        );
    }) {
        Some(contents) => {
            report_edge_whitespace(&contents, source, errors);
            parse_group_contents(&contents, source, errors)
        }
        None => Vec::new(),
    };

    expect_delimiter(children.child_2.slot(), |bad| {
        report_tree_shape(
            bad,
            format!(
                "Expected '>' in group_with_annotations, found '{}'",
                bad.kind()
            ),
            source,
            errors,
        );
    });

    // The annotations are required by the grammar. A MISSING placeholder
    // has no annotations in it and is the backstop's to report; an ERROR or
    // displaced node there is the group losing its shape.
    let markers = match children.annotations.slot().view() {
        SlotView::Present(annotations) => {
            parse_scoped_annotations(annotations.raw_node(), source, errors)
        }
        SlotView::Missing(_) | SlotView::Absent(NoChild) => Vec::new(),
        SlotView::Error(bad) => {
            report_tree_shape(
                bad,
                format!(
                    "Expected 'base_annotations' in group_with_annotations, found '{}'",
                    bad.kind()
                ),
                source,
                errors,
            );
            Vec::new()
        }
    };
    surface_displaced(
        &children.unexpected,
        "group_with_annotations",
        source,
        errors,
    );

    if group_items.is_empty() {
        return ParseOutcome::rejected();
    }

    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let bracketed = BracketedContent::new(group_items);
    let group = Group::new(bracketed).with_span(span);

    ParseOutcome::parsed(fold_marker_chain(
        UtteranceContent::Group(group),
        markers,
        span,
    ))
}
