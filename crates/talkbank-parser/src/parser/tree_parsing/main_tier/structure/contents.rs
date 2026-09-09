//! Parse `contents` subtrees into `UtteranceContent` sequences.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::UtteranceContent;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use crate::generated_traversal::{
    AsRawNode, BaseContentItemNode, ChoiceSlot, ContentItemCaNoBreakLinkerChoice,
    ContentItemChoice, ContentItemNode, ContentsChild0Choice, ContentsChild1Choice,
    ContentsChildren, ContentsNode, FromNodeKind, GroupWithAnnotationsNode, IllegalCurlyQuoteNode,
    MainPhoGroupNode, MainSinGroupNode, NoChild, NodeSlot, OverlapPointNode,
    QuotationWithOptionalAnnotationsNode, SeparatorNode, extract_content_item, extract_contents,
};

use super::super::super::parser_helpers::parse_separator_like;
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
trait ContentsItem<'tree> {
    /// Which of the four alternatives this item is, with its raw node.
    ///
    /// The choice enum the generator emits already proves the kind, so the
    /// per-item processing dispatches on this and never re-reads
    /// `node.kind()`: a `contents` child is one of exactly these four, and a
    /// match with no other arm is the grammar's own statement of that.
    fn leaf(&self) -> ContentsLeaf<'tree>;
}

/// The four things a `contents` child can be, carrying the raw node the
/// per-kind parser takes.
enum ContentsLeaf<'tree> {
    /// Whitespace between items; contributes nothing.
    Whitespace,
    /// A `content_item` wrapper around a word, group, quotation or the like.
    ContentItem(tree_sitter::Node<'tree>),
    /// A bare separator token, which the grammar places directly under
    /// `contents` (a colon after an overlap marker, for one).
    Separator(tree_sitter::Node<'tree>),
    /// A bare overlap marker, likewise a direct child.
    OverlapPoint(tree_sitter::Node<'tree>),
}

impl<'tree> ContentsItem<'tree> for ContentsChild0Choice<'tree> {
    fn leaf(&self) -> ContentsLeaf<'tree> {
        match self {
            Self::Whitespaces(_) => ContentsLeaf::Whitespace,
            Self::ContentItem(n) => ContentsLeaf::ContentItem(n.raw_node()),
            Self::Separator(n) => ContentsLeaf::Separator(n.raw_node()),
            Self::OverlapPoint(n) => ContentsLeaf::OverlapPoint(n.raw_node()),
        }
    }
}

impl<'tree> ContentsItem<'tree> for ContentsChild1Choice<'tree> {
    fn leaf(&self) -> ContentsLeaf<'tree> {
        match self {
            Self::Whitespaces(_) => ContentsLeaf::Whitespace,
            Self::ContentItem(n) => ContentsLeaf::ContentItem(n.raw_node()),
            Self::Separator(n) => ContentsLeaf::Separator(n.raw_node()),
            Self::OverlapPoint(n) => ContentsLeaf::OverlapPoint(n.raw_node()),
        }
    }
}

/// Where a `contents` node sits, which decides one thing: whether an ERROR
/// fragment at its start can be "an annotation with nothing to attach to"
/// (E759, CLAN CHECK 52). That is a fact about the utterance's first item,
/// so it holds only for the tier body; inside a bracketed construct the
/// same fragment is ordinary broken content. Everything else the walker
/// does is the same in both places: the grammar rule is one rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContentsRegion {
    /// The `contents` of a `tier_body`: the utterance's own words.
    UtteranceBody,
    /// The `contents` inside `< >`, a quotation, a pho or a sin group.
    InsideBrackets,
}

/// Parse main-tier `contents` nodes into ordered `UtteranceContent` items.
///
/// The `contents` rule (`repeat1(choice(whitespaces, content_item, separator, overlap_point))`)
/// collects words, separators, overlap markers, and other inline tokens described in the Main Tier
/// section of the manual. Iteration is driven by the generated typed visitor: one
/// [`extract_contents`] call yields the required first element (`child_0`) plus the repeated tail
/// (`child_1`, a `Vec`), each a [`NodeSlot`] over its own per-position choice enum
/// ([`ContentsChild0Choice`] / [`ContentsChild1Choice`]), so structure comes from typed node
/// dispatch rather than `node.kind()` string matching, and a recovery node can never be silently
/// dropped. Unlike the OLD backend's lazy `extract_contents_iter`, the NEW backend has no iterator
/// form (every migrated cluster in this workstream materializes its repeats eagerly, per the B1
/// template), so the `Vec` for `child_1` is fully built before this function iterates it; this is a
/// deliberate, accepted architectural property of the NEW backend, not a generator gap. Each
/// concrete choice is handed to the existing [`parse_content_item`] (its internals migrate in a
/// separate task). When we encounter parser `ERROR` fragments (common around overlapped markers
/// such as `⌈2`), we attempt to glue them to the preceding word token so the resulting
/// `UtteranceContent` still matches the manual’s lookahead expectations.
pub fn parse_main_tier_contents(
    typed: ContentsNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<UtteranceContent> {
    parse_contents(
        &extract_contents(typed),
        ContentsRegion::UtteranceBody,
        source,
        errors,
    )
}

/// Parse an already-extracted `contents` node, wherever it sits.
///
/// The bracketed constructs (angle group, quotation, pho and sin groups)
/// extract their `contents` slot themselves, because the angle group reads
/// its edge whitespace off the same extraction for E750, and hand the
/// children here. Until 2026-09-08 each of them walked `contents` by hand
/// through a second, `node.kind()`-driven copy of this dispatch
/// (`group/nested.rs`), with its own arms for every separator kind the
/// grammar never places there.
pub(crate) fn parse_contents(
    contents: &ContentsChildren<'_>,
    region: ContentsRegion,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<UtteranceContent> {
    // `content` starts empty rather than pre-sized to the child count: that
    // count includes the (now explicit) `whitespaces` children, so on a normal
    // whitespace-separated utterance it over-allocates by roughly 2x. Utterances
    // are short, so the one or two reallocations a growing `Vec` costs are cheaper
    // than a guaranteed 2x over-allocation and leave no wasted capacity.
    let mut content = Vec::new();
    process_contents_slot(
        contents.child_0.slot(),
        region,
        source,
        errors,
        &mut content,
    );
    for element in contents.child_1.slot() {
        process_contents_slot(element.slot(), region, source, errors, &mut content);
    }
    surface_main_tier_sink(
        &contents.unexpected,
        MainTierRegion::Body,
        "contents",
        source,
        errors,
    );
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
fn process_contents_slot<'tree, C: ContentsItem<'tree>>(
    slot: &ChoiceSlot<'tree, C>,
    region: ContentsRegion,
    source: &str,
    errors: &impl ErrorSink,
    content: &mut Vec<UtteranceContent>,
) {
    match slot {
        // The choice enum names which of the four kinds this is; nothing here
        // reads `node.kind()`.
        NodeSlot::Present(item) => {
            let parsed = match item.leaf() {
                ContentsLeaf::Whitespace => return,
                ContentsLeaf::Separator(node) => {
                    parse_separator_like(node, source, errors).map(UtteranceContent::Separator)
                }
                ContentsLeaf::OverlapPoint(node) => parse_overlap_point(node, source, errors),
                ContentsLeaf::ContentItem(node) => match ContentItemNode::from_node(node) {
                    Some(item) => parse_content_item(item, source, errors),
                    // The choice enum already proved the kind; a refusal here
                    // would mean the generator disagrees with itself.
                    None => {
                        errors.report(unexpected_node_error(node, source, "content item"));
                        ParseOutcome::rejected()
                    }
                },
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
        NodeSlot::Missing(item_node) => {
            let node = *item_node;
            let parsed = if SeparatorNode::from_node(node).is_some() {
                parse_separator_like(node, source, errors).map(UtteranceContent::Separator)
            } else if OverlapPointNode::from_node(node).is_some() {
                parse_overlap_point(node, source, errors)
            } else if let Some(item) = ContentItemNode::from_node(node) {
                parse_content_item(item, source, errors)
            } else {
                errors.report(unexpected_node_error(node, source, "content item"));
                ParseOutcome::rejected()
            };
            if let ParseOutcome::Parsed(parsed) = parsed {
                content.push(parsed);
            }
        }
        // Parser `ERROR` fragment, reproduced byte-identically from the old
        // `child.is_error()` branch: first try to glue the fragment to the
        // preceding word token; only if that fails report the word-error
        // diagnostic at the exact node span (so the whole-tree recovery
        // backstop, which also covers ERROR nodes, dedups on span).
        NodeSlot::Error(error_node) => {
            // E759: an ERROR fragment that is the FIRST content item and has
            // the shape of a postfix annotation (retrace / overlap /
            // replacement / quotation code) is an annotation with nothing to
            // attach to (CLAN CHECK 52). The emptiness of `content` is the
            // typed leading-position signal; mid-utterance broken codes fall
            // through to the ordinary word-error analysis.
            // A fragment whose bytes are not UTF-8 has no readable leading
            // annotation; the whole-tree backstop reports the node itself.
            let leading_annotation = if region == ContentsRegion::UtteranceBody
                && content.is_empty()
                && let Ok(fragment) = error_node.utf8_text(source.as_bytes())
            {
                crate::parser::tree_parsing::parser_helpers::error_analysis::dedicated::leading_postfix_annotation(
                    fragment.trim_start(),
                )
                .map(|code_token| (code_token.to_string(), fragment.to_string()))
            } else {
                None
            };
            if let Some((code_token, fragment)) = leading_annotation {
                errors.report(
                    crate::parser::tree_parsing::parser_helpers::error_analysis::dedicated::annotation_at_utterance_start(
                        &code_token,
                        crate::error::SourceLocation::from_offsets(
                            error_node.start_byte(),
                            error_node.end_byte(),
                        ),
                        crate::error::ErrorContext::new(
                            source,
                            error_node.start_byte()..error_node.end_byte(),
                            &fragment,
                        ),
                    ),
                );
            } else if !attach_error_suffix_to_previous_word(*error_node, source, content) {
                errors.report(classify_main_tier_recovery(
                    *error_node,
                    source,
                    MainTierRegion::Body,
                ));
            }
        }
        // A child whose kind is none of the `contents` alternatives. On valid
        // CHAT this is unreachable: the grammar's `contents` rule yields only
        // `whitespaces` / `content_item` / `separator` / `overlap_point`, so a
        // non-matching kind can arrive only via error recovery, which wraps
        // stray tokens in `ERROR` nodes (handled above). Reproduce the old
        // catch-all's structural diagnostic verbatim. (The old leaf-fallback
        // also listed bare separator leaves such as `colon`/`comma`, but the
        // grammar never emits those directly under `contents`; were one to
        // surface via recovery, flagging it as unexpected is a sanctioned
        // malformed-only improvement over the old silent accept, and routing it
        // back through kind() dispatch is the very anti-pattern this migration
        // removes.)
        NodeSlot::Unexpected(unexpected_node) => {
            errors.report(ParseError::new(
                ErrorCode::StructuralOrderError,
                Severity::Error,
                SourceLocation::from_offsets(
                    unexpected_node.start_byte(),
                    unexpected_node.end_byte(),
                ),
                ErrorContext::new(
                    source,
                    unexpected_node.start_byte()..unexpected_node.end_byte(),
                    "",
                ),
                format!("Unexpected '{}' in contents", unexpected_node.kind()),
            ));
        }
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
        NodeSlot::Absent(NoChild) => {}
    }
}

/// Attach compact error fragments to the previous word token when the parser emits a split marker.
///
/// Tree-sitter sometimes splits tokens such as `@x` into a word plus a trailing `ERROR` node. When the
/// fragment looks like part of the originating word, we append it so downstream tools reproduce the
/// manual’s tokens exactly and avoid duplicate diagnostics.
fn attach_error_suffix_to_previous_word(
    error_node: Node,
    source: &str,
    content: &mut [UtteranceContent],
) -> bool {
    let Ok(error_text) = error_node.utf8_text(source.as_bytes()) else {
        return false;
    };

    let Some(last) = content.last_mut() else {
        return false;
    };

    match last {
        UtteranceContent::Word(word)
            if should_attach_error_fragment(word.raw_text(), error_text) =>
        {
            let new_raw = format!("{}{}", word.raw_text(), error_text);
            word.set_raw_text(new_raw);
            true
        }
        UtteranceContent::AnnotatedWord(annotated)
            if should_attach_error_fragment(annotated.inner.raw_text(), error_text) =>
        {
            let new_raw = format!("{}{}", annotated.inner.raw_text(), error_text);
            annotated.inner.set_raw_text(new_raw);
            true
        }
        UtteranceContent::ReplacedWord(replaced)
            if should_attach_error_fragment(replaced.word.raw_text(), error_text) =>
        {
            let new_raw = format!("{}{}", replaced.word.raw_text(), error_text);
            replaced.word.set_raw_text(new_raw);
            true
        }
        _ => false,
    }
}

/// Decide whether an `ERROR` fragment should be bound to the preceding word.
///
/// We only attach non-whitespace fragments that either start with `@` or extend an `@`-suffix already
/// present on the word so the parser’s recovery logic stays consistent with CHAT tag notation.
fn should_attach_error_fragment(existing_raw: &str, fragment: &str) -> bool {
    if fragment.is_empty() || fragment.bytes().any(|b| b.is_ascii_whitespace()) {
        return false;
    }

    // Always keep explicit @-suffix fragments attached to the originating word.
    if fragment.starts_with('@') {
        return true;
    }

    // Recovery for split marker tails like hello@x + ERROR("yz").
    existing_raw.contains('@')
        && fragment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b':' | b'+' | b'&' | b'-' | b'_'))
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
fn parse_content_item(
    typed: ContentItemNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let children = extract_content_item(typed);
    let outcome = match children.content.slot() {
        NodeSlot::Present(choice) => parse_content_item_choice(choice, source, errors),
        // A zero-width MISSING placeholder for a whole construct: classified
        // through the typed constructors and parsed like a present one, the
        // precedent `separator.rs` set (the old walk never checked
        // `is_missing` here either).
        NodeSlot::Missing(placeholder) => match content_item_choice_of(*placeholder) {
            Some(choice) => parse_content_item_choice(&choice, source, errors),
            None => {
                errors.report(unexpected_node_error(
                    *placeholder,
                    source,
                    "content item child",
                ));
                ParseOutcome::rejected()
            }
        },
        NodeSlot::Error(error_node) => {
            errors.report(classify_main_tier_recovery(
                *error_node,
                source,
                MainTierRegion::Body,
            ));
            ParseOutcome::rejected()
        }
        NodeSlot::Unexpected(unexpected) => {
            errors.report(unexpected_node_error(
                *unexpected,
                source,
                "content item child",
            ));
            ParseOutcome::rejected()
        }
        // A `content_item` with no child at all, which only a childless
        // MISSING wrapper can be; it carries nothing to parse.
        NodeSlot::Absent(NoChild) => ParseOutcome::rejected(),
    };
    surface_main_tier_sink(
        &children.unexpected,
        MainTierRegion::Body,
        "content_item",
        source,
        errors,
    );
    outcome
}

/// Dispatch one `content_item` alternative to the parser that owns it.
fn parse_content_item_choice(
    choice: &ContentItemChoice<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    use super::super::content::{
        parse_base_content, parse_group_content, parse_pho_group_content,
        parse_quotation_with_annotations_content, parse_sin_group_content,
    };
    match choice {
        ContentItemChoice::BaseContentItem(base) => parse_base_content(*base, source, errors),
        ContentItemChoice::GroupWithAnnotations(group) => {
            parse_group_content(*group, source, errors)
        }
        ContentItemChoice::QuotationWithOptionalAnnotations(quotation) => {
            parse_quotation_with_annotations_content(*quotation, source, errors)
        }
        ContentItemChoice::MainPhoGroup(pho) => parse_pho_group_content(*pho, source, errors),
        ContentItemChoice::MainSinGroup(sin) => parse_sin_group_content(*sin, source, errors),
        // A recognised illegal curly single quote: E256, no model element.
        ContentItemChoice::IllegalCurlyQuote(quote) => {
            errors.report(illegal_curly_quote_error(quote.raw_node(), source));
            ParseOutcome::rejected()
        }
        // A linker in content position. Linkers are utterance-initial by
        // definition; one that reduced here instead of into the tier body's
        // `linkers` field is misplaced: E766, no model element.
        ContentItemChoice::CaNoBreakLinker(linker) => {
            errors.report(misplaced_linker_error(linker.raw_node(), source));
            ParseOutcome::rejected()
        }
    }
}

/// Classify a raw node into the `content_item` alternative of its kind.
///
/// The generator gives the choice enum no `FromNodeKind` (one alternative is
/// itself a supertype choice), so the MISSING arm above asks each wrapper in
/// turn. A node of none of these kinds is refused.
fn content_item_choice_of(node: Node<'_>) -> Option<ContentItemChoice<'_>> {
    if let Some(base) = BaseContentItemNode::from_node(node) {
        Some(ContentItemChoice::BaseContentItem(base))
    } else if let Some(group) = GroupWithAnnotationsNode::from_node(node) {
        Some(ContentItemChoice::GroupWithAnnotations(group))
    } else if let Some(quotation) = QuotationWithOptionalAnnotationsNode::from_node(node) {
        Some(ContentItemChoice::QuotationWithOptionalAnnotations(
            quotation,
        ))
    } else if let Some(quote) = IllegalCurlyQuoteNode::from_node(node) {
        Some(ContentItemChoice::IllegalCurlyQuote(quote))
    } else if let Some(pho) = MainPhoGroupNode::from_node(node) {
        Some(ContentItemChoice::MainPhoGroup(pho))
    } else if let Some(sin) = MainSinGroupNode::from_node(node) {
        Some(ContentItemChoice::MainSinGroup(sin))
    } else {
        ContentItemCaNoBreakLinkerChoice::from_node(node).map(ContentItemChoice::CaNoBreakLinker)
    }
}
