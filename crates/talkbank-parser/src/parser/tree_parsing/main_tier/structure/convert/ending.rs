//! Utterance-end tail extraction for `main_tier` conversion.
//!
//! Driven by the generated typed visitor: one `extract_utterance_end` call yields
//! the optional terminator (as a typed per-position `UtteranceEndChild0Choice`),
//! the optional `final_codes` (postcodes), the optional trailing media `bullet`
//! (nested one level under an explicit-whitespace group), an explicit trailing
//! `whitespace` slot, and the required `newline`, all as typed slots. This
//! replaces the previous flat positional `node.kind()` loop (the removed
//! `parse_utterance_end` in `utterance_end.rs`), so the 13 terminator subtypes
//! are now a compiler-exhaustive typed match (the shared `terminator_from_new_choice`)
//! instead of a `node.kind()` string dispatch. Each slot is matched EXHAUSTIVELY
//! over [`NodeSlot`], so a recovery node is handled explicitly rather than
//! silently dropped, and the ONE reachable recovery diagnostic (E360
//! `InvalidMediaBullet` on a malformed trailing bullet) is reproduced
//! byte-identically.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Postcodes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, FinalCodesChild0Children, FinalCodesChild1Children, NodeSlot, PostcodeNode,
    UtteranceEndNode, extract_final_codes, extract_utterance_end,
};
use crate::model::{Bullet, Postcode, Terminator};
use crate::parser::tree_parsing::media_bullet::parse_bullet_node_timestamps;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use crate::parser::tree_parsing::postcode::parse_postcode_node;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::super::content::analyze_word_error;
use super::super::terminator::{span_of, terminator_from_new_choice};

/// The utterance-end tail parsed from an `utterance_end` node: the optional
/// terminator, the ordered postcodes, and the optional trailing media bullet. The
/// `parse_tier_body` seam folds these fields into `TierBodyData`. A named struct
/// (not a tuple) keeps this domain seam self-documenting, matching the sibling
/// `TierBodyData`; `Default` is the empty tail used on the recovery arms.
#[derive(Default)]
pub(super) struct UtteranceEndTail {
    pub terminator: Option<Terminator>,
    pub postcodes: Vec<Postcode>,
    pub bullet: Option<Bullet>,
}

/// Decode a typed `utterance_end` node into its terminator, postcodes, and trailing
/// media bullet, reporting any diagnostics into `errors`.
///
/// Driven by `extract_utterance_end`, which yields five ordered slots
/// (`terminator` optional, `final_codes` optional, `bullet` optional -
/// nested under an explicit-whitespace group, an explicit trailing
/// `whitespace` optional, `newline` required). Every slot is matched
/// EXHAUSTIVELY; the valid path emits no diagnostics. Replaces the removed
/// flat-loop `parse_utterance_end`.
pub(super) fn parse_utterance_end(
    node: Node<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> UtteranceEndTail {
    let end = extract_utterance_end(UtteranceEndNode(node));

    // child_0 (`terminator` supertype, optional). A `Present` choice maps through
    // the exhaustive typed match; an `Error` slot routes to the shared word-error
    // analyzer (as the body.rs seam does for its ERROR arms) and yields no
    // terminator; `Missing` / `Unexpected` / an absent slot yield no terminator and
    // no invented diagnostic (a truly missing terminator is a validation concern,
    // reported at validation time, not here). The NEW-backend `Missing` recovery
    // variant carries a raw `Node` (not a typed choice), matching `Error` /
    // `Unexpected` below, unlike the OLD flattened `Option<NodeSlot<..>>` where
    // `Missing` still carried the typed choice.
    let terminator = match end.child_0.slot {
        Some(NodeSlot::Present(choice)) => Some(terminator_from_new_choice(&choice)),
        Some(NodeSlot::Error(error_node)) => {
            errors.report(analyze_word_error(error_node, source));
            None
        }
        Some(NodeSlot::Missing(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent) | None => None,
    };

    // child_1 (`final_codes`, optional). Only a `Present` `final_codes` contributes
    // postcodes: re-extract its elements and parse each `Present` `postcode`. The
    // NEW backend models `final_codes = repeat1(seq(whitespaces, postcode))` as a
    // required first group (`child_0`) plus a repeated tail (`child_1`, a Vec of
    // the SAME two-position group shape), because whitespace between codes is now
    // an explicit grammar position rather than skipped; the postcode itself moved
    // from the OLD flat `element.child_0` to the group's `child_1`. Every other
    // element/group slot state is skipped (the safe default, matching the removed
    // loop which acted only on `postcode`-kind children); a `Missing` / `Error` /
    // `Unexpected` / absent `final_codes` slot yields no postcodes.
    let mut postcodes: Vec<Postcode> = Vec::new();
    match end.child_1.slot {
        Some(NodeSlot::Present(final_codes)) => {
            let codes = extract_final_codes(final_codes);
            push_postcode_from_final_codes_group(
                codes.child_0.slot,
                source,
                errors,
                &mut postcodes,
            );
            for element in codes.child_1.slot {
                push_postcode_from_final_codes_group(element.slot, source, errors, &mut postcodes);
            }
            surface_unexpected(&codes.unexpected, source, errors);
        }
        Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => {}
    }

    // child_2 (`bullet`, optional). The NEW backend groups the trailing
    // `[whitespace?, bullet]` pair into one nested optional carrier
    // (`UtteranceEndChild2Children`) because the interstitial whitespace is now
    // an explicit position; descend to its `child_1` (the bullet) exactly as the
    // B2 nested-group precedent does. A `Present` bullet with valid timestamps
    // becomes a `Bullet` carrying the node span; on `None` timestamps (a
    // grammar-rejected or malformed bullet, e.g. the deprecated `·N_N-·` skip
    // marker) the E360 diagnostic is emitted byte-identically to the removed
    // flat loop, so the file still fails validation. Every other slot state
    // (at either nesting level) yields no bullet, no diagnostic.
    let bullet = match end.child_2.slot {
        Some(NodeSlot::Present(group)) => {
            surface_unexpected(&group.unexpected, source, errors);
            match group.child_1.slot {
                NodeSlot::Present(bullet_node) => {
                    let raw = bullet_node.raw_node();
                    match parse_bullet_node_timestamps(raw, source, errors) {
                        Some((start_ms, end_ms)) => {
                            Some(Bullet::new(start_ms, end_ms).with_span(span_of(raw)))
                        }
                        None => {
                            report_invalid_media_bullet(raw, source, errors);
                            None
                        }
                    }
                }
                NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) => None,
                NodeSlot::Absent => None,
            }
        }
        Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => None,
    };

    // child_3 (trailing `whitespace`, optional). NEWLY MATERIALIZED position (the
    // OLD backend's `--skip whitespaces` absorbed this silently): structural
    // only, carries no terminator, postcode, or bullet, so every slot state is a
    // no-op. Matched explicitly so no state is silently dropped.
    match end.child_3.slot {
        Some(
            NodeSlot::Present(_)
            | NodeSlot::Missing(_)
            | NodeSlot::Error(_)
            | NodeSlot::Unexpected(_)
            | NodeSlot::Absent,
        )
        | None => {}
    }

    // child_4 (`newline`, required; was `child_3` under OLD). Structural only: it
    // carries no terminator, postcode, or bullet, so every slot state is a no-op.
    // Matched explicitly so the required newline slot is never silently dropped.
    match end.child_4.slot {
        NodeSlot::Present(_)
        | NodeSlot::Missing(_)
        | NodeSlot::Error(_)
        | NodeSlot::Unexpected(_)
        | NodeSlot::Absent => {}
    }

    // Surface the carrier's own `unexpected` sink (R2). Empty on every fixture
    // probed so far; load-bearing once the whole-tree backstop is deleted.
    surface_unexpected(&end.unexpected, source, errors);

    UtteranceEndTail {
        terminator,
        postcodes,
        bullet,
    }
}

/// A `final_codes` element group: `{ whitespaces, postcode }`.
///
/// The NEW backend generates two SEPARATELY-NAMED, structurally-identical
/// carrier types for this shape: `FinalCodesChild0Children` (the required
/// first group; `final_codes = repeat1(...)` always has at least one) and
/// `FinalCodesChild1Children` (each element of the repeated tail). This trait
/// lets [`push_postcode_from_final_codes_group`] handle both with one body
/// instead of duplicating the match.
trait FinalCodesGroup<'tree> {
    /// The group's `unexpected` sink (R2).
    fn group_unexpected(&self) -> &[tree_sitter::Node<'tree>];
    /// The group's `postcode` slot (`child_1`, after the leading whitespace).
    fn postcode_slot(self) -> NodeSlot<'tree, PostcodeNode<'tree>>;
}
impl<'tree> FinalCodesGroup<'tree> for FinalCodesChild0Children<'tree> {
    fn group_unexpected(&self) -> &[tree_sitter::Node<'tree>] {
        &self.unexpected
    }
    fn postcode_slot(self) -> NodeSlot<'tree, PostcodeNode<'tree>> {
        self.child_1.slot
    }
}
impl<'tree> FinalCodesGroup<'tree> for FinalCodesChild1Children<'tree> {
    fn group_unexpected(&self) -> &[tree_sitter::Node<'tree>] {
        &self.unexpected
    }
    fn postcode_slot(self) -> NodeSlot<'tree, PostcodeNode<'tree>> {
        self.child_1.slot
    }
}

/// Decode one `final_codes` element group's outer `NodeSlot` into `postcodes`.
///
/// Every non-`Present` group state (including a `Present` group whose own
/// `postcode` slot is not `Present`) yields no postcode, matching the removed
/// flat loop which acted only on `postcode`-kind children.
fn push_postcode_from_final_codes_group<'tree, G: FinalCodesGroup<'tree>>(
    group_slot: NodeSlot<'tree, G>,
    source: &str,
    errors: &impl ErrorSink,
    postcodes: &mut Vec<Postcode>,
) {
    match group_slot {
        NodeSlot::Present(group) => {
            surface_unexpected(group.group_unexpected(), source, errors);
            match group.postcode_slot() {
                NodeSlot::Present(postcode_node) => {
                    if let ParseOutcome::Parsed(postcode) =
                        parse_postcode_node(postcode_node.raw_node(), source, errors)
                    {
                        postcodes.push(postcode);
                    }
                }
                NodeSlot::Missing(_)
                | NodeSlot::Error(_)
                | NodeSlot::Unexpected(_)
                | NodeSlot::Absent => {}
            }
        }
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {}
    }
}

/// Report the E360 `InvalidMediaBullet` diagnostic for a malformed trailing bullet.
///
/// Reproduces the removed `parse_utterance_end` bullet-`None` arm byte-identically:
/// the same error code, severity, span, context (carrying the bullet text, or
/// `<unreadable>` on a UTF-8 error), and message. The `<unreadable>` fallback is
/// defensive only, since the CHAT source is already valid UTF-8 in practice.
fn report_invalid_media_bullet(node: Node<'_>, source: &str, errors: &impl ErrorSink) {
    let bullet_text = node.utf8_text(source.as_bytes()).unwrap_or("<unreadable>");
    errors.report(ParseError::new(
        ErrorCode::InvalidMediaBullet,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), bullet_text),
        format!(
            "Invalid media bullet: grammar rejected '{}'. Legal form: ·START_END· with numeric timestamps only",
            bullet_text
        ),
    ));
}
