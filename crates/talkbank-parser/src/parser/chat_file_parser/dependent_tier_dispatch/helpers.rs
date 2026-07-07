//! Shared helper routines for dependent-tier dispatch.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{AsRawNode, NodeSlot};
use crate::model::NonEmptyString;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Read the raw text of a simple text-like dependent tier's body slot as a
/// `NonEmptyString`, driven by the generated typed visitor.
///
/// The 15 raw text-like tiers (`%ort` / `%eng` / `%gls` / `%alt` / `%coh` /
/// `%def` / `%err` / `%fac` / `%flo` / `%par` / `%tim` / `%modsyl` / `%phosyl`
/// / `%phoaln` / `%xphoint`) share the grammar shape
/// `seq(<x>_tier_prefix, tier_sep, text_with_bullets, newline)`, so each caller
/// extracts its own concrete tier via the generated `extract_<kind>_dependent_tier`
/// and hands the body slot (`child_2`, a `text_with_bullets` node) AND the
/// carrier's `unexpected` sink here.
///
/// This replaces the removed `extract_unparsed_tier_content` hand-walk, which
/// located the body by scanning `node.children()` for a child of kind
/// `free_text` / `text_with_bullets` / `text_with_bullets_and_pics`. Behavior is
/// preserved byte for byte:
///
/// - `Present` / `Missing`: the removed loop matched the body BY KIND, and a
///   tree-sitter MISSING node still carries that expected kind, so a MISSING
///   body was ALSO "found" and its (empty) text read; both are handled here by
///   reading the raw node's text (`Present` via [`AsRawNode::raw_node`],
///   `Missing` directly, since the NEW backend's `NodeSlot::Missing` carries the
///   raw `tree_sitter::Node`, not the typed wrapper). A non-empty text yields
///   `Parsed`; an empty text reports "Tier has empty content" at the tier-node
///   span; a UTF-8 error reports at the body-node span, exactly as before.
/// - `Error` / `Unexpected` / `Absent`: no child matched the body kind (the
///   removed loop's `None` branch): an ERROR node has kind `ERROR`, an
///   unexpected node has a different kind, and an absent child is not present at
///   all, so none satisfied the kind filter. Report "Tier is missing content
///   node" at the tier-node span, matching the removed code.
///
/// The carrier's `unexpected` sink is surfaced FIRST via [`surface_unexpected`]
/// (R2; a no-op on valid input, load-bearing for migration Task D).
pub(crate) fn read_tier_body_text<'tree, T>(
    tier_node: Node<'tree>,
    body: NodeSlot<'tree, T>,
    unexpected: &[Node<'tree>],
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<NonEmptyString>
where
    T: AsRawNode<'tree>,
{
    surface_unexpected(unexpected, source, errors);

    match body {
        NodeSlot::Present(text) => decode_body_text(tier_node, text.raw_node(), source, errors),
        NodeSlot::Missing(raw) => decode_body_text(tier_node, raw, source, errors),
        NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(source, tier_node.start_byte()..tier_node.end_byte(), "tier"),
                "Tier is missing content node",
            ));
            ParseOutcome::rejected()
        }
    }
}

/// Decode one body node's raw UTF-8 text into a `NonEmptyString`, reproducing
/// the removed helper's content-node handling: a UTF-8 error reports at the
/// BODY-node span; an empty (or whitespace-only-that-`NonEmptyString`-rejects)
/// text reports "Tier has empty content" at the TIER-node span.
fn decode_body_text(
    tier_node: Node,
    body_node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<NonEmptyString> {
    let text = match body_node.utf8_text(source.as_bytes()) {
        Ok(text) => text,
        Err(e) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(body_node.start_byte(), body_node.end_byte()),
                ErrorContext::new(
                    source,
                    body_node.start_byte()..body_node.end_byte(),
                    "tier_content",
                ),
                format!("Failed to extract UTF-8 text from tier content: {}", e),
            ));
            return ParseOutcome::rejected();
        }
    };

    match NonEmptyString::new(text) {
        Some(content) => ParseOutcome::parsed(content),
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(source, tier_node.start_byte()..tier_node.end_byte(), "tier"),
                "Tier has empty content",
            ));
            ParseOutcome::rejected()
        }
    }
}
