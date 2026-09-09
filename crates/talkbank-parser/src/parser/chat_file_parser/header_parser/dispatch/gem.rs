//! Per-kind parsing for GEM-style headers (`@Bg`, `@Eg`, `@G`).
//!
//! Each function here is the LEVEL-1 entry for one `HeaderChoice` GEM variant.
//! The `Header::BeginGem` / `EndGem` / `LazyGem` construction is unchanged
//! from the pre-migration string-dispatch arms; the label is read only from
//! the typed `free_text` position (see `parse_gem_label`).
//!
//! LEVEL 2: the optional `free_text` content child is read through the NEW
//! backend's free, typed `extract_<kind>(node)` (replacing the local
//! `find_child_by_kind(header_actual, FREE_TEXT)` scan, and, as of Task B2, the
//! OLD `TypedTraversal.extract_<kind>` trait-receiver call). The NEW backend does
//! not skip whitespace, so `bg_header`/`eg_header` no longer expose the
//! `header_sep` + `free_text` pair as two flat OPTIONAL positions the way the OLD
//! module's `child_2: Option<NodeSlot<FreeTextNode>>` did; they are grouped into
//! ONE optional GROUP position (`child_1: Option<NodeSlot<BgHeaderChild1Children>>`
//! / `EgHeaderChild1Children`, each `{ child_0: header_sep, child_1: free_text }`)
//! because the whole `seq(header_sep, free_text)` pair is what is optional at the
//! grammar level, not `free_text` alone. `g_header` is unaffected (its `free_text`
//! is grammar-REQUIRED, so it stays a flat `child_2: &NodeSlot<FreeTextNode>`, same
//! index as the OLD module). In every case the slot is mapped to the
//! `Option<FreeTextNode>` that `parse_gem_label` / `parse_optional_gem_label`
//! takes, exhaustively, with no `_ =>` arm that silently drops variants:
//! `Present` hands over the typed node (for `Bg`/`Eg`, reached by descending one
//! level into the group's own `child_1`, i.e. the `free_text` member of the
//! pair); every recovery state maps to `None`, no label, with the whole-tree
//! pass naming the recovery (`gem_headers_from_source.rs` pins the shapes).
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
    // LEVEL 2: read the required free_text child through the typed positional slot
    // (extract_g_header child_2: &NodeSlot<FreeTextNode>, the SAME index as the OLD
    // module: g_header's free_text is grammar-required, so there is no optional
    // group to descend into here). `present_or_recover().ok()` maps Present to the
    // typed node and every recovery variant to None (a MISSING or ERROR free_text
    // is the whole-tree pass's to name), the option `parse_gem_label` takes.
    let children = extract_g_header(typed);
    let free_text_child = children.child_2.slot().clone().present_or_recover().ok();
    let outcome = match parse_gem_label(free_text_child, input, errors) {
        ParseOutcome::Parsed(label) => ParseOutcome::parsed(Header::LazyGem { label }),
        ParseOutcome::Rejected => ParseOutcome::rejected(),
    };
    surface_displaced(&children.unexpected, "g_header", input, errors);
    outcome
}
