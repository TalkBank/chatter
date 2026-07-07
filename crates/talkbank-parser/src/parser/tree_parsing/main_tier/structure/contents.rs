//! Parse `contents` subtrees into `UtteranceContent` sequences.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::UtteranceContent;
use crate::node_types::{
    CA_CONTINUATION_MARKER, CA_NO_BREAK, CA_TECHNICAL_BREAK, COLON, COMMA, CONTENT_ITEM,
    FALLING_TO_LOW, FALLING_TO_MID, LEVEL_PITCH, NON_COLON_SEPARATOR, OVERLAP_POINT,
    RISING_TO_HIGH, RISING_TO_MID, SEMICOLON, SEPARATOR, TAG_MARKER, UNMARKED_ENDING,
    UPTAKE_SYMBOL, VOCATIVE_MARKER, WHITESPACES,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use crate::generated_traversal::{
    AsRawNode, ContentsChild0Choice, ContentsChild1Choice, ContentsNode, NodeSlot, extract_contents,
};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;

use super::super::super::parser_helpers::parse_separator_like;
use super::super::content::{analyze_word_error, illegal_curly_quote_error, parse_overlap_point};
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
    /// The item's raw node, or `None` for the `whitespaces` alternative (the
    /// NEW backend models whitespace as an explicit choice member since it
    /// does not use `--skip whitespaces`; skipping it here subsumes the OLD
    /// `WHITESPACES => continue` arm).
    fn item_node(&self) -> Option<tree_sitter::Node<'tree>>;
}
impl<'tree> ContentsItem<'tree> for ContentsChild0Choice<'tree> {
    fn item_node(&self) -> Option<tree_sitter::Node<'tree>> {
        match self {
            Self::Whitespaces(_) => None,
            Self::ContentItem(n) => Some(n.raw_node()),
            Self::Separator(n) => Some(n.raw_node()),
            Self::OverlapPoint(n) => Some(n.raw_node()),
        }
    }
}
impl<'tree> ContentsItem<'tree> for ContentsChild1Choice<'tree> {
    fn item_node(&self) -> Option<tree_sitter::Node<'tree>> {
        match self {
            Self::Whitespaces(_) => None,
            Self::ContentItem(n) => Some(n.raw_node()),
            Self::Separator(n) => Some(n.raw_node()),
            Self::OverlapPoint(n) => Some(n.raw_node()),
        }
    }
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
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<UtteranceContent> {
    // `content` starts empty rather than pre-sized to `node.child_count()`: that
    // count includes the (now explicit) `whitespaces` children, so on a normal
    // whitespace-separated utterance it over-allocates by roughly 2x. Utterances
    // are short, so the one or two reallocations a growing `Vec` costs are cheaper
    // than a guaranteed 2x over-allocation and leave no wasted capacity.
    let mut content = Vec::new();

    let contents = extract_contents(ContentsNode(node));
    process_contents_slot(contents.child_0.slot, source, errors, &mut content);
    for element in contents.child_1.slot {
        process_contents_slot(element.slot, source, errors, &mut content);
    }
    surface_unexpected(&contents.unexpected, source, errors);

    content
}

/// Process one `contents` position's slot (either the required `child_0` or one
/// element of the repeated `child_1` tail) into `content`.
///
/// The per-slot body is byte-identical to the OLD backend's single loop body
/// (see the removed `extract_contents_iter` version this replaces): the SAME
/// four arms, just driven by an exhaustive `NodeSlot` match instead of an
/// iterator yield.
fn process_contents_slot<'tree, C: ContentsItem<'tree>>(
    slot: NodeSlot<'tree, C>,
    source: &str,
    errors: &impl ErrorSink,
    content: &mut Vec<UtteranceContent>,
) {
    match slot {
        // A classified `content_item` / `separator` / `overlap_point` (or a
        // `whitespaces` item, skipped: `item_node()` returns `None`). The
        // pre-migration code routed ALL THREE non-whitespace alternatives
        // through `parse_content_item` with byte-identical bodies. The old
        // kind() dispatch never checked `is_missing`, so a MISSING node (whose
        // kind is still one of the alternatives) followed the same path; we
        // keep that by treating a `Missing` raw node the same as a `Present`
        // non-whitespace item's raw node (the NEW backend's `Missing` variant
        // carries no choice classification to check for `whitespaces` against,
        // matching the OLD "treat Missing like Present" precedent as closely as
        // the closed `NodeSlot` shape allows). `Present` is the valid path;
        // `Missing` is recovery-only (a childless MISSING node yields a
        // rejected `parse_content_item` and pushes nothing).
        NodeSlot::Present(item) => {
            if let Some(item_node) = item.item_node()
                && let ParseOutcome::Parsed(parsed) = parse_content_item(item_node, source, errors)
            {
                content.push(parsed);
            }
        }
        NodeSlot::Missing(item_node) => {
            if let ParseOutcome::Parsed(parsed) = parse_content_item(item_node, source, errors) {
                content.push(parsed);
            }
        }
        // Parser `ERROR` fragment, reproduced byte-identically from the old
        // `child.is_error()` branch: first try to glue the fragment to the
        // preceding word token; only if that fails report the word-error
        // diagnostic at the exact node span (so the whole-tree recovery
        // backstop, which also covers ERROR nodes, dedups on span).
        NodeSlot::Error(error_node) => {
            if !attach_error_suffix_to_previous_word(error_node, source, content) {
                errors.report(analyze_word_error(error_node, source));
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
        NodeSlot::Absent => {}
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

/// Parse a `content_item` or compatible leaf node into `UtteranceContent`.
///
/// This helper mirrors the grammar alternatives listed in the Main Tier section: base content, groups,
/// quotations, phonology/syntax groups, separators, and overlap points. When the parser emits bare
/// separators or overlap markers directly (without `content_item` wrappers) we still accept them so the
/// model stays faithful to the grammar’s concrete tokens.
/// Parse a `content_item` or compatible leaf node into `UtteranceContent`.
///
/// Mirrors the main tier grammar described in the CHAT manual by handling base content, groups,
/// quotations, phonology/syntax groups, separators, and overlap points. When the tree-sitter parser
/// emits bare separator/overlap tokens directly (without a `content_item` wrapper) we still consume
/// them to ensure the resulting `UtteranceContent` list matches the concrete syntax the manual defines.
fn parse_content_item(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    use super::super::content::{
        parse_base_content, parse_group_content, parse_pho_group_content, parse_quotation_content,
        parse_sin_group_content,
    };
    use crate::node_types::{
        BASE_CONTENT_ITEM, GROUP_WITH_ANNOTATIONS, ILLEGAL_CURLY_QUOTE, MAIN_PHO_GROUP,
        MAIN_SIN_GROUP, QUOTATION,
    };

    // CRITICAL FIX: Handle the node itself if it's a leaf node (e.g., bare COLON, SEPARATOR)
    // This is needed because the serializer outputs canonical spacing like "⌈2 :" where
    // the colon appears as a bare child of contents, not wrapped in a content_item node.
    match node.kind() {
        SEPARATOR => {
            if let ParseOutcome::Parsed(sep) = parse_separator_like(node, source, errors) {
                return ParseOutcome::parsed(UtteranceContent::Separator(sep));
            }
            return ParseOutcome::rejected();
        }
        NON_COLON_SEPARATOR
        | COLON
        | COMMA
        | SEMICOLON
        | TAG_MARKER
        | VOCATIVE_MARKER
        | CA_CONTINUATION_MARKER
        | UNMARKED_ENDING
        | UPTAKE_SYMBOL
        | CA_NO_BREAK
        | CA_TECHNICAL_BREAK
        | RISING_TO_HIGH
        | RISING_TO_MID
        | LEVEL_PITCH
        | FALLING_TO_MID
        | FALLING_TO_LOW => {
            if let ParseOutcome::Parsed(sep) = parse_separator_like(node, source, errors) {
                return ParseOutcome::parsed(UtteranceContent::Separator(sep));
            }
            return ParseOutcome::rejected();
        }
        OVERLAP_POINT => {
            return parse_overlap_point(node, source, errors);
        }
        BASE_CONTENT_ITEM => return parse_base_content(node, source, errors),
        GROUP_WITH_ANNOTATIONS => return parse_group_content(node, source, errors),
        MAIN_PHO_GROUP => return parse_pho_group_content(node, source, errors),
        MAIN_SIN_GROUP => return parse_sin_group_content(node, source, errors),
        QUOTATION => return parse_quotation_content(node, source, errors),
        ILLEGAL_CURLY_QUOTE => {
            // Recognized illegal curly single quote: report E256 and reject
            // (no model element). The surrounding words are separate content
            // items and parse normally.
            errors.report(illegal_curly_quote_error(node, source));
            return ParseOutcome::rejected();
        }
        // content_item is a supertype wrapper, fall through to iterate its children below
        CONTENT_ITEM => {}
        _ => {
            errors.report(unexpected_node_error(node, source, "content item"));
            return ParseOutcome::rejected();
        }
    }

    // If not a leaf node, iterate over children
    let child_count = node.child_count();

    for idx in 0..child_count {
        let Some(child) = node.child(idx as u32) else {
            continue;
        };

        if child.is_error() {
            errors.report(analyze_word_error(child, source));
            return ParseOutcome::rejected();
        }

        match child.kind() {
            BASE_CONTENT_ITEM => return parse_base_content(child, source, errors),
            GROUP_WITH_ANNOTATIONS => return parse_group_content(child, source, errors),
            MAIN_PHO_GROUP => return parse_pho_group_content(child, source, errors),
            MAIN_SIN_GROUP => return parse_sin_group_content(child, source, errors),
            QUOTATION => return parse_quotation_content(child, source, errors),
            ILLEGAL_CURLY_QUOTE => {
                // Recognized illegal curly single quote inside a content_item
                // wrapper: report E256 and reject (no model element).
                errors.report(illegal_curly_quote_error(child, source));
                return ParseOutcome::rejected();
            }
            OVERLAP_POINT => {
                return parse_overlap_point(child, source, errors);
            }
            SEPARATOR => {
                if let ParseOutcome::Parsed(sep) = parse_separator_like(child, source, errors) {
                    return ParseOutcome::parsed(UtteranceContent::Separator(sep));
                }
                return ParseOutcome::rejected();
            }
            NON_COLON_SEPARATOR
            | COLON
            | COMMA
            | SEMICOLON
            | TAG_MARKER
            | VOCATIVE_MARKER
            | CA_CONTINUATION_MARKER
            | UNMARKED_ENDING
            | UPTAKE_SYMBOL
            | RISING_TO_HIGH
            | RISING_TO_MID
            | LEVEL_PITCH
            | FALLING_TO_MID
            | FALLING_TO_LOW => {
                if let ParseOutcome::Parsed(sep) = parse_separator_like(child, source, errors) {
                    return ParseOutcome::parsed(UtteranceContent::Separator(sep));
                }
                return ParseOutcome::rejected();
            }
            WHITESPACES => continue,
            _ => {
                errors.report(unexpected_node_error(child, source, "content item child"));
                return ParseOutcome::rejected();
            }
        }
    }

    ParseOutcome::rejected()
}
