//! Parsers for `%mor` content items (`mor_content`, post-clitics).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use crate::generated_traversal::{
    AsRawNode, MorContentNode, MorPostCliticChildren, MorPostCliticNode, MorWordNode, NodeSlot,
    SlotValue, extract_mor_content, extract_mor_post_clitic,
};
use talkbank_model::ErrorSink;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Mor, MorWord};

use super::word::parse_mor_word;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;

/// Converts a `mor_content` CST node into one `Mor` item.
///
/// **Grammar Rule:**
/// ```text
/// mor_content: $ => seq(
///     field('main', $.mor_word),
///     field('post_clitics', repeat($.mor_post_clitic))
/// )
/// ```
///
/// Driven by the generated typed visitor: `extract_mor_content` yields the
/// named `main` and `post_clitics` fields as typed `Positioned` slots,
/// replacing the removed flat `while node.child(idx)` walk that dispatched by
/// `child.kind()`. Note the removed walk called neither `check_not_missing`
/// nor any other MISSING-specific gate: a MISSING `mor_word` still carries the
/// `mor_word` kind, so it was dispatched into [`parse_mor_word`] exactly like
/// a real one (which itself then finds zero children and reports its own
/// "missing POS/lemma" diagnostics). The migration reproduces that by feeding
/// `Present` AND `Missing` slots to [`parse_mor_word`] / [`parse_mor_post_clitic`]
/// identically; only `Error`/`Unexpected` diverge from `Present`/`Missing`,
/// matching the removed loop's `_ =>` arm.
pub fn parse_mor_content(
    typed: MorContentNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Mor> {
    let node = typed.raw_node();
    let children = extract_mor_content(typed);
    surface_unexpected(&children.unexpected, source, errors);

    let main_word = decode_main_word(children.main.slot(), source, errors);

    let mut post_clitics = Vec::new();
    for element in children.post_clitics.slot() {
        match element.slot().typed_or_placeholder() {
            SlotValue::Present(clitic) | SlotValue::Placeholder(clitic) => {
                if let ParseOutcome::Parsed(Some(clitic)) =
                    parse_mor_post_clitic(clitic, source, errors)
                {
                    post_clitics.push(clitic);
                }
            }
            SlotValue::UnclassifiedPlaceholder(raw)
            | SlotValue::Error(raw)
            | SlotValue::Unexpected(raw) => {
                errors.report(unexpected_node_error(raw, source, "mor_content"));
            }
            SlotValue::Absent => {}
        }
    }

    let Some(main) = main_word else {
        errors.report(unexpected_node_error(
            node,
            source,
            "mor_content missing main mor_word",
        ));
        return ParseOutcome::rejected();
    };

    ParseOutcome::parsed(Mor::new(main).with_post_clitics(post_clitics))
}

/// Decode the `main` field slot, dispatching `Present`/`Missing` into
/// [`parse_mor_word`] alike (see the module doc comment for why), and
/// reporting `Error`/`Unexpected` the way the removed loop's `_ =>` arm did.
fn decode_main_word<'tree>(
    slot: &NodeSlot<'tree, MorWordNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
) -> Option<MorWord> {
    match slot.typed_or_placeholder() {
        SlotValue::Present(word) | SlotValue::Placeholder(word) => {
            match parse_mor_word(word, source, errors) {
                ParseOutcome::Parsed(word) => Some(word),
                ParseOutcome::Rejected => None,
            }
        }
        SlotValue::UnclassifiedPlaceholder(raw)
        | SlotValue::Error(raw)
        | SlotValue::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_content"));
            None
        }
        SlotValue::Absent => None,
    }
}

/// Converts one `mor_post_clitic` CST node (`~` + `mor_word`).
///
/// **Grammar Rule:**
/// ```text
/// mor_post_clitic: $ => seq($.tilde, $.mor_word)
/// ```
///
/// Driven by the generated typed visitor: `extract_mor_post_clitic` yields the
/// tilde and `mor_word` positions as typed `Positioned` slots. The removed
/// walk called no MISSING-specific gate either (see [`parse_mor_content`]'s
/// doc comment); a MISSING `tilde` is a no-op exactly like a present one
/// (`kind::TILDE => {}` never distinguished missing-ness), and a MISSING
/// `mor_word` is still dispatched into [`parse_mor_word`] like a present one.
fn parse_mor_post_clitic(
    typed: MorPostCliticNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<MorWord>> {
    let node = typed.raw_node();
    let children: MorPostCliticChildren<'_> = extract_mor_post_clitic(typed);
    surface_unexpected(&children.unexpected, source, errors);

    match children.child_0.slot() {
        NodeSlot::Present(_) | NodeSlot::Missing(_) | NodeSlot::Absent => {}
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(*raw, source, "mor_post_clitic"));
        }
    }

    match children.child_1.slot().typed_or_placeholder() {
        SlotValue::Present(word) | SlotValue::Placeholder(word) => {
            if let ParseOutcome::Parsed(word) = parse_mor_word(word, source, errors) {
                return ParseOutcome::parsed(Some(word));
            }
        }
        SlotValue::UnclassifiedPlaceholder(raw)
        | SlotValue::Error(raw)
        | SlotValue::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_post_clitic"));
        }
        SlotValue::Absent => {}
    }

    errors.report(unexpected_node_error(
        node,
        source,
        "mor_post_clitic missing mor_word",
    ));
    ParseOutcome::parsed(None)
}
