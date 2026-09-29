#![deny(clippy::wildcard_enum_match_arm)]

//! Parse `contents` subtrees into `UtteranceContent` sequences.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>

use crate::error::ErrorSink;
use crate::generated_traversal::{Absence, Never, NodeSlot};
use crate::model::UtteranceContent;
use talkbank_model::ParseOutcome;

use crate::generated_traversal::{
    AsRawNode, ContentItemChoice, ContentItemChoiceBoundView, ContentItemNode,
    ContentsChild0Choice, ContentsChild0ChoiceBoundView, ContentsChild1Choice,
    ContentsChild1ChoiceBoundView, ContentsChildren, ContentsNode, OverlapPointNode, SeparatorNode,
    SourceBound, SourceBoundKind, SourceChildren, SourceField, SourceSlotView,
};
use crate::parser::typed_cst::{read_source_field, report_cst_failure};

use super::super::super::parser_helpers::parse_separator_node;
use super::super::content::{
    MainTierRegion, classify_main_tier_recovery, illegal_curly_quote_error, misplaced_linker_error,
    parse_overlap_point, surface_main_tier_sink,
};
use crate::parser::tree_parsing::helpers::unexpected_node_error;

/// One `contents` alternative, keyed by its NEW-backend per-position choice
/// enum (`ContentsChild0Choice` for the required first element,
/// `ContentsChild1Choice` for each repeated-tail element): structurally
/// identical 4-way choices (`whitespaces` / `content_item` / `separator` /
/// `overlap_point`) the generator mangles into two separately-named types
/// because `contents = repeat1(..)` splits into a required-first `child_0`
/// plus a repeated-tail `child_1`. This trait lets the shared per-item
/// processing below handle both with one body.
trait ContentsItem<'tree>: SourceBoundKind<'tree> {
    /// Which alternative this item is, retaining its admitted source-bound node.
    ///
    /// The choice enum the generator emits already proves the kind, so the
    /// per-item processing dispatches on this and never re-reads
    /// `node.kind()`: a `contents` child is one of exactly these four, and a
    /// match with no other arm is the grammar's own statement of that.
    fn leaf<'source>(bound: SourceBound<'tree, 'source, Self>) -> ContentsLeaf<'tree, 'source>;
}

/// The four things a `contents` child can be. Each parsed alternative retains
/// its producer-issued wrapper through dispatch.
enum ContentsLeaf<'tree, 'source> {
    /// Whitespace between items; contributes nothing.
    Whitespace,
    /// A `content_item` wrapper around a word, group, quotation or the like.
    ContentItem(SourceBound<'tree, 'source, ContentItemNode<'tree>>),
    /// A bare separator token, which the grammar places directly under
    /// `contents` (a colon after an overlap marker, for one).
    Separator(SourceBound<'tree, 'source, SeparatorNode<'tree>>),
    /// A bare overlap marker, likewise a direct child.
    OverlapPoint(SourceBound<'tree, 'source, OverlapPointNode<'tree>>),
}

impl<'tree> ContentsItem<'tree> for ContentsChild0Choice<'tree> {
    fn leaf<'source>(bound: SourceBound<'tree, 'source, Self>) -> ContentsLeaf<'tree, 'source> {
        match bound.view() {
            ContentsChild0ChoiceBoundView::Whitespaces(_) => ContentsLeaf::Whitespace,
            ContentsChild0ChoiceBoundView::ContentItem(n) => ContentsLeaf::ContentItem(n),
            ContentsChild0ChoiceBoundView::Separator(n) => ContentsLeaf::Separator(n),
            ContentsChild0ChoiceBoundView::OverlapPoint(n) => ContentsLeaf::OverlapPoint(n),
        }
    }
}

impl<'tree> ContentsItem<'tree> for ContentsChild1Choice<'tree> {
    fn leaf<'source>(bound: SourceBound<'tree, 'source, Self>) -> ContentsLeaf<'tree, 'source> {
        match bound.view() {
            ContentsChild1ChoiceBoundView::Whitespaces(_) => ContentsLeaf::Whitespace,
            ContentsChild1ChoiceBoundView::ContentItem(n) => ContentsLeaf::ContentItem(n),
            ContentsChild1ChoiceBoundView::Separator(n) => ContentsLeaf::Separator(n),
            ContentsChild1ChoiceBoundView::OverlapPoint(n) => ContentsLeaf::OverlapPoint(n),
        }
    }
}

/// Parse main-tier `contents` nodes into ordered `UtteranceContent` items.
///
/// The `contents` rule (`repeat1(choice(whitespaces, content_item, separator, overlap_point))`)
/// collects words, separators, overlap markers, and other inline tokens described in the Main Tier
/// section of the manual. Iteration is driven by the generated typed visitor: one
/// source-bound extraction yields the required first element (`child_0`) plus the repeated tail
/// (`child_1`, a `Vec`), each a source-associated slot over its own per-position choice enum
/// ([`ContentsChild0Choice`] / [`ContentsChild1Choice`]), so structure comes from typed node
/// dispatch rather than `node.kind()` string matching, and a recovery node can never be silently
/// dropped. Unlike the OLD backend's lazy `extract_contents_iter`, the NEW backend has no iterator
/// form (every migrated cluster in this workstream materializes its repeats eagerly, per the B1
/// template), so the `Vec` for `child_1` is fully built before this function iterates it; this is a
/// deliberate, accepted architectural property of the NEW backend, not a generator gap. Each
/// concrete choice is handed to [`parse_content_item`]. An `ERROR` fragment
/// remains structural recovery evidence; its text is never glued to a preceding
/// word or used to reconstruct an unparsed lexical token.
pub fn parse_main_tier_contents<'tree>(
    typed: SourceBound<'tree, '_, ContentsNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Vec<UtteranceContent>, crate::generated_traversal::ReconstructionFault> {
    Ok(parse_contents(&typed.extract()?, errors))
}

/// Parse source-associated `contents` children, wherever they sit.
///
/// The bracketed constructs (angle group, quotation, pho and sin groups)
/// extract their `contents` slot themselves, because the angle group reads
/// its edge whitespace off the same extraction for E750, and hand the
/// children here. Until 2026-09-08 each of them walked `contents` by hand
/// through a second, `node.kind()`-driven copy of this dispatch
/// (`group/nested.rs`), with its own arms for every separator kind the
/// grammar never places there.
pub(crate) fn parse_contents<'tree>(
    contents: &SourceChildren<'tree, '_, ContentsChildren<'tree>>,
    errors: &impl ErrorSink,
) -> Vec<UtteranceContent> {
    // `content` starts empty rather than pre-sized to the child count: that
    // count includes the (now explicit) `whitespaces` children, so on a normal
    // whitespace-separated utterance it over-allocates by roughly 2x. Utterances
    // are short, so the one or two reallocations a growing `Vec` costs are cheaper
    // than a guaranteed 2x over-allocation and leave no wasted capacity.
    let mut content = Vec::new();
    process_contents_slot(contents.field_child_0().slot(), errors, &mut content);
    for element in contents.field_child_1().slot().iter() {
        process_contents_slot(element.slot(), errors, &mut content);
    }
    surface_main_tier_sink(contents.children(), contents.source(), errors);
    content
}

/// Process one `contents` position's slot (either the required `child_0` or one
/// element of the repeated `child_1` tail) into `content`.
///
/// A present item is dispatched on the typed choice it already carries; a
/// MISSING placeholder is classified like a present one through the same
/// typed constructors, which is the precedent `separator.rs` set and
/// explained: a migration that changes diagnostics is a behaviour change
/// wearing a refactor's clothes, and this one changes none.
fn process_contents_slot<'tree, C: ContentsItem<'tree>, A: Absence>(
    slot: SourceField<'_, 'tree, '_, NodeSlot<'tree, C, tree_sitter::Node<'tree>, Never, A>>,
    errors: &impl ErrorSink,
    content: &mut Vec<UtteranceContent>,
) {
    let source = slot.source();
    match slot.view() {
        // The choice enum names which of the four kinds this is; nothing here
        // reads `node.kind()`.
        SourceSlotView::Present(item) => {
            let Some(item) = read_source_field(item, errors) else {
                return;
            };
            let parsed = match C::leaf(item) {
                ContentsLeaf::Whitespace => return,
                ContentsLeaf::Separator(node) => {
                    parse_separator_node(node, errors).map(UtteranceContent::Separator)
                }
                ContentsLeaf::OverlapPoint(node) => parse_overlap_point(node, errors),
                ContentsLeaf::ContentItem(node) => parse_content_item(node, errors),
            };
            if let ParseOutcome::Parsed(parsed) = parsed {
                content.push(parsed);
            }
        }
        // A zero-width MISSING placeholder at a content position. It carries
        // no choice classification, so it is classified through the typed
        // constructors and parsed like a present item, which is what the
        // old kind() dispatch did (it never checked `is_missing`) and what
        // `separator.rs` chose for the same case, for the reason its comment
        // gives. A MISSING `whitespaces` is the one kind the old dispatch had
        // no arm for, and it fell to the fail-loud arm; it still does.
        SourceSlotView::Missing(item_node) => {
            let node = item_node.raw_node();
            let bound = match item_node.read_raw() {
                Ok(bound) => bound,
                Err(error) => {
                    report_cst_failure(node, source, error, errors);
                    return;
                }
            };
            let parsed = if let Some(separator) = bound.typed::<SeparatorNode>() {
                parse_separator_node(separator, errors).map(UtteranceContent::Separator)
            } else if let Some(overlap) = bound.typed::<OverlapPointNode>() {
                parse_overlap_point(overlap, errors)
            } else if let Some(item) = bound.typed::<ContentItemNode>() {
                parse_content_item(item, errors)
            } else {
                errors.report(unexpected_node_error(node, source, "content item"));
                ParseOutcome::rejected()
            };
            if let ParseOutcome::Parsed(parsed) = parsed {
                content.push(parsed);
            }
        }
        // ERROR is recovery evidence, not an admitted word suffix. Keep its
        // exact source span for diagnostics; never manufacture a lexical
        // relationship with the preceding typed word from text resemblance.
        SourceSlotView::Error(error_node) => {
            errors.report(classify_main_tier_recovery(
                error_node.raw_node(),
                error_node.source(),
                MainTierRegion::Body,
            ));
        }
        // Selected choices cannot produce Unexpected; producer inconsistencies
        // return CstFailure before content construction.
        SourceSlotView::Unexpected(never) => match never {},
        // Reachable at `child_0` (never at a `child_1` repeat element, since
        // `repeat_split` never pushes an `Absent` element there) when the
        // OUTER `contents` node itself is a childless MISSING placeholder
        // (`body.rs`'s `NodeSlot::Missing` arm for the tier-body `content`
        // position hands this function a zero-width synthetic node with no
        // children at all, so `child_0`'s cursor peek finds nothing). Matches
        // the OLD backend's `extract_contents_iter` on the same input, which
        // likewise yields zero items (empty iteration over a childless node):
        // both produce empty `content`. No-op, not a diagnostic (a missing
        // `contents` node's own "Missing"-ness is reported once, by `body.rs`'s
        // caller, not duplicated here).
        SourceSlotView::Absent(_) => {}
    }
}

/// Parse a `content_item` wrapper into `UtteranceContent`.
///
/// The wrapper has one grammar position, `content`, holding one of the
/// alternatives `ContentItemChoice` names: base content, an annotated
/// group, an annotated quotation, an illegal curly quote, a pho or sin
/// group, or a misplaced linker. Each alternative goes to its own parser
/// with its typed node, so no parser re-checks the kind it was handed.
/// Until 2026-09-08 this function walked the wrapper's children matching
/// `node.kind()`, and `group/nested.rs` kept a second copy of that walk for
/// the same wrapper inside groups.
fn parse_content_item<'tree>(
    typed: SourceBound<'tree, '_, ContentItemNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let source = typed.source();
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        typed.extract(),
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let outcome = match children.field_content().slot().view() {
        SourceSlotView::Present(choice) => match read_source_field(choice, errors) {
            Some(choice) => parse_content_item_choice(choice, errors),
            None => ParseOutcome::rejected(),
        },
        // A zero-width MISSING placeholder for a whole construct: classified
        // through the typed constructors and parsed like a present one, the
        // precedent `separator.rs` set (the old walk never checked
        // `is_missing` here either).
        SourceSlotView::Missing(placeholder) => match placeholder.read_typed::<ContentItemChoice>()
        {
            Some(Ok(choice)) => parse_content_item_choice(choice, errors),
            Some(Err(error)) => {
                report_cst_failure(placeholder.raw_node(), source, error, errors);
                ParseOutcome::rejected()
            }
            None => {
                errors.report(unexpected_node_error(
                    placeholder.raw_node(),
                    source,
                    "content item child",
                ));
                ParseOutcome::rejected()
            }
        },
        SourceSlotView::Error(error_node) => {
            errors.report(classify_main_tier_recovery(
                error_node.raw_node(),
                source,
                MainTierRegion::Body,
            ));
            ParseOutcome::rejected()
        }
        SourceSlotView::Unexpected(never) => match never {},
        // A `content_item` with no child at all, which only a childless
        // MISSING wrapper can be; it carries nothing to parse.
        SourceSlotView::Absent(_) => ParseOutcome::rejected(),
    };
    surface_main_tier_sink(children.children(), source, errors);
    outcome
}

/// Dispatch one `content_item` alternative to the parser that owns it.
fn parse_content_item_choice<'tree>(
    choice: SourceBound<'tree, '_, ContentItemChoice<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    use super::super::content::{
        parse_base_content, parse_group_content, parse_pho_group_content,
        parse_quotation_with_annotations_content, parse_sin_group_content,
    };
    let source = choice.source();
    match choice.view() {
        ContentItemChoiceBoundView::BaseContentItem(base) => parse_base_content(base, errors),
        ContentItemChoiceBoundView::GroupWithAnnotations(group) => {
            parse_group_content(group, errors)
        }
        ContentItemChoiceBoundView::QuotationWithOptionalAnnotations(quotation) => {
            parse_quotation_with_annotations_content(quotation, errors)
        }
        ContentItemChoiceBoundView::MainPhoGroup(pho) => parse_pho_group_content(pho, errors),
        ContentItemChoiceBoundView::MainSinGroup(sin) => parse_sin_group_content(sin, errors),
        // A recognised illegal curly single quote: E256, no model element.
        ContentItemChoiceBoundView::IllegalCurlyQuote(quote) => {
            errors.report(illegal_curly_quote_error(quote.raw_node(), source));
            ParseOutcome::rejected()
        }
        // A linker in content position. Linkers are utterance-initial by
        // definition; one that reduced here instead of into the tier body's
        // `linkers` field is misplaced: E766, no model element.
        ContentItemChoiceBoundView::CaNoBreakLinker(linker) => {
            errors.report(misplaced_linker_error(linker));
            ParseOutcome::rejected()
        }
    }
}
