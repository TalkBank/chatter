//! Parsing for annotated angle-bracket groups (`< ... >[...]`), over the
//! generated typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Annotations>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{
    AsRawNode, ContentsChild0Choice, ContentsChild1Choice, ContentsChildren,
    GroupWithAnnotationsNode, KindAdmitted, NodeSlot, SlotView, SourceBound,
};
use crate::model::{BracketedContent, Group, UtteranceContent};
use talkbank_model::{ParseOutcome, Span};
use tree_sitter::Node;

use super::super::super::annotations::parse_scoped_annotations;
use super::super::marker_chain::{MarkerSeed, fold_marker_chain};
use super::super::recovery::surface_main_tier_sink;
use super::contents::{contents_of, parse_group_contents};
use crate::parser::node_span::span_of;

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
        DiagnosticLocation::new(span_of(child)),
        ErrorContext::new(source, span_of(child), ""),
        format!("Space is not allowed {position} in an angle-bracket group"),
    ));
}

/// Whitespace hugging either delimiter lands INSIDE `contents`, as its
/// first or last item; the grammar tolerates it so the parse recovers, but
/// it is invalid CHAT (CLAN CHECK 160). The typed choice at each edge says
/// whether it is whitespace; interior whitespace between words is legal and
/// never looked at. A `contents` of one whitespace item is reported once,
/// as after `<`.
fn report_edge_whitespace(
    contents: &ContentsChildren<'_, KindAdmitted>,
    source: &str,
    errors: &impl ErrorSink,
) {
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
pub(crate) fn parse_group_content<'tree>(
    typed: SourceBound<'tree, '_, GroupWithAnnotationsNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let node = typed.raw_node();
    let source = typed.source();
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_inner(typed)
            .and_then(|children| children.admit_ranges().map_err(Into::into)),
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let children = associated.children();

    let SlotView::Present(contents) = children.content_2.slot.view();
    let group_items = match contents_of(*contents, errors) {
        Some(contents) => {
            report_edge_whitespace(contents.children(), source, errors);
            parse_group_contents(&contents, errors)
        }
        None => ParseOutcome::Rejected,
    };

    // Presence proves the wrapper, not the annotation payload's validity.
    let markers = match children.annotations.slot.view() {
        SlotView::Present(annotations) => parse_scoped_annotations(*annotations, errors),
    };
    // Group contents belong to the main-tier body even when recovery places
    // their ERROR beside the contents slot. The sealed carrier retains that
    // ownership instead of sending the same word fault to a generic reporter.
    surface_main_tier_sink(children, source, errors);

    let ParseOutcome::Parsed(group_items) = group_items else {
        return ParseOutcome::Rejected;
    };
    let Ok(markers) = markers else {
        return ParseOutcome::Rejected;
    };
    if group_items.is_empty() {
        return ParseOutcome::rejected();
    }

    let span = span_of(node);
    // Required presence excludes delimiter ERROR/absence, but lexical
    // MISSING delimiters retain their real zero-width recovery positions.
    let open = match children.child_0.slot.view() {
        SlotView::Present(open) => span_of(open.raw_node()).start(),
        SlotView::Missing(open) => span_of(open.raw_node()).start(),
    };
    let close = match children.child_2.slot.view() {
        SlotView::Present(close) => span_of(close.raw_node()).end(),
        SlotView::Missing(close) => span_of(close.raw_node()).end(),
    };
    let brackets = Span::new(open, close);
    let bracketed = BracketedContent::new(group_items);
    let group = Group::new(brackets, bracketed);

    ParseOutcome::parsed(fold_marker_chain(MarkerSeed::Group(group), markers, span))
}
