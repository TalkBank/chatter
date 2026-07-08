// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! Characterization tests for the utterance-level dispatch as it is migrated
//! onto the generated `GrammarTraversal::extract_utterance` visitor (Task 3a of
//! the visitor-driven parser migration).
//!
//! These tests pin the OBSERVABLE behavior of `parse_utterance_node` at the real
//! parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics + per-utterance `ParseHealth`). Task 3a replaces the hand-walked
//! `for utt_child in utt_node.children()` / `match child.kind()` dispatch with
//! `extract_utterance` + exhaustive `NodeSlot` dispatch over the `child_0`
//! (`main_tier`) and `child_1` (`dependent_tier` repeat) slots, but it is
//! behavior-preserving: the model, the recovery diagnostics, and the parse-health
//! taint must not change for any reachable input.
//!
//! Both expectation sets below were captured by RUNNING the pre-migration parser
//! (HEAD `acd466e`), not guessed:
//!
//! - A valid two-speaker file WITH `%mor` + `%gra` dependent tiers parses to two
//!   utterances, each carrying both dependent tiers, with zero diagnostics and a
//!   `Clean` parse-health. This exercises the `Present(main_tier)` arm followed by
//!   the `Present(dependent_tier)` repeat arm in document order, and pins the
//!   build-order invariant (the main tier is built before any dependent tier is
//!   attached).
//! - A `*CHI:` utterance with a structurally well-formed but semantically invalid
//!   `%gra:\t0|0|ROOT` line produces exactly one E709 diagnostic at the exact
//!   span, still attaches the `%gra` tier to the already-built main tier (proving
//!   the `utterance_builder.take()` build order survives), and taints ONLY the
//!   `Gra` alignment domain (Main stays clean). This exercises the dependent-tier
//!   Present arm's internal error-check + taint.

use talkbank_model::model::{ParseHealthState, ParseHealthTier};

mod common;
use common::parse_utterances_and_diags;

/// A `*CHI:` utterance whose `%gra` line is structurally valid but uses the
/// illegal grammatical index `0` (relations are 1-indexed). Tree-sitter parses
/// this as a `Present` `gra_dependent_tier`; `parse_and_attach_dependent_tier`
/// reports E709 and the dependent-tier branch taints the `Gra` domain. Reuses the
/// existing parse-health-recovery suite shape (inline input, not a new `.cha`).
const MALFORMED_GRA: &str = "@UTF8\n@Begin\n*CHI:\thello .\n%gra:\t0|0|ROOT\n@End\n";

#[test]
fn valid_utterances_with_dependent_tiers_parse_to_expected_model() {
    // Read the real reference fixture (no ad-hoc `.cha`): two valid utterances,
    // each with a `%mor` and a `%gra` dependent tier.
    let input = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/tiers/mor-gra.cha"
    ))
    .expect("read corpus/reference/tiers/mor-gra.cha");

    let (utterances, diags) = parse_utterances_and_diags(&input);

    assert!(
        diags.is_empty(),
        "valid mor/gra file must produce zero diagnostics, got: {diags:?}"
    );

    let speakers: Vec<&str> = utterances.iter().map(|u| u.main.speaker.as_str()).collect();
    assert_eq!(
        speakers,
        vec!["CHI", "MOT"],
        "must parse exactly the two utterances in file order"
    );

    for utt in &utterances {
        assert_eq!(
            utt.dependent_tiers.len(),
            2,
            "each utterance must carry both dependent tiers (%mor, %gra)"
        );
        assert!(utt.mor_tier().is_some(), "%mor tier must be attached");
        assert!(utt.gra_tier().is_some(), "%gra tier must be attached");
        assert!(
            matches!(utt.parse_health, ParseHealthState::Clean),
            "a valid utterance must have a clean parse-health, got: {:?}",
            utt.parse_health
        );
    }
}

#[test]
fn malformed_gra_emits_exact_diagnostic_and_taints_only_gra() {
    let (utterances, diags) = parse_utterances_and_diags(MALFORMED_GRA);

    // EXACTLY one diagnostic, captured from the pre-migration parser: E709 at the
    // `0|0|ROOT` span, reported by `parse_and_attach_dependent_tier`.
    assert_eq!(
        diags,
        vec![(
            "E709".to_string(),
            33,
            41,
            "Index cannot be 0 (indices are 1-indexed)".to_string(),
        )],
        "malformed %gra must emit exactly one E709 at span (33..41)"
    );

    assert_eq!(utterances.len(), 1, "exactly one utterance");
    let utt = &utterances[0];
    assert_eq!(utt.main.speaker.as_str(), "CHI");

    // The %gra tier is attached to the main tier that was built FIRST: this pins
    // the build-order invariant (main tier before dependent-tier attach).
    assert_eq!(
        utt.dependent_tiers.len(),
        1,
        "the malformed %gra tier is still attached to the built main tier"
    );
    assert!(utt.gra_tier().is_some(), "%gra tier must be attached");

    // Parse-health: the dependent-tier branch taints ONLY the Gra alignment
    // domain; the main tier stays clean.
    let ParseHealthState::Tainted(health) = &utt.parse_health else {
        panic!(
            "malformed %gra must taint parse-health, got: {:?}",
            utt.parse_health
        );
    };
    assert!(
        health.is_tier_clean(ParseHealthTier::Main),
        "main tier must remain clean for a malformed dependent tier"
    );
    assert!(
        health.is_tier_tainted(ParseHealthTier::Gra),
        "malformed %gra must taint the Gra alignment domain"
    );
}
