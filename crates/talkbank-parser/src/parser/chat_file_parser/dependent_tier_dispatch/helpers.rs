//! Shared helper routines for dependent-tier dispatch.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{AsRawNode, NodeSlot, TierSepNode, extract_tier_sep};
use crate::model::TextTier;
use crate::model::{NonEmptyString, TierSeparator};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Read the raw text of a simple text-like dependent tier's body slot as a
/// `NonEmptyString`, driven by the generated typed visitor.
///
/// The raw text-like tiers share the grammar shape
/// `seq(<x>_tier_prefix, tier_sep, optional(text_with_bullets), newline)`, so
/// each caller extracts its own concrete tier via the generated
/// `extract_<kind>_dependent_tier` and hands the body slot (`child_2`, a
/// `text_with_bullets` node) AND the carrier's `unexpected` sink here.
///
/// Since the E756 widening made those bodies optional, NOTHING calls this
/// directly: every caller goes through [`read_optional_tier_body_text`] or
/// [`read_optional_tier_body_raw_text`], which unwrap the `Option` and then
/// delegate here for a body that IS present. This function therefore describes
/// what to do with a body that exists, and the absent case is not its business.
/// Its `Absent` arm below is the tree-sitter recovery state, which is a
/// different fact from "the grammar says there need not be one".
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
fn read_tier_body_text<'tree, T>(
    tier_node: Node<'tree>,
    body: &NodeSlot<'tree, T>,
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
        NodeSlot::Missing(raw) => decode_body_text(tier_node, *raw, source, errors),
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

/// Read a text tier's body when the grammar makes that body OPTIONAL.
///
/// The ten text-payload tiers (`%alt %coh %def %eng %err %fac %flo %gls %ort
/// %par`) gained optional bodies on 2026-08-15 with the E756 widening; every
/// other free-text tier followed on 2026-08-16. A tier
/// line with nothing after the separator is a real, invalid construct a file
/// can contain: before, it failed to parse and recovered as E602 "malformed
/// dependent tier header" while the re2c backend read the same file as VALID.
///
/// The empty case lowers to [`TextTier::empty`] rather than reporting from the
/// parse path, because recovery is not validity: the parser says what the file
/// contains and `DependentTier::empty_content_span` lets the validator judge
/// it. That also keeps the tier in the model, so the line survives a roundtrip.
///
/// `TextTier::empty` is called here and in the re2c converter's own lowering;
/// both are parsers saying what the file contains, which is what that
/// constructor's doc asks for.
pub(crate) fn read_optional_tier_body_text<'tree, T>(
    tier_node: Node<'tree>,
    body: &Option<NodeSlot<'tree, T>>,
    unexpected: &[Node<'tree>],
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<TextTier>
where
    T: AsRawNode<'tree>,
{
    // Defined over the raw reader rather than repeating its `match`, so the
    // absent-body policy (surface the carrier's sink, report nothing, return a
    // payload that says it is empty) is stated in exactly one place. Two copies
    // thirty lines apart would agree only by inspection.
    read_optional_tier_body_raw_text(tier_node, body, unexpected, source, errors).map(|content| {
        match content {
            Some(text) => TextTier::new(text),
            None => TextTier::empty(),
        }
    })
}

/// Read a tier's body when the grammar makes that body OPTIONAL and the caller
/// builds its own model type from the raw text.
///
/// Sibling of [`read_optional_tier_body_text`], which owns [`TextTier`]
/// construction because all ten of its callers build the same type. The five
/// remaining free-text tiers (`%tim %modsyl %phosyl %phoaln %xphoint`) each
/// lower their text differently: `%tim` classifies it into time segments, the
/// Phon tiers run their own fallible content parsers. So this returns the
/// question rather than an answer, and each caller says what an absent body
/// means IN ITS OWN TYPE (`TimTier::empty`, or the Phon tiers' empty word
/// lists), which is the state `DependentTier::empty_content_span` then reads.
///
/// `Parsed(None)` is an absent body, NOT a failure: the outer [`ParseOutcome`]
/// says whether the body could be read, the inner [`Option`] says whether there
/// was one to read. Collapsing the two would make an empty tier
/// indistinguishable from an unreadable one, and they get different diagnostics
/// (E756 versus a parse error).
pub(crate) fn read_optional_tier_body_raw_text<'tree, T>(
    tier_node: Node<'tree>,
    body: &Option<NodeSlot<'tree, T>>,
    unexpected: &[Node<'tree>],
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<NonEmptyString>>
where
    T: AsRawNode<'tree>,
{
    match body {
        Some(slot) => read_tier_body_text(tier_node, slot, unexpected, source, errors).map(Some),
        None => {
            surface_unexpected(unexpected, source, errors);
            ParseOutcome::Parsed(None)
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
        Ok(content) => ParseOutcome::parsed(content),
        Err(_) => {
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

/// Decode a dependent tier's `tier_sep` slot (E758 provenance) into a
/// [`TierSeparator`].
///
/// Every dependent-tier grammar shape is `seq(<x>_tier_prefix, tier_sep,
/// <body>, newline)`, so `tier_sep` is always positional child 1 of the
/// generated `<Kind>DependentTierChildren` carrier; callers pass that
/// carrier's `child_1.slot`. Mirrors the main-tier `sep_from_slot` helper
/// (`main_tier/structure/convert/mod.rs`) one level deeper: first unwrap the
/// `tier_sep` node itself, then read its own optional `sep_trailing_space`
/// child (`extract_tier_sep(..).child_2.slot`). Only a `Present` trailing-space
/// node carries a real span; every other outer/inner recovery state
/// (Missing/Error/Unexpected/Absent, or an absent `tier_sep` itself) means no
/// illegal trailing space was captured, and maps to a clean separator (the
/// E758 check itself is a later validation pass over this provenance, not
/// parse-time).
pub(crate) fn dependent_tier_separator(slot: &NodeSlot<'_, TierSepNode<'_>>) -> TierSeparator {
    let NodeSlot::Present(tier_sep) = slot else {
        return TierSeparator::CLEAN;
    };
    let tier_sep_children = extract_tier_sep(*tier_sep);
    let trailing = tier_sep_children.child_2.slot();
    match trailing {
        Some(NodeSlot::Present(sep_node)) => {
            let node = sep_node.raw_node();
            TierSeparator::with_trailing_space(Span::new(
                node.start_byte() as u32,
                node.end_byte() as u32,
            ))
        }
        Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => TierSeparator::CLEAN,
    }
}
