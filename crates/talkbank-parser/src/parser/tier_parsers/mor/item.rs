//! Parsers for `%mor` content items (`mor_content`, post-clitics).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use crate::generated_traversal::{
    AsRawNode, MorContentNode, MorPostCliticNode, MorWordNode, NoChild, NonMissingKindSlot,
    SlotView, SourceBound, SourceField, SourceSlotView,
};
use talkbank_model::ErrorSink;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Mor, MorWord};

use super::word::parse_mor_word;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;

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
/// `child.kind()`. Canonical grammar admission proves that composite `mor_word`
/// and `mor_post_clitic` nodes cannot themselves be Missing. Their children may
/// still contain lexical recovery; Error, absent main words, displaced children
/// and source failures retain their own handling.
pub fn parse_mor_content<'tree>(
    typed: SourceBound<'tree, '_, MorContentNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Mor>, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();
    let bound_children = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let children = bound_children.children();
    surface_displaced(&children.unexpected, "mor_content", source, errors);

    let main_word = decode_main_word(bound_children.field_main().slot(), errors)?;

    let mut post_clitics = Vec::new();
    for element in bound_children.field_post_clitics().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(clitic) => {
                if let ParseOutcome::Parsed(Some(clitic)) =
                    parse_mor_post_clitic(clitic.read()?, errors)?
                {
                    post_clitics.push(clitic);
                }
            }
            SourceSlotView::Error(raw) => {
                errors.report(unexpected_node_error(raw.raw_node(), source, "mor_content"));
            }
            SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
            SourceSlotView::Absent(never) => match never {},
        }
    }

    let Some(main) = main_word else {
        errors.report(unexpected_node_error(
            node,
            source,
            "mor_content missing main mor_word",
        ));
        return Ok(ParseOutcome::rejected());
    };

    Ok(ParseOutcome::parsed(
        Mor::new(main).with_post_clitics(post_clitics),
    ))
}

/// Decode the admitted `main` field, retaining absent and recovered states.
fn decode_main_word<'tree>(
    slot: SourceField<'_, 'tree, '_, NonMissingKindSlot<'tree, MorWordNode<'tree>>>,
    errors: &impl ErrorSink,
) -> Result<Option<MorWord>, crate::CstFailure> {
    let source = slot.source();
    Ok(match slot.view() {
        SourceSlotView::Present(word) => match parse_mor_word(word.read()?, errors)? {
            ParseOutcome::Parsed(word) => Some(word),
            ParseOutcome::Rejected => None,
        },
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(raw.raw_node(), source, "mor_content"));
            None
        }
        SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => None,
    })
}

/// Converts one `mor_post_clitic` CST node (`~` + `mor_word`).
///
/// **Grammar Rule:**
/// ```text
/// mor_post_clitic: $ => seq($.tilde, $.mor_word)
/// ```
///
/// Driven by the generated typed visitor: `extract_mor_post_clitic` yields the
/// tilde and `mor_word` positions as typed `Positioned` slots. Lexical tilde
/// recovery retains its existing no-op policy. Canonical admission rules out
/// a Missing composite `mor_word`, not absence or recovery within that word.
fn parse_mor_post_clitic<'tree>(
    typed: SourceBound<'tree, '_, MorPostCliticNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Option<MorWord>>, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();
    let bound_children = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let children = bound_children.children();
    surface_displaced(&children.unexpected, "mor_post_clitic", source, errors);

    match children.child_0.slot().view() {
        SlotView::Present(_) | SlotView::Missing(_) | SlotView::Absent(NoChild) => {}
        SlotView::Error(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_post_clitic"));
        }
    }

    match bound_children.field_child_1().slot().view() {
        SourceSlotView::Present(word) => {
            if let ParseOutcome::Parsed(word) = parse_mor_word(word.read()?, errors)? {
                return Ok(ParseOutcome::parsed(Some(word)));
            }
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                source,
                "mor_post_clitic",
            ));
        }
        SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => {}
    }

    errors.report(unexpected_node_error(
        node,
        source,
        "mor_post_clitic missing mor_word",
    ));
    Ok(ParseOutcome::parsed(None))
}
