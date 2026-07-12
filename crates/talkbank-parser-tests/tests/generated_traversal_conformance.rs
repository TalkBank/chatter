// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! KEYSTONE generator-conformance test: assert the generated `extract_*` free
//! functions CLASSIFY real CST nodes correctly on the valid reference corpus.
//!
//! The sibling `generated_traversal_parity.rs` already walks the corpus and
//! calls every `extract_*`, but discards the result with `let _ = ...`, so a
//! mis-typed function (e.g. a supertype seq-member guarded on the literal kind
//! that classifies every real child `Unexpected`, or a choice rule whose
//! content alternative is dropped) compiles, round-trips, passes clippy, and
//! RUNS there, yet the wrongness is thrown away. This test KEEPS the result
//! and asserts, on the valid reference corpus, that:
//!   * no slot is `NodeSlot::Unexpected` (anywhere, including inside a
//!     synthetic group or self-classifying choice reached by recursion),
//!   * no REQUIRED (non-`Option`) slot is `Missing` / `Absent` / `Error`,
//!   * no repeat-slot element is `Unexpected`.
//!
//! Optional-absent (`None` / `Some(Missing|Error|Absent)`) is allowed.
//!
//! The per-rule inspection inventory (one no-op `Inspect` impl per leaf node
//! wrapper, one variant-dispatching `Inspect` impl per `*Choice` enum, one
//! field-recursing `Inspect` impl per `*Children` struct, one `dispatch` arm
//! per `extract_*` free function) is MECHANICALLY derived from
//! `crates/talkbank-parser/src/generated_traversal.rs` and lives in the
//! `inventory` submodule; this file holds the stable harness + allowlist.
//! Regenerate the inventory with the committed generator after a visitor regen:
//! `cargo run -p talkbank-parser-tests --example gen_conformance_inventory`
//! (the generator's logic lives in `conformance_inventory::generate_inventory`).
//! The staleness guard `conformance_inventory_is_current`
//! (`tests/conformance_inventory_current.rs`) re-derives the inventory from the
//! current typed traversal + node-types.json and fails if the committed copy has
//! drifted, so a forgotten regen breaks the suite instead of silently losing
//! coverage. After a regen, re-derive the allowlist from a fresh run.
//!
//! ## Why every field is inspected the SAME way regardless of shape
//!
//! The NEW backend wraps every field in `Positioned<'tree, S>` where `S` is
//! one of `NodeSlot<T>` (required), `Option<NodeSlot<T>>` (optional), or
//! `Vec<Positioned<NodeSlot<T>>>` (repeat). `T` itself is always exactly one
//! of: a leaf node wrapper (its own node kind is separately visited by
//! `walk_all`, so nothing to recurse into), a synthetic `*Children` group with
//! no grammar node of its own (a `seq`/optional-group folded into a carrier,
//! e.g. `bg_header`'s optional `(header_sep, free_text)` pair), or a
//! self-classifying `*Choice` enum whose variant payload is, recursively, one
//! of these same three things. `InspectField` (below) dispatches on the
//! *container* shape (required/optional/repeat) exactly once, and recurses
//! into the payload via `Inspect` regardless of which of the three payload
//! kinds it is; `Inspect` itself is what varies per generated type
//! (mechanically, in `inventory.rs`). This closure was verified exhaustive
//! against the live module before generation (every `T` ever used as a
//! `NodeSlot` payload or `Choice` variant payload is one of the three kinds;
//! zero unclassified).

use std::collections::BTreeMap;

use talkbank_parser_tests::generated_traversal::*;

// `#[path]` so the shared inventory module lives in a subdir (NOT a sibling
// `tests/inventory.rs`, which cargo would compile as its own test binary).
#[path = "generated_traversal_conformance/inventory.rs"]
mod inventory;

/// Which faulty `NodeSlot` state a violation records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SlotStatus {
    /// A real node landed at a position whose declared kind it does not match.
    Unexpected,
    /// A required slot held a MISSING placeholder.
    Missing,
    /// A required slot held an ERROR node.
    Error,
    /// A required slot had no node at all (child list too short).
    Absent,
}

/// A bad slot found on the corpus. It serves two roles with identical fields:
/// the raw per-position record produced by inspection, AND the aggregation key of
/// the `BTreeMap` that counts distinct misclassification signatures, so the map
/// keys directly on it (deriving `Ord` for that) with no field-by-field copy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RawViolation {
    /// The rule (node kind) whose `extract_*` produced the bad slot.
    pub(crate) rule_kind: &'static str,
    /// The struct field (slot) that was bad.
    pub(crate) slot: &'static str,
    /// Which faulty state the slot was in.
    pub(crate) status: SlotStatus,
    /// The kind of the offending real node (or a `<missing>`/`<absent>` marker).
    pub(crate) actual_child_kind: String,
}

/// Recursive inspection for a generated type: visit every slot reachable from
/// `self` and record any bad one. Implemented mechanically in `inventory.rs`
/// for every leaf node wrapper (no-op), `*Choice` enum (dispatch into the
/// present variant), and `*Children` struct (recurse into each named field).
pub(crate) trait Inspect {
    fn inspect(&self, rule: &'static str, out: &mut Vec<RawViolation>);
}

/// Field-level inspection, dispatched by the field's STATIC (container) TYPE
/// so a `*Children` struct's per-field macro invocation never needs to know
/// whether a given field is required / optional / repeat:
/// * `Positioned<S>`                    -> delegate to the slot/collection it carries.
/// * `NodeSlot<T>`                      -> required: `Present` recurses via `Inspect`; else a violation.
/// * `Option<NodeSlot<T>>`              -> optional: `Some(Present)` recurses; only `Some(Unexpected)` is a violation.
/// * `Vec<Positioned<NodeSlot<T>>>`     -> repeat: each `Present` element recurses; only `Unexpected` elements are violations.
pub(crate) trait InspectField {
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<RawViolation>);
}

/// An UNTYPED slot: the generated traversal falls back to a raw
/// `tree_sitter::Node` member where a repeat position admits children of
/// several flattened HIDDEN rules (e.g. `word_body`'s continuation repeat
/// after the 2026-07-11 overlap-custody port: `_interior_overlap` /
/// `_final_overlap_*` members flatten into the word). The conformance
/// contract still holds through the slot variants (ERROR / MISSING /
/// Unexpected are violations); a Present raw node is an opaque leaf with
/// nothing further to recurse into.
impl<'tree> Inspect for tree_sitter::Node<'tree> {
    fn inspect(&self, _rule: &'static str, _out: &mut Vec<RawViolation>) {}
}

/// A tracked position: delegate straight to the slot/collection it carries.
/// The leading extras a position records are not part of the conformance
/// contract (they are comments/whitespace, never classified content).
impl<'tree, S: InspectField> InspectField for Positioned<'tree, S> {
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<RawViolation>) {
        self.slot.inspect_field(rule, slot, out);
    }
}

/// Required slot: `Present` recurses into its payload (a leaf wrapper's own
/// `Inspect` is a no-op; a synthetic group or self-classifying choice may hold
/// further slots); every recovery state is itself a violation.
impl<'tree, T: Inspect> InspectField for NodeSlot<'tree, T> {
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<RawViolation>) {
        match self {
            NodeSlot::Present(value) => value.inspect(rule, out),
            NodeSlot::Unexpected(n) => out.push(RawViolation {
                rule_kind: rule,
                slot,
                status: SlotStatus::Unexpected,
                actual_child_kind: n.kind().to_string(),
            }),
            NodeSlot::Error(n) => out.push(RawViolation {
                rule_kind: rule,
                slot,
                status: SlotStatus::Error,
                actual_child_kind: n.kind().to_string(),
            }),
            NodeSlot::Missing(_) => out.push(RawViolation {
                rule_kind: rule,
                slot,
                status: SlotStatus::Missing,
                actual_child_kind: "<missing>".to_string(),
            }),
            NodeSlot::Absent => out.push(RawViolation {
                rule_kind: rule,
                slot,
                status: SlotStatus::Absent,
                actual_child_kind: "<absent>".to_string(),
            }),
        }
    }
}

/// Optional slot: `Some(Present)` recurses; optional-absent and
/// optional-missing/error are allowed; only a real node at an unexpected kind
/// (`Some(Unexpected)`) is a violation.
impl<'tree, T: Inspect> InspectField for Option<NodeSlot<'tree, T>> {
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<RawViolation>) {
        match self {
            Some(NodeSlot::Present(value)) => value.inspect(rule, out),
            Some(NodeSlot::Unexpected(n)) => out.push(RawViolation {
                rule_kind: rule,
                slot,
                status: SlotStatus::Unexpected,
                actual_child_kind: n.kind().to_string(),
            }),
            Some(NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Absent) | None => {}
        }
    }
}

/// Repeat-slot: each element should be a `Present` (recursed) or benign
/// recovery member; flag only `Unexpected` elements per the spec.
impl<'tree, T: Inspect> InspectField for Vec<Positioned<'tree, NodeSlot<'tree, T>>> {
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<RawViolation>) {
        for elem in self {
            match &elem.slot {
                NodeSlot::Present(value) => value.inspect(rule, out),
                NodeSlot::Unexpected(n) => out.push(RawViolation {
                    rule_kind: rule,
                    slot,
                    status: SlotStatus::Unexpected,
                    actual_child_kind: n.kind().to_string(),
                }),
                NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Absent => {}
            }
        }
    }
}

/// A fully-anonymous seq-member leaf (a `PATTERN` token tree-sitter absorbs, so
/// it has no child node): the content is read as an inter-sibling byte span, not
/// a `NodeSlot`. It carries no classification, so it can never be a
/// misclassification (no `Unexpected`/`Missing`/`Error`/`Absent` state exists for
/// it); inspection is a no-op.
impl<'tree> InspectField for LeafSpan<'tree> {
    fn inspect_field(
        &self,
        _rule: &'static str,
        _slot: &'static str,
        _out: &mut Vec<RawViolation>,
    ) {
    }
}

/// Walk a tree-sitter tree, calling `callback` on every node.
fn walk_all<'tree, F>(node: tree_sitter::Node<'tree>, callback: &mut F)
where
    F: FnMut(tree_sitter::Node<'tree>),
{
    callback(node);
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_all(cursor.node(), callback);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn corpus_dir() -> Option<std::path::PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .parent()?
        .join("corpus/reference");
    dir.exists().then_some(dir)
}

// ===========================================================================
// DOCUMENTED ALLOWLIST of the (rule, slot) positions that currently
// MISCLASSIFY a real child on the VALID reference corpus. Each entry is a
// known generator gap with its reason. Keyed by (rule, slot) rather than by
// individual offending kind so that adding a new grammar subtype does not
// spuriously fail (the whole choice POSITION is the gap), while a NEW gap at
// any other (rule, slot) still fails the test.
//
// EMPTY as of the Task B5 port to the NEW backend. The OLD-backend harness
// carried exactly one entry (`("mor_contents", "child_0")`, the 0d-D
// choice-of-seq gap: a `choice([seq(mor_content, ...), terminator])` whose
// `seq` alternative was dropped at generation, so a real `mor_content` child
// mis-slotted `Unexpected`). The NEW backend's ground truth is 0 partials / 0
// noted limitations for the whole grammar (recorded in
// `.superpowers/sdd/migration-progress.md`), and its `*Choice` enums type a
// seq alternative as a synthetic `*Children` payload variant rather than
// dropping it (see `MorContentsChild0Choice::MorContent(..)` in
// `generated_traversal.rs`, recursed into by this harness's `Inspect`
// impl for that enum), so the 0d-D gap is closed at the source and no
// allowlist entry carries forward. If a fresh run of this test surfaces a
// violation, that is a NEW finding to adjudicate (construct/confirm against
// the real grammar and the CHAT manual), never a reason to silently
// re-populate this list.
// ===========================================================================
const ALLOWLIST: &[(&str, &str)] = &[];

/// Whether a `(rule, slot)` misclassification is a documented known gap.
fn is_allowed(rule: &str, slot: &str) -> bool {
    ALLOWLIST.iter().any(|(r, s)| *r == rule && *s == slot)
}

#[test]
fn generated_traversal_conformance_no_misclassification_on_valid_corpus() {
    let Some(dir) = corpus_dir() else {
        eprintln!("Skipping: corpus/reference not found");
        return;
    };
    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    // signature -> (occurrence count, one example file)
    let mut found: BTreeMap<RawViolation, (usize, String)> = BTreeMap::new();
    let mut files = 0usize;

    for entry in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
    {
        let source = std::fs::read_to_string(entry.path()).expect("read file");
        let tree = parser.parse(&source, None).expect("parse");
        let file = entry.path().display().to_string();
        walk_all(tree.root_node(), &mut |node| {
            let mut raw = Vec::new();
            inventory::dispatch(node, &mut raw);
            for v in raw {
                let e = found.entry(v).or_insert((0, file.clone()));
                e.0 += 1;
            }
        });
        files += 1;
    }

    assert!(files >= 74, "expected >=74 reference files, got {files}");

    // Print the COMPLETE distinct violation set (the controller diagnostic).
    eprintln!(
        "=== generator-conformance: {} distinct violation signatures over {files} files ===",
        found.len()
    );
    for (sig, (count, example)) in &found {
        let allowed = is_allowed(sig.rule_kind, sig.slot);
        eprintln!(
            "  [{}] rule={} slot={} status={:?} actual={} (x{count}) e.g. {example}",
            if allowed { "ALLOW" } else { "NEW" },
            sig.rule_kind,
            sig.slot,
            sig.status,
            sig.actual_child_kind,
        );
    }

    // New-violation guard (REQUIRED): any (rule, slot) not on the allowlist is
    // a failure -- a newly mis-classifying rule, or a known-gap rule that began
    // mis-slotting at a NEW position.
    let new_violations: Vec<&RawViolation> = found
        .keys()
        .filter(|s| !is_allowed(s.rule_kind, s.slot))
        .collect();
    assert!(
        new_violations.is_empty(),
        "NEW generator-conformance violations (not on ALLOWLIST): {new_violations:#?}",
    );

    // Stale-allowlist guard (shrink-only): a listed (rule, slot) that no longer
    // violates must be removed from ALLOWLIST, so the allowlist only shrinks as
    // generator gaps are closed.
    let stale: Vec<&(&str, &str)> = ALLOWLIST
        .iter()
        .filter(|(r, sl)| !found.keys().any(|s| s.rule_kind == *r && s.slot == *sl))
        .collect();
    assert!(
        stale.is_empty(),
        "STALE ALLOWLIST entries (position no longer violates -- remove it): {stale:?}",
    );
}
