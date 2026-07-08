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

//! Characterization tests for the utterance-END tail dispatch as it is migrated
//! onto the generated `GrammarTraversal::extract_utterance_end` visitor (Task 3d
//! of the visitor-driven parser migration).
//!
//! These tests pin the OBSERVABLE behavior of the utterance-end decode at the real
//! parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics). Task 3d replaces the flat positional `node.kind()` loop
//! (`parse_utterance_end`) with `extract_utterance_end` + an exhaustive typed match
//! over the four `utterance_end` slots (`terminator` supertype, `final_codes`,
//! `bullet`, `newline`), but it is behavior-preserving: the terminator, the
//! postcodes, the trailing media bullet, and the ONE reachable recovery diagnostic
//! (E360 `InvalidMediaBullet` on a malformed bullet) must not change for any
//! reachable input.
//!
//! Both expectation sets below were captured by RUNNING the pre-migration parser
//! (HEAD `b0a1735`), not guessed:
//!
//! - A valid `*CHI:` line carrying a period terminator, a `[+ trn]` postcode, and a
//!   valid `·0_1000·` media bullet parses to one utterance with that terminator,
//!   that postcode, and that bullet, with ZERO diagnostics. This exercises the
//!   `Present(TerminatorChoice::Period)` arm, the `final_codes` postcode arm, and
//!   the `bullet` `Some(timestamps)` arm together.
//! - A `*CHI:` line whose trailing bullet is the deprecated `·N_N-·` skip marker
//!   (grammar-rejected on the trailing `-`) still parses the period terminator, but
//!   drops the bullet and emits EXACTLY ONE E360 at the bullet's byte span. This
//!   exercises the `bullet` `None` arm, whose E360 the migration must reproduce
//!   byte-identically. Note there is NO separate whole-tree-backstop diagnostic on
//!   this input: E360 is the single reachable diagnostic.

use talkbank_model::model::Terminator;

mod common;
use common::parse_utterances_and_diags;

/// A valid line with all three utterance-end adjuncts: `.` terminator, `[+ trn]`
/// postcode, and a `·0_1000·` media bullet (the `·` is the 0x15 time-bullet
/// delimiter). Header scaffolding keeps the streaming parser happy.
const VALID_END: &str = "@UTF8\n@Begin\n*CHI:\thello . [+ trn] \u{15}0_1000\u{15}\n@End\n";

/// Same shape, but the bullet is the deprecated `·N_N-·` skip marker: the grammar
/// reports an ERROR on the trailing `-`, so `parse_bullet_node_timestamps` returns
/// `None` and the decode emits E360 `InvalidMediaBullet`.
const MALFORMED_BULLET: &str = "@UTF8\n@Begin\n*CHI:\thello . \u{15}123_456-\u{15}\n@End\n";

#[test]
fn valid_terminator_postcode_bullet_unchanged() {
    let (utterances, diags) = parse_utterances_and_diags(VALID_END);

    assert!(
        diags.is_empty(),
        "a valid terminator + postcode + bullet line must emit zero diagnostics, got: {diags:?}"
    );
    assert_eq!(utterances.len(), 1, "exactly one utterance");
    let content = &utterances[0].main.content;

    // Period terminator at the captured span (25..26).
    match &content.terminator {
        Some(Terminator::Period { span }) => {
            assert_eq!(
                (span.start, span.end),
                (25, 26),
                "period terminator span must be preserved"
            );
        }
        other => panic!("expected a Period terminator, got: {other:?}"),
    }

    // One `[+ trn]` postcode.
    assert_eq!(content.postcodes.len(), 1, "exactly one postcode");
    assert_eq!(
        content.postcodes[0].text.as_str(),
        "trn",
        "postcode text must be preserved"
    );

    // Valid media bullet `·0_1000·` at the captured span (35..43).
    let bullet = content
        .bullet
        .as_ref()
        .expect("valid media bullet must be attached");
    assert_eq!(
        (bullet.timing.start_ms, bullet.timing.end_ms),
        (0, 1000),
        "bullet timestamps must be preserved"
    );
    assert_eq!(
        (bullet.span.start, bullet.span.end),
        (35, 43),
        "bullet span must be preserved"
    );
}

#[test]
fn malformed_bullet_emits_exact_e360() {
    let (utterances, diags) = parse_utterances_and_diags(MALFORMED_BULLET);

    // EXACTLY one diagnostic, captured from the pre-migration parser: E360 at the
    // bullet's byte span (27..37) with the exact legal-form message. There is NO
    // additional whole-tree-backstop diagnostic on this input.
    let expected_message = format!(
        "Invalid media bullet: grammar rejected '{}'. Legal form: \u{b7}START_END\u{b7} with numeric timestamps only",
        "\u{15}123_456-\u{15}"
    );
    assert_eq!(
        diags,
        vec![("E360".to_string(), 27, 37, expected_message)],
        "malformed bullet must emit exactly one E360 at span (27..37)"
    );

    assert_eq!(utterances.len(), 1, "exactly one utterance");
    let content = &utterances[0].main.content;

    // The period terminator is still parsed (terminator decode is independent of
    // the malformed bullet).
    assert!(
        matches!(content.terminator, Some(Terminator::Period { .. })),
        "period terminator must still be parsed, got: {:?}",
        content.terminator
    );

    // The malformed bullet is dropped, and there is no postcode on this line.
    assert!(content.postcodes.is_empty(), "no postcodes on this line");
    assert!(
        content.bullet.is_none(),
        "a malformed bullet must not be attached to the model"
    );
}
