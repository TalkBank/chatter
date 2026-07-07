//! Shared helpers for text-like dependent tier parsers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::generated_traversal::{AsRawNode, NodeSlot};
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::model::BulletContent;
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

/// Compute the source span for an entire tier node.
pub(super) fn tier_span(node: Node) -> Span {
    Span::new(node.start_byte() as u32, node.end_byte() as u32)
}

/// Parse the text/bullet payload of a text-like dependent tier from the tier's
/// already-extracted body slot (`child_2` of `extract_<tier>_dependent_tier`)
/// and surface the carrier's `unexpected` sink.
///
/// This is the shared body parser for `%com` / `%exp` / `%add` / `%spa` / `%sit`
/// / `%int` / `%gpx`; each caller extracts its own tier via the generated typed
/// visitor and hands the body slot AND the carrier's `unexpected` sink here, so
/// the concrete body wrapper type (`TextWithBulletsNode`, or
/// `TextWithBulletsAndPicsNode` for `%com`) is abstracted behind [`AsRawNode`],
/// and every caller surfaces its `unexpected` sink uniformly (R2), matching how
/// the sibling carriers `act.rs` / `cod.rs` / gra / pho / sin already surface
/// theirs.
///
/// `unexpected` is surfaced FIRST via [`surface_unexpected`] (a no-op when
/// empty, which is every case on valid input: the tier's own body slot below
/// is the only position that carries content for these grammar rules).
///
/// The `child_2` slot is matched EXHAUSTIVELY over [`NodeSlot`] (no `_`
/// catch-all, no `.ok()`), reproducing the removed hand-walk loop byte for byte:
///
/// - `Present` / `Missing`: the removed loop matched the body by kind
///   (`text_with_bullets` / `text_with_bullets_and_pics`), and a tree-sitter
///   MISSING node carries that expected kind, so BOTH a real body and a MISSING
///   body were handed to [`parse_bullet_content`]. The raw body node is parsed in
///   both arms (`Present` via [`AsRawNode::raw_node`] on the typed wrapper,
///   `Missing` directly: the NEW backend's `NodeSlot::Missing` carries the raw
///   `tree_sitter::Node`, not the typed wrapper OLD carried, so the two arms can
///   no longer share one `|`-pattern binding, but the observable parse is
///   unchanged). (Empirically the only reachable malformed case, an empty `%com:`
///   body, lands here as `Present` with a MISSING inner `continuation` and
///   recovers to a single `Continuation` segment; the E342 recovery diagnostics
///   come from the whole-tree backstop, not this parser.)
/// - `Error` / `Unexpected`: the removed loop's `_` arm reported
///   [`unexpected_node_error`] for a non-structural, non-text child, then fell
///   through to the end-of-loop "no content" rejection because no text body was
///   found. Both are reproduced, at the same code and span (largely unreachable
///   in practice; the whole-tree recovery backstop covers these).
/// - `Absent`: the removed loop simply never matched a text node and reported the
///   "no content" rejection.
pub(super) fn parse_text_tier_content<'tree, T>(
    tier_node: Node<'tree>,
    body: NodeSlot<'tree, T>,
    unexpected: &[Node<'tree>],
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
    message: &str,
) -> BulletContent
where
    T: AsRawNode<'tree>,
{
    surface_unexpected(unexpected, source, errors);

    match body {
        NodeSlot::Present(text) => parse_bullet_content(text.raw_node(), source, errors),
        NodeSlot::Missing(node) => parse_bullet_content(node, source, errors),
        NodeSlot::Error(node) | NodeSlot::Unexpected(node) => {
            errors.report(unexpected_node_error(node, source, context));
            report_missing_text_content(tier_node, source, errors, context, message);
            BulletContent::from_text("")
        }
        NodeSlot::Absent => {
            report_missing_text_content(tier_node, source, errors, context, message);
            BulletContent::from_text("")
        }
    }
}

/// Report the "no content" rejection for a text-like dependent tier whose body
/// slot carried no parseable `text_with_bullets` node.
///
/// Reproduces the removed hand-walk loop's end-of-loop `TreeParsingError`
/// byte-identically: the same error code, severity, span (the whole tier node),
/// context, and caller-supplied message (for example "Missing content in %com
/// tier").
fn report_missing_text_content(
    tier_node: Node,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
    message: &str,
) {
    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
        ErrorContext::new(
            source,
            tier_node.start_byte()..tier_node.end_byte(),
            context,
        ),
        message.to_string(),
    ));
}
