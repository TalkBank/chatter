#![deny(clippy::wildcard_enum_match_arm)]

//! Parse `contents` subtrees into `UtteranceContent` sequences.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>

use crate::error::ErrorSink;
use crate::generated_traversal::{Absence, Never, NodeSlot};
use crate::model::UtteranceContent;
use crate::parser::typed_cst::ExtractCanonical;
use talkbank_model::ParseOutcome;

use crate::generated_traversal::{
    AsRawNode, ContentItemChoice, ContentItemChoiceBoundView, ContentItemNode,
    ContentsChild0Choice, ContentsChild0ChoiceBoundView, ContentsChild0MissingSourceView,
    ContentsChild1Choice, ContentsChild1ChoiceBoundView, ContentsChild1MissingSourceView,
    ContentsChildren, ContentsNode, KindAdmitted, OverlapPointNode, SeparatorNode,
    SourceBindingError, SourceBound, SourceBoundKind, SourceChildren, SourceField, SourceRecovery,
    SourceSlotView,
};
use crate::parser::tree_parsing::parser_helpers::check_not_missing;
use crate::parser::typed_cst::{read_source_field, report_admission, report_cst_failure};

use super::super::super::parser_helpers::parse_separator_node;
use super::super::content::{
    MainTierRegion, classify_main_tier_recovery, illegal_curly_quote_error, misplaced_linker_error,
    parse_overlap_point, surface_main_tier_sink,
};

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

/// A `contents` position's generated missing view under the admitted kind
/// proof, read as the leaf a present child of its kind would be, so a MISSING
/// overlap point keeps its kind (the policy `separator.rs` measured and
/// records). The view still names `content_item` and `separator`, but those
/// are rules, which tree-sitter never inserts MISSING, and the compiled
/// grammar proves it: under `KindAdmitted` their payload is the uninhabited
/// `Never`, so their arms below are empty matches rather than code. One
/// exhaustive conversion per position, so a kind added to a position's heads
/// fails to compile here instead of falling through.
trait ContentsPlaceholder<'value, 'tree, 'source> {
    /// The leaf, its zero-width range admitted like a present child's.
    fn read_leaf(self) -> Result<ContentsLeaf<'tree, 'source>, SourceBindingError>;
}

impl<'value, 'tree, 'source> ContentsPlaceholder<'value, 'tree, 'source>
    for ContentsChild0MissingSourceView<'value, 'tree, 'source, KindAdmitted>
{
    fn read_leaf(self) -> Result<ContentsLeaf<'tree, 'source>, SourceBindingError> {
        Ok(match self {
            Self::Whitespaces(_) => ContentsLeaf::Whitespace,
            Self::ContentItem(item) => match item.0 {},
            Self::Separator(separator) => match separator.0 {},
            Self::OverlapPoint(overlap) => ContentsLeaf::OverlapPoint(overlap.read()?),
        })
    }
}

impl<'value, 'tree, 'source> ContentsPlaceholder<'value, 'tree, 'source>
    for ContentsChild1MissingSourceView<'value, 'tree, 'source, KindAdmitted>
{
    fn read_leaf(self) -> Result<ContentsLeaf<'tree, 'source>, SourceBindingError> {
        Ok(match self {
            Self::Whitespaces(_) => ContentsLeaf::Whitespace,
            Self::ContentItem(item) => match item.0 {},
            Self::Separator(separator) => match separator.0 {},
            Self::OverlapPoint(overlap) => ContentsLeaf::OverlapPoint(overlap.read()?),
        })
    }
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
) -> Result<Vec<UtteranceContent>, crate::CstFailure> {
    // Admitted extraction: the compiled grammar's proof that `content_item`
    // and `separator` are never MISSING is what empties their missing arms.
    // The failure is a grammar-binding fault or the proof's refusal of a
    // contradicting tree; like every other fault in the tier body it
    // propagates, so the caller rejects the tier rather than lowering it
    // with invented empty content.
    let children = typed.extract_canonical()?;
    Ok(parse_contents(&children, errors))
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
    contents: &SourceChildren<'tree, '_, ContentsChildren<'tree, KindAdmitted>>,
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
/// MISSING placeholder is dispatched on its typed missing view (since
/// 2026-10-20 the generator types a choice's MISSING head by kind), and both
/// go down one path, the precedent `separator.rs` set and explained.
fn process_contents_slot<'value, 'tree, 'source, C, M, A>(
    slot: SourceField<'value, 'tree, 'source, NodeSlot<C, M, tree_sitter::Node<'tree>, Never, A>>,
    errors: &impl ErrorSink,
    content: &mut Vec<UtteranceContent>,
) where
    C: ContentsItem<'tree>,
    M: SourceRecovery<'value, 'tree, 'source>,
    M::View: ContentsPlaceholder<'value, 'tree, 'source> + AsRawNode<'tree>,
    A: Absence,
{
    let source = slot.source();
    let leaf = match slot.view() {
        // The choice enum names which of the four kinds this is; nothing here
        // reads `node.kind()`.
        SourceSlotView::Present(item) => {
            let Some(item) = read_source_field(item, errors) else {
                return;
            };
            C::leaf(item)
        }
        // A zero-width MISSING placeholder: reported here (E342; the
        // whole-tree backstop's same-code duplicate is suppressed, and the
        // backstop can drop a candidate another diagnostic covers, so this
        // position does not rely on it), then read as the leaf of its kind.
        // A MISSING `whitespaces` contributes nothing, like a present one;
        // until 2026-10-20 it was reported as an "unexpected node" instead.
        SourceSlotView::Missing(placeholder) => {
            let node = placeholder.raw_node();
            check_not_missing(node, source, errors, "contents");
            match placeholder.read_leaf() {
                Ok(leaf) => leaf,
                Err(error) => {
                    report_cst_failure(node, source, error, errors);
                    return;
                }
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
            return;
        }
        // Selected choices cannot produce Unexpected; producer inconsistencies
        // return CstFailure before content construction.
        SourceSlotView::Unexpected(never) => match never {},
        // Only `child_0` has this state (a repeat element never records
        // `Absent`). It is not a MISSING `contents` node, which tree-sitter
        // cannot produce (MISSING nodes are terminals and `contents` is a
        // rule); it would be a walk that left the required first position
        // empty. It is classified with the coverage drive's batch 5 (the
        // main-tier batch in the drive plan); until then it is left as it was.
        SourceSlotView::Absent(_) => return,
    };
    push_leaf(leaf, errors, content);
}

/// Parse one `contents` leaf and push what it yields.
fn push_leaf(
    leaf: ContentsLeaf<'_, '_>,
    errors: &impl ErrorSink,
    content: &mut Vec<UtteranceContent>,
) {
    let parsed = match leaf {
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
    // Admitted, so the five rule kinds of the missing view below are
    // uninhabited types rather than a claim in the comment on its arm.
    let Ok(children) =
        report_admission(typed.extract_canonical(), typed.raw_node(), source, errors)
    else {
        return ParseOutcome::rejected();
    };
    let outcome = match children.field_content().slot().view() {
        SourceSlotView::Present(choice) => match read_source_field(choice, errors) {
            Some(choice) => parse_content_item_choice(choice, errors),
            None => ParseOutcome::rejected(),
        },
        // A MISSING head: tree-sitter inserts MISSING nodes only for
        // terminals, and under the admitted proof the view's rule kinds are
        // uninhabited, so what arrives is a linker token or an illegal curly
        // quote. Neither is content the author wrote, so it yields no
        // model element; it is reported as the missing element it is (E342,
        // the backstop's duplicate suppressed). Until 2026-10-20 the raw
        // placeholder was re-classified and parsed like a present item, which
        // reported E766 ("Linker '' must be utterance-initial"), E233 or E256
        // for a token that is not in the source.
        SourceSlotView::Missing(placeholder) => {
            check_not_missing(placeholder.raw_node(), source, errors, "content item");
            ParseOutcome::rejected()
        }
        SourceSlotView::Error(error_node) => {
            errors.report(classify_main_tier_recovery(
                error_node.raw_node(),
                source,
                MainTierRegion::Body,
            ));
            ParseOutcome::rejected()
        }
        SourceSlotView::Unexpected(never) => match never {},
        // A `content_item` whose required content the walk left empty. It is
        // not a MISSING `content_item` (a rule, never inserted MISSING); it is
        // classified with the drive's batch 5 and carries nothing to parse.
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
