//! Per-kind parsing for GEM-style headers (`@Bg`, `@Eg`, `@G`).
//!
//! Each function here is the LEVEL-1 entry for one `HeaderChoice` GEM variant.
//! The `Header::BeginGem` / `EndGem` / `LazyGem` construction is unchanged
//! from the pre-migration string-dispatch arms; the label is read only from
//! the typed `free_text` position (see `parse_gem_label`).
//!
//! All three kinds permit bare markers. Their generated traversal represents
//! the optional separator/label pair as one group, not independently optional
//! fields. Labels come only from that typed group's free-text position.
//! Missing/error slots remain recovery states; the whole-tree diagnostic pass
//! is retained, and displaced children are surfaced at both group and header
//! boundaries (`gem_headers_from_source.rs` pins recovery shapes).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    BgHeaderNode, EgHeaderNode, FreeTextNode, GHeaderNode, extract_bg_header, extract_eg_header,
    extract_g_header,
};
use crate::model::{self, Header};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use talkbank_model::ParseOutcome;

use super::super::helpers::parse_optional_gem_label;
use crate::parser::tree_parsing::parser_helpers::present;

/// The GEM label a header's typed free-text position carries, or none.
///
/// Until 2026-09-09 a header whose typed read found no label fell back to
/// slicing the header's raw text after its first colon, which is the
/// hand-parse of CHAT text the repository bans: the typed position is the
/// only read now, and a header whose free text did not parse has no label,
/// with the whole-tree pass naming the recovery.
fn parse_gem_label(
    free_text_child: Option<FreeTextNode<'_>>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<model::GemLabel>> {
    parse_optional_gem_label(free_text_child, input, errors)
}

/// `@Bg` -> `Header::BeginGem` (always, even when malformed).
///
/// A `@Bg` input NEVER produces `Header::LazyGem`; only `@G` does. An empty or
/// malformed label (e.g. `@Bg:` with no label text) yields `BeginGem { label: None }`.
/// Any parse-time diagnostic for the malformed input (E316 for an unparsable `:`,
/// E342 for a missing required element) is emitted by the tree-sitter backstop and
/// is orthogonal to the model kind.
pub(super) fn bg(
    typed: BgHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // LEVEL 2: read the optional `seq(header_sep, free_text)` GROUP through the
    // typed positional slot (extract_bg_header child_1:
    // Option<NodeSlot<BgHeaderChild1Children>>), then descend one level into the
    // group's own `child_1` (the free_text member) via `present_or_recover().ok()`
    // at each level. Every non-Present state at EITHER level (and the outer None,
    // meaning the whole group is grammar-absent) collapses to None, the option of
    // typed `FreeTextNode` that `parse_gem_label` takes; a recovery state at either
    // level is the whole-tree pass's to name.
    let children = extract_bg_header(typed);
    let group = children
        .child_1
        .slot()
        .clone()
        .and_then(|s| s.present_or_recover().ok());
    let free_text_child = group
        .as_ref()
        .and_then(|group| present(group.child_1.slot()))
        .copied();
    let outcome = match parse_gem_label(free_text_child, input, errors) {
        ParseOutcome::Parsed(label) => ParseOutcome::parsed(Header::BeginGem { label }),
        ParseOutcome::Rejected => ParseOutcome::rejected(),
    };
    if let Some(group) = &group {
        surface_displaced(&group.unexpected, "bg_header", input, errors);
    }
    surface_displaced(&children.unexpected, "bg_header", input, errors);
    outcome
}

/// `@Eg` -> `Header::EndGem`.
pub(super) fn eg(
    typed: EgHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // Same optional-GROUP shape as `bg` (see its comment); `eg_header`'s pair is
    // `EgHeaderChild1Children { child_0: header_sep, child_1: free_text }`.
    let children = extract_eg_header(typed);
    let group = children
        .child_1
        .slot()
        .clone()
        .and_then(|s| s.present_or_recover().ok());
    let free_text_child = group
        .as_ref()
        .and_then(|group| present(group.child_1.slot()))
        .copied();
    let outcome = match parse_gem_label(free_text_child, input, errors) {
        ParseOutcome::Parsed(label) => ParseOutcome::parsed(Header::EndGem { label }),
        ParseOutcome::Rejected => ParseOutcome::rejected(),
    };
    if let Some(group) = &group {
        surface_displaced(&group.unexpected, "eg_header", input, errors);
    }
    surface_displaced(&children.unexpected, "eg_header", input, errors);
    outcome
}

/// `@G` -> `Header::LazyGem`.
pub(super) fn g(
    typed: GHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_g_header(typed);
    let group = children
        .child_1
        .slot()
        .clone()
        .and_then(|s| s.present_or_recover().ok());
    let free_text_child = group
        .as_ref()
        .and_then(|group| present(group.child_1.slot()))
        .copied();
    let outcome = match parse_gem_label(free_text_child, input, errors) {
        ParseOutcome::Parsed(label) => ParseOutcome::parsed(Header::LazyGem { label }),
        ParseOutcome::Rejected => ParseOutcome::rejected(),
    };
    if let Some(group) = &group {
        surface_displaced(&group.unexpected, "g_header", input, errors);
    }
    surface_displaced(&children.unexpected, "g_header", input, errors);
    outcome
}
