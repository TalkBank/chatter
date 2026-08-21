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

//! PERMANENT regression tests, not migration scaffolding.
//!
//! The visitor-driven parser migration these were written during is COMPLETE
//! (shipped 0.3.x). What they pin is the parser's CURRENT observable contract
//! at the real boundary, and every expected value here was captured by RUNNING
//! the parser rather than guessed. Historical "migration" wording below refers
//! to that finished work; these files are not leftovers and must not be deleted
//! as such.
//!
//! Characterization tests for the TEXT dependent tiers migrated onto the
//! generated `extract_<tier>_dependent_tier` typed visitor (Task 4b).
//!
//! These tests pin the OBSERVABLE behaviour of the nine text tiers (`%com`,
//! `%act`, `%cod`, `%exp`, `%add`, `%spa`, `%sit`, `%int`, `%gpx`) at the real
//! parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics). The migration replaces the hand-walked `child.kind()`
//! string-dispatch loop in `tier_parsers/text/helpers.rs` (and the inline copies
//! in `act.rs` / `cod.rs`) with exhaustive typed `NodeSlot` matching over each
//! tier's `child_2` body slot, but it is BEHAVIOUR-PRESERVING: the model and the
//! recovery diagnostics must not change.
//!
//! Every value here was captured by RUNNING the pre-migration parser (base HEAD
//! `3d146a8`), not guessed. The tests PASS on the current code and MUST STAY
//! GREEN, byte-identical, after the migration.
//!
//! # Coverage
//!
//! - VALID: `corpus/reference/edge-cases/multi-tier-alignment.cha`, a real
//!   reference file carrying `%com`, `%act`, `%cod`, `%gpx`, `%add` across two
//!   utterances. Pins the exact `BulletContent` of every text tier and ZERO
//!   diagnostics. This exercises the `NodeSlot::Present(text_node)` arm (the only
//!   arm reached for valid CHAT).
//! - MALFORMED: a `%com:` tier with an empty body. Empirically (captured on
//!   current code) tree-sitter forms a PRESENT `text_with_bullets_and_pics` body
//!   whose only child is a MISSING `continuation`, so the body slot is
//!   `NodeSlot::Present` (NOT `Missing`/`Absent`); the text-tier parser recovers
//!   it to a single `Continuation` segment, and the two E342 diagnostics come
//!   from the whole-tree recovery backstop (which Task 4b does not touch), NOT
//!   from the text-tier parser itself. This documents that the parser's own
//!   "Missing content" / `unexpected_node_error` recovery arms are unreachable
//!   in practice, and pins the recovery that IS observed.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{BulletContent, BulletContentSegment, DependentTier, Line};
use talkbank_parser::TreeSitterParser;

/// One diagnostic: (code, span start, span end, message).
type Diag = (String, u32, u32, String);

/// Render one bullet segment as a stable structural tag for assertions.
fn seg_tag(seg: &BulletContentSegment) -> String {
    match seg {
        BulletContentSegment::Text(t) => format!("Text({})", t.text),
        BulletContentSegment::Bullet(_) => "Bullet".to_string(),
        BulletContentSegment::Picture(_) => "Picture".to_string(),
        BulletContentSegment::Continuation => "Continuation".to_string(),
    }
}

/// Render a `BulletContent` as an ordered list of segment tags.
fn content_tags(content: &BulletContent) -> Vec<String> {
    content.segments.iter().map(seg_tag).collect()
}

/// One text dependent tier described as (variant name, segment tags). Only the
/// nine text tiers in scope for Task 4b are returned; structured tiers (`%mor`,
/// `%gra`, `%pho`, ...) are ignored so the assertion stays focused on the text
/// tier bodies this migration touches.
fn text_tier(dt: &DependentTier) -> Option<(&'static str, Vec<String>)> {
    match dt {
        DependentTier::Com(t) => Some(("Com", content_tags(&t.content))),
        DependentTier::Act(t) => Some(("Act", content_tags(&t.content))),
        DependentTier::Cod(t) => Some(("Cod", content_tags(&t.content))),
        DependentTier::Exp(t) => Some(("Exp", content_tags(&t.content))),
        DependentTier::Add(t) => Some(("Add", content_tags(&t.content))),
        DependentTier::Spa(t) => Some(("Spa", content_tags(&t.content))),
        DependentTier::Sit(t) => Some(("Sit", content_tags(&t.content))),
        DependentTier::Int(t) => Some(("Int", content_tags(&t.content))),
        DependentTier::Gpx(t) => Some(("Gpx", content_tags(&t.content))),
        _ => None,
    }
}

/// Parse `input` at the streaming boundary and return, in document order, every
/// text dependent tier as (variant name, segment tags), plus every collected
/// diagnostic as `(code, start, end, message)`.
fn parse_text_tiers_and_diags(input: &str) -> (Vec<(&'static str, Vec<String>)>, Vec<Diag>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);

    let mut tiers = Vec::new();
    for line in chat.lines.as_slice() {
        if let Line::Utterance(u) = line {
            for dt in &u.dependent_tiers {
                if let Some(entry) = text_tier(&dt.tier) {
                    tiers.push(entry);
                }
            }
        }
    }

    let diags = errors
        .into_vec()
        .into_iter()
        .map(|d| {
            (
                d.code.as_str().to_string(),
                d.location.span.start,
                d.location.span.end,
                d.message,
            )
        })
        .collect();

    (tiers, diags)
}

/// VALID: a real reference file with five text tiers across two utterances must
/// parse to the exact `BulletContent` for each tier and produce ZERO
/// diagnostics. Pins the `NodeSlot::Present(text_node)` -> `parse_bullet_content`
/// path that valid CHAT always takes.
#[test]
fn valid_reference_text_tiers_parse_byte_identical_with_zero_diagnostics() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/edge-cases/multi-tier-alignment.cha"
    );
    let input = std::fs::read_to_string(path).expect("read multi-tier-alignment.cha fixture");

    let (tiers, diags) = parse_text_tiers_and_diags(&input);

    assert_eq!(
        tiers,
        vec![
            ("Com", vec!["Text(CHI is excited and jumping)".to_string()]),
            ("Act", vec!["Text(jumping up and down)".to_string()]),
            ("Gpx", vec!["Text(waving arms wildly)".to_string()]),
            ("Add", vec!["Text(MOT)".to_string()]),
            ("Cod", vec!["Text($IMP $V)".to_string()]),
            ("Com", vec!["Text(MOT is smiling)".to_string()]),
        ],
        "every valid text tier must parse to its exact BulletContent, in document order"
    );
    assert!(
        diags.is_empty(),
        "valid reference file must produce zero diagnostics, got: {diags:?}"
    );
}

// REMOVED 2026-08-16: `empty_body_com_tier_recovers_to_continuation_with_e342`.
//
// It characterized an empty-body `%com:` recovering to a single `Continuation`
// segment with two E342 diagnostics, values "captured on the pre-migration
// parser". The E756 widening abolished exactly that behavior: `%com`'s grammar
// body is now `optional(...)`, so the line lowers to a tier that says it is
// empty and the validator reports E756, with no recovery and no E342. The test
// asserted, in its own words, that the result was "NOT an empty tier", which is
// now the correct answer.
//
// Not rewritten in place, because the same input is already gated permanently
// and generatively: spec `E756_empty_dependent_tier.md` Example 3 produces
// `error_corpus/validation_errors/E756_empty_dependent_tier_3.cha`. Keeping a
// hand-written twin here would relocate a check the spec owns rather than
// remove one. Deletion approved by the maintainer, per danger rule 7.
