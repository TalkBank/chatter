//! Parsers for `%mor` content items (`mor_content`, post-clitics).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use crate::generated_traversal::{
    AsRawNode, MorContentNode, MorPostCliticNode, ReadableSlot, SlotView, SourceBound,
};
use talkbank_model::ErrorSink;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Mor, MorWord};

use super::word::parse_mor_word;
use crate::node_types::{MOR_CONTENT, MOR_POST_CLITIC};
use crate::parser::tier_parsers::admission::recovery_inside_healthy_tier;
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
/// `child.kind()`. Compiled admission excludes MISSING composite `mor_word`
/// and `mor_post_clitic` nodes. Their children may
/// still contain lexical recovery. The compiled prefix proof establishes the
/// main word's presence. The item belongs to a tier admitted as recovery-free
/// (see the tier module's doc), so a repeated clitic in the `Error` state or
/// a MISSING tilde contradicts that admission and reports the producer fault;
/// displaced children, payload rejection and source failures keep their own
/// handling.
pub fn parse_mor_content<'tree>(
    typed: SourceBound<'tree, '_, MorContentNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Mor>, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();
    let bound_children = crate::parser::typed_cst::extract_inner(typed)?;
    let children = bound_children.children();
    surface_displaced(&children.unexpected, "mor_content", source, errors);

    let admitted = bound_children.admit_ranges()?;
    let children = admitted.children();
    let SlotView::Present(word) = children.main.slot.view();
    let main_word = match parse_mor_word(*word, errors)? {
        ParseOutcome::Parsed(word) => Some(word),
        ParseOutcome::Rejected => None,
    };

    let mut post_clitics = Vec::new();
    for element in &children.post_clitics.slot {
        match &element.slot {
            ReadableSlot::Present(clitic) => {
                if let ParseOutcome::Parsed(Some(clitic)) = parse_mor_post_clitic(*clitic, errors)?
                {
                    post_clitics.push(clitic);
                }
            }
            ReadableSlot::Error(raw) => {
                recovery_inside_healthy_tier(raw.raw_node(), raw.source(), MOR_CONTENT, errors);
                return Ok(ParseOutcome::rejected());
            }
            ReadableSlot::Missing(missing) => match missing.0 {},
            ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
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

/// Converts one `mor_post_clitic` CST node (`~` + `mor_word`).
///
/// **Grammar Rule:**
/// ```text
/// mor_post_clitic: $ => seq($.tilde, $.mor_word)
/// ```
///
/// Driven by the generated typed visitor: `extract_mor_post_clitic` yields the
/// tilde and `mor_word` positions as typed `Positioned` slots. A MISSING tilde
/// contradicts the tier's admission. Compiled admission excludes a MISSING
/// composite `mor_word`; source admission does not prove syntax validity.
fn parse_mor_post_clitic<'tree>(
    typed: SourceBound<'tree, '_, MorPostCliticNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Option<MorWord>>, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();
    let bound_children = crate::parser::typed_cst::extract_inner(typed)?;
    let children = bound_children.children();
    surface_displaced(&children.unexpected, "mor_post_clitic", source, errors);
    let admitted = bound_children.admit_ranges()?;
    let children = admitted.children();

    match children.child_0.slot.view() {
        SlotView::Present(_) => {}
        SlotView::Missing(missing) => {
            recovery_inside_healthy_tier(
                missing.raw_node(),
                missing.source(),
                MOR_POST_CLITIC,
                errors,
            );
            return Ok(ParseOutcome::rejected());
        }
    }

    match children.child_1.slot.view() {
        SlotView::Present(word) => {
            if let ParseOutcome::Parsed(word) = parse_mor_word(*word, errors)? {
                return Ok(ParseOutcome::parsed(Some(word)));
            }
        }
    }

    errors.report(unexpected_node_error(
        node,
        source,
        "mor_post_clitic missing mor_word",
    ));
    Ok(ParseOutcome::parsed(None))
}
