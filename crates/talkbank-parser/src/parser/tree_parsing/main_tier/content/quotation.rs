//! Parsing for quoted main-tier segments, over the generated typed
//! traversal.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, QuotationNode, QuotationWithOptionalAnnotationsNode, extract_quotation,
    extract_quotation_with_optional_annotations,
};
use crate::model::UtteranceContent;
use talkbank_model::ParseOutcome;

use super::group::{contents_of, parse_group_contents};
use super::report_tree_shape;
use crate::parser::tree_parsing::parser_helpers::{expect_delimiter, present, surface_displaced};

/// Parse a bare `quotation` node.
///
/// Grammar: `seq(left_double_quote, contents, right_double_quote)`. The
/// quote marks are structure, the contents go through the one shared
/// walker, and a quotation with no items is rejected.
///
/// PRIVATE to this module since 2026-08-26. It was `pub(crate)` and reached
/// from four `node.kind()` dispatch arms, and those arms became unreachable
/// when `content_item` stopped offering a bare `quotation`: `$.quotation` now
/// has exactly ONE parent in the grammar (`quotation_with_optional_annotations`,
/// verified against `node-types.json`, not by reading the JS). Narrowing the
/// visibility is what makes that structural fact hold, rather than leaving four
/// callers that a future grammar change could quietly revive.
fn parse_quotation_content(
    typed: QuotationNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let node = typed.raw_node();
    let children = extract_quotation(typed);

    expect_delimiter(children.child_0.slot(), |bad| {
        report_tree_shape(
            bad,
            format!(
                "Expected opening quote (U+201C) at position 0 of quotation, found '{}'",
                bad.kind()
            ),
            source,
            errors,
        );
    });
    let group_items = match contents_of(children.child_1.slot(), |bad| {
        report_tree_shape(
            bad,
            format!("Expected 'contents' in quotation, found '{}'", bad.kind()),
            source,
            errors,
        );
    }) {
        Some(contents) => parse_group_contents(&contents, source, errors),
        None => Vec::new(),
    };
    expect_delimiter(children.child_2.slot(), |bad| {
        report_tree_shape(
            bad,
            format!(
                "Expected closing quote (U+201D) in quotation, found '{}'",
                bad.kind()
            ),
            source,
            errors,
        );
    });
    surface_displaced(&children.unexpected, "quotation", source, errors);

    if group_items.is_empty() {
        return ParseOutcome::rejected();
    }

    let bracketed = crate::model::BracketedContent::new(group_items);
    let span = crate::error::Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let quotation = crate::model::Quotation::with_span(bracketed, span);
    ParseOutcome::parsed(UtteranceContent::Quotation(quotation))
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
/// for. The caller hands in the typed node the `content_item` choice already
/// proved, so there is no kind to re-check on the way in either.
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
    typed: QuotationWithOptionalAnnotationsNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let node = typed.raw_node();
    let children = extract_quotation_with_optional_annotations(typed);
    // Reported before any early return: a child that filled no grammar
    // position is a fact about the input whether or not the rest parses.
    surface_displaced(
        &children.unexpected,
        "quotation_with_optional_annotations",
        source,
        errors,
    );

    // A MISSING or ERROR quotation is rejected rather than reconstructed. The
    // grammar makes the absent case unreachable on a well-formed parse; on a
    // recovered one it is exactly the state that must not produce a value.
    let Some(quotation) = present(children.quotation.slot()) else {
        return ParseOutcome::rejected();
    };
    let ParseOutcome::Parsed(content) = parse_quotation_content(*quotation, source, errors) else {
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
