//! Parsing for annotated angle-bracket groups (`< ... >[...]`).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Annotations>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{Annotated, BracketedContent, Group, Retrace, UtteranceContent};
use crate::node_types::{BASE_ANNOTATIONS, CONTENTS, GREATER_THAN, LESS_THAN, WHITESPACES};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::super::annotations::parse_scoped_annotations;
use super::contents::parse_group_contents;

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

/// Parse one `group_with_annotations` node into group utterance content, preserving the `<...>[...]` semantics.
///
/// CHAT group annotations appear as `< contents > base_annotations` and are described in the Group and Annotation
/// sections of the manual. This parser consumes the expected `<`, optional whitespace, contents block,
/// optional trailing whitespace, closing `>`, and the required annotations block, emitting either a
/// bare `Group` or an `AnnotatedGroup` depending on whether scoped annotations exist. Any deviation from that
/// structure is reported through `ParseError` so users can correlate the diagnostic with the manual’s grammar.
pub(crate) fn parse_group_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let child_count = node.child_count();
    // Pre-allocate: group typically has 1-5 items, annotations typically 0-2
    let mut group_items = Vec::with_capacity(4);
    let mut annotations = Vec::with_capacity(2);
    let mut retrace_kind = None;
    let mut idx = 0;

    // Grammar: group_with_annotations: $ => seq(
    //   $.less_than,
    //   optional($.whitespaces),  // Allow leading whitespace after <
    //   $.contents,
    //   optional($.whitespaces),  // Allow trailing whitespace before >
    //   $.greater_than,
    //   $.base_annotations  // REQUIRED
    // )

    // Position 0: '<' (required)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == LESS_THAN {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected '<' at start of group_with_annotations, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Position 1: optional whitespaces after <. The grammar tolerates
    // it so the parse recovers, but it is invalid CHAT (CLAN CHECK
    // 160): report E750 instead of silently dropping the space.
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() == WHITESPACES
    {
        report_space_inside_angle_group(child, source, errors, AngleSide::AfterOpen);
        idx += 1;
    }

    // Next: contents (required)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == CONTENTS {
            // In the real CST the delimiter-hugging whitespace lands
            // INSIDE `contents` as its first/last child (empirically
            // verified on the CHECK-160 fixture), not as a sibling
            // between `less_than` and `contents` as the grammar sketch
            // above suggests; check both shapes. Only the edge
            // positions violate CHECK 160; interior whitespace between
            // words is legal.
            let contents_children = child.child_count();
            if contents_children > 0 {
                if let Some(first) = child.child(0)
                    && first.kind() == WHITESPACES
                {
                    report_space_inside_angle_group(first, source, errors, AngleSide::AfterOpen);
                }
                if contents_children > 1
                    && let Some(last) = child.child((contents_children - 1) as u32)
                    && last.kind() == WHITESPACES
                {
                    report_space_inside_angle_group(last, source, errors, AngleSide::BeforeClose);
                }
            }
            group_items = parse_group_contents(child, source, errors);
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected 'contents' in group_with_annotations, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Next: optional whitespaces before >. Same E750 contract as the
    // after-`<` position above.
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() == WHITESPACES
    {
        report_space_inside_angle_group(child, source, errors, AngleSide::BeforeClose);
        idx += 1;
    }

    // Next: '>' (required)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == GREATER_THAN {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected '>' in group_with_annotations, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Next: base_annotations (required for groups)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == BASE_ANNOTATIONS {
            let parsed = parse_scoped_annotations(child, source, errors);
            annotations = parsed.content;
            retrace_kind = parsed.retrace;
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected 'base_annotations' in group_with_annotations, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Check for unexpected extra children
    if idx < child_count {
        for extra_idx in idx..child_count {
            if let Some(extra) = node.child(extra_idx as u32) {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(extra.start_byte(), extra.end_byte()),
                    ErrorContext::new(source, extra.start_byte()..extra.end_byte(), ""),
                    format!(
                        "Unexpected extra child '{}' at position {} of group_with_annotations",
                        extra.kind(),
                        extra_idx
                    ),
                ));
            }
        }
    }

    if group_items.is_empty() {
        return ParseOutcome::rejected();
    }

    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let bracketed = BracketedContent::new(group_items);
    let group = Group::new(bracketed).with_span(span);

    if let Some(kind) = retrace_kind {
        // Group retrace: <content> [/] etc.
        let retrace = Retrace::new(group.content, kind)
            .as_group()
            .with_annotations(annotations)
            .with_span(span);
        ParseOutcome::parsed(UtteranceContent::Retrace(Box::new(retrace)))
    } else if annotations.is_empty() {
        ParseOutcome::parsed(UtteranceContent::Group(group))
    } else {
        let annotated = Annotated::new(group)
            .with_scoped_annotations(annotations)
            .with_span(span);
        ParseOutcome::parsed(UtteranceContent::AnnotatedGroup(annotated))
    }
}
