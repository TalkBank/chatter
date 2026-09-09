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
//! mis-typed function (a supertype seq-member guarded on the literal kind that
//! classifies every real child `Unexpected`, or a choice rule whose content
//! alternative is dropped) compiles, round-trips, passes clippy, and RUNS
//! there, yet the wrongness is thrown away. This test KEEPS the result and
//! asserts, on the valid reference corpus, that no position is a violation
//! under `talkbank_parser_tests::conformance::is_violation`: no slot is
//! `Unexpected` anywhere, and no REQUIRED slot is `Missing`, `Absent` or
//! `Error`. Optional-absent, and a recovery node in an optional or repeat
//! position, are allowed.
//!
//! The walk itself, and the per-type inspection inventory it drives, live in
//! the library (`talkbank_parser_tests::conformance`), shared with the
//! slot-state census example; this file holds the policy's application and the
//! allowlist. The inventory is MECHANICALLY derived from
//! `crates/talkbank-parser/src/generated_traversal.rs`; regenerate it with
//! `cargo run -p talkbank-parser-tests --example gen_conformance_inventory`,
//! and the staleness guard `conformance_inventory_is_current` fails if the
//! committed copy has drifted.

use std::collections::BTreeMap;

use talkbank_parser_tests::conformance::{Observation, dispatch, is_violation, walk_all};

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
// mis-slotted `Unexpected`). The NEW backend types a seq alternative as a
// synthetic `*Children` payload variant rather than dropping it, so the gap is
// closed at the source and no allowlist entry carries forward. If a fresh run
// of this test surfaces a violation, that is a NEW finding to adjudicate
// (construct/confirm against the real grammar and the CHAT manual), never a
// reason to silently re-populate this list.
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
    let mut found: BTreeMap<Observation, (usize, String)> = BTreeMap::new();
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
            dispatch(node, &mut raw);
            for v in raw.into_iter().filter(is_violation) {
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
            "  [{}] rule={} slot={} container={:?} observed={:?} actual={} (x{count}) e.g. {example}",
            if allowed { "ALLOW" } else { "NEW" },
            sig.rule_kind,
            sig.slot,
            sig.container,
            sig.observed,
            sig.actual_child_kind,
        );
    }

    // New-violation guard (REQUIRED): any (rule, slot) not on the allowlist is
    // a failure: a newly mis-classifying rule, or a known-gap rule that began
    // mis-slotting at a NEW position.
    let new_violations: Vec<&Observation> = found
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
        "STALE ALLOWLIST entries (position no longer violates: remove it): {stale:?}",
    );
}
