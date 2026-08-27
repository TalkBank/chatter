//! Parsing for quoted main-tier segments.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::UtteranceContent;
use crate::node_types::{
    CA_CONTINUATION_MARKER, CA_NO_BREAK, CA_TECHNICAL_BREAK, COLON, COMMA, CONTENT_ITEM, CONTENTS,
    FALLING_TO_LOW, FALLING_TO_MID, LEFT_DOUBLE_QUOTE, LEVEL_PITCH, NON_COLON_SEPARATOR,
    OVERLAP_POINT, RIGHT_DOUBLE_QUOTE, RISING_TO_HIGH, RISING_TO_MID, SEMICOLON, SEPARATOR,
    TAG_MARKER, UNMARKED_ENDING, UPTAKE_SYMBOL, VOCATIVE_MARKER, WHITESPACES,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use crate::generated_traversal::{
    AsRawNode, FromNodeKind, QuotationWithOptionalAnnotationsNode,
    extract_quotation_with_optional_annotations,
};
use crate::parser::tree_parsing::parser_helpers::{present, surface_unexpected};

use super::group::{convert_to_group_content, parse_nested_content};
use crate::parser::tree_parsing::helpers::unexpected_node_error;

/// Converts a CST `quotation` node into `UtteranceContent`.
///
/// **Grammar Rule:**
/// ```text
/// quotation: $ => seq(
///   seq(
///     '\u201C',  // Left double quotation mark "
///     optional($.whitespaces)
///   ),
///   $.contents,
///   seq(
///     optional($.whitespaces),
///     '\u201D'   // Right double quotation mark "
///   )
/// ),
/// ```
/// Parse a bare `quotation` node.
///
/// PRIVATE to this module since 2026-08-26. It was `pub(crate)` and reached
/// from four `node.kind()` dispatch arms, and those arms became unreachable
/// when `content_item` stopped offering a bare `quotation`: `$.quotation` now
/// has exactly ONE parent in the grammar (`quotation_with_optional_annotations`,
/// verified against `node-types.json`, not by reading the JS). Narrowing the
/// visibility is what makes that structural fact hold, rather than leaving four
/// callers that a future grammar change could quietly revive.
fn parse_quotation_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let mut group_items: Vec<crate::model::BracketedItem> = Vec::new();
    let child_count = node.child_count();
    let mut idx = 0;

    // Position 0: Opening quote mark (LEFT DOUBLE QUOTATION MARK)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == LEFT_DOUBLE_QUOTE {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected opening quote (U+201C) at position 0 of quotation, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Optional whitespace after opening quote - skip it (not semantic content)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() == WHITESPACES
    {
        idx += 1;
    }

    // Parse contents
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == CONTENTS {
            group_items = parse_quotation_contents_items(child, source, errors);
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!("Expected 'contents' in quotation, found '{}'", child.kind()),
            ));
            idx += 1;
        }
    }

    // Optional whitespace before closing quote - skip it (not semantic content)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() == WHITESPACES
    {
        idx += 1;
    }

    // Position last: Closing quote mark (RIGHT DOUBLE QUOTATION MARK)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == RIGHT_DOUBLE_QUOTE {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected closing quote (U+201D) in quotation, found '{}'",
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
                        "Unexpected extra child '{}' at position {} of quotation",
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

    // Create quotation - no space tracking needed
    let bracketed = crate::model::BracketedContent::new(group_items);
    let span = crate::error::Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let quotation = crate::model::Quotation::with_span(bracketed, span);
    // Quotations have no annotations
    ParseOutcome::parsed(UtteranceContent::Quotation(quotation))
}

/// Parse contents inside quotation
///
/// **Grammar Rule:**
/// ```text
/// contents: $ => repeat1(content_item)
/// ```text
fn parse_quotation_contents_items(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<crate::model::BracketedItem> {
    let child_count = node.child_count();
    // Pre-allocate: each child is typically one content item
    let mut group_items = Vec::with_capacity(child_count);
    for idx in 0..child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                CONTENT_ITEM
                | OVERLAP_POINT
                | SEPARATOR
                | NON_COLON_SEPARATOR
                | COLON
                | COMMA
                | SEMICOLON
                | TAG_MARKER
                | VOCATIVE_MARKER
                | CA_CONTINUATION_MARKER
                | UNMARKED_ENDING
                | UPTAKE_SYMBOL
                | CA_NO_BREAK
                | CA_TECHNICAL_BREAK
                | RISING_TO_HIGH
                | RISING_TO_MID
                | LEVEL_PITCH
                | FALLING_TO_MID
                | FALLING_TO_LOW => {
                    for content in parse_nested_content(child, source, errors) {
                        group_items.push(convert_to_group_content(content));
                    }
                }
                // Expected: whitespace between content items (no model representation needed)
                WHITESPACES => {}
                _ => {
                    errors.report(unexpected_node_error(
                        child,
                        source,
                        "quotation contents (expected content_item)",
                    ));
                }
            }
        }
    }

    group_items
}

/// Parse `quotation_with_optional_annotations`: a quotation that may carry
/// scoped annotations.
///
/// # Why this exists
///
/// Until 2026-08-26 `content_item` offered a BARE `quotation`, so a quotation
/// followed by a scoped annotation had no production to reduce into and the
/// whole region became an ERROR node. The CHAT maintainer hit it on a real
/// transcript, where the ERROR-text classifier then blamed the quotes. Real
/// CLAN CHECK accepts the construct, so this is CHECK parity.
///
/// # It reads the GENERATED typed children, not `node.kind()`
///
/// The first version of this function hand-walked `0..child_count` matching
/// `child.kind()`, which root `CLAUDE.md` design rule 6 bans and which cost
/// two defects immediately: a bare `QUOTATION` identifier that Rust read as a
/// BINDING pattern (so every child reached the quotation parser), and no
/// MISSING check, because a tree-sitter MISSING placeholder carries the
/// EXPECTED `kind()` with a zero-length span and therefore passes a `kind`
/// comparison as if real.
///
/// [`NodeSlot`] answers both. It distinguishes `Present` from `Missing`,
/// `Error`, `Unexpected` and `Absent`, so a fabricated quotation is not
/// constructible here; and the extractor's `unexpected` sink is documented as
/// never dropped, which is what the hand-written catch-all was standing in
/// for. The hold-out rationale in `content/base/mod.rs` covers code that would
/// owe a real-corpus comparison to migrate; it does not cover code written after
/// the typed carrier for its own node already existed.
///
/// # It delegates rather than duplicating, in both directions
///
/// The quotation itself goes through [`parse_quotation_content`], and the
/// annotations are folded by `fold_marker_chain`, the single owner of "what
/// happens when an annotation attaches to a content item". That is what makes
/// the bare and annotated forms agree by construction, and it is why teaching
/// `fold_marker_chain` to promote `Quotation` to `AnnotatedQuotation` was the
/// whole of the model-side change.
///
/// [`NodeSlot`]: crate::generated_traversal::NodeSlot
pub(crate) fn parse_quotation_with_annotations_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    // The dispatch sites hold a raw `Node`; classify it once, here.
    let Some(typed) = QuotationWithOptionalAnnotationsNode::from_node(node) else {
        errors.report(unexpected_node_error(
            node,
            source,
            "quotation_with_optional_annotations",
        ));
        return ParseOutcome::rejected();
    };

    let children = extract_quotation_with_optional_annotations(typed);
    // Reported before any early return: a child that filled no grammar
    // position is a fact about the input whether or not the rest parses.
    surface_unexpected(&children.unexpected, source, errors);

    // A MISSING or ERROR quotation is rejected rather than reconstructed. The
    // grammar makes the absent case unreachable on a well-formed parse; on a
    // recovered one it is exactly the state that must not produce a value.
    let Some(quotation) = present(children.quotation.slot()) else {
        return ParseOutcome::rejected();
    };
    let ParseOutcome::Parsed(content) =
        parse_quotation_content(quotation.raw_node(), source, errors)
    else {
        // The inner parser has already reported why; propagate rather than
        // inventing an empty quotation to hang the annotations on.
        return ParseOutcome::rejected();
    };

    // `annotations` is ONE optional slot, so there is no accumulator that a
    // second `base_annotations` child could silently overwrite. The hand-written
    // loop assigned into a `Vec` and carried no note saying why that was safe.
    let markers = match children.annotations.slot() {
        Some(slot) => match present(slot) {
            Some(annotations) => super::super::annotations::parse_scoped_annotations(
                annotations.raw_node(),
                source,
                errors,
            ),
            // Present-but-unusable (MISSING/ERROR): the quotation still stands,
            // and the recovery state is the extractor's to have surfaced.
            None => Vec::new(),
        },
        // Genuinely absent: a bare quotation, which folds to itself.
        None => Vec::new(),
    };

    let span = crate::error::Span::new(node.start_byte() as u32, node.end_byte() as u32);
    ParseOutcome::parsed(super::marker_chain::fold_marker_chain(
        content, markers, span,
    ))
}
