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

//! Word-level validation must reach words nested inside groups, end to end
//! through the CLI.
//!
//! # The defect these pin
//!
//! Main-tier word validation iterated the utterance's content items FLATLY
//! and matched only `Word`, `AnnotatedWord`, and `ReplacedWord`, with a
//! `_ => {}` catch-all swallowing every container. So a word inside a
//! retrace, a reformulation, or an angle group was never validated at all:
//! not for digits, not for illegal characters, not for anything.
//!
//! The identical token was therefore rejected outside a group and accepted
//! inside one. `hello3 dog .` was invalid (E220); `hello3 [/] hello dog .`
//! was valid. That held on every chatter release up to this fix, so E220 had
//! the hole for as long as the rule has existed.
//!
//! This is exactly the failure mode the repository's own design rules name:
//! exhaustive matches over `UtteranceContent` with no `_ =>` catch-all that
//! discards content, and traversal through the shared recursive `walk_words`
//! walker rather than flat index iteration.
//!
//! # What these tests are worth
//!
//! Digits (`E220`) are used as the probe because that rule is old, simple,
//! and independent of anything added recently: if a nested word is validated
//! at all, E220 fires. The prefix-marker rule is probed too, so the same
//! guarantee is pinned for a language-gated check added later.

use predicates::prelude::*;
use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

/// Build a one-utterance English file with the given main-tier content.
fn chat_file(words: &str) -> String {
    format!(
        "@UTF8\n\
         @Begin\n\
         @Languages:\teng\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|3;00.00|male|||Target_Child|||\n\
         *CHI:\t{words} .\n\
         @End\n",
    )
}

/// Run `chatter validate` and assert the verdict.
fn assert_validation(name: &str, content: &str, valid: bool) -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join(name);
    fs::write(&file_path, content)?;

    let assertion = assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .assert();
    if valid {
        assertion
            .success()
            .stdout(predicate::str::contains("Invalid: 0"));
    } else {
        assertion
            .failure()
            .stdout(predicate::str::contains("Invalid: 1"));
    }
    Ok(())
}

/// Baseline: the defect is about NESTING, so the un-nested case must already
/// be rejected. If this ever fails, the probe rule itself has changed and the
/// rest of the file proves nothing.
#[test]
fn a_digit_bearing_word_is_rejected_when_it_stands_alone() -> Result<(), TestError> {
    assert_validation("flat.cha", &chat_file("hello3 dog"), false)
}

#[test]
fn a_digit_bearing_word_is_rejected_inside_a_retrace() -> Result<(), TestError> {
    assert_validation("retrace.cha", &chat_file("hello3 [/] hello dog"), false)
}

#[test]
fn a_digit_bearing_word_is_rejected_inside_a_reformulation() -> Result<(), TestError> {
    assert_validation(
        "reformulation.cha",
        &chat_file("hello3 [//] hello dog"),
        false,
    )
}

#[test]
fn a_digit_bearing_word_is_rejected_inside_an_angle_group() -> Result<(), TestError> {
    assert_validation(
        "group.cha",
        &chat_file("<hello3 there> [/] hello there dog"),
        false,
    )
}

/// The prefix-marker rule gets the same guarantee, so a language-gated check
/// added after the walker fix cannot regress back to flat iteration.
#[test]
fn a_prefix_marker_word_is_rejected_inside_a_retrace() -> Result<(), TestError> {
    assert_validation("marker_retrace.cha", &chat_file("sun# [/] sun dog"), false)
}

/// Control: nesting must not make a LEGAL word illegal.
///
/// Without this, "validate everything nested" could be satisfied by a rule
/// that simply rejects nested content, and the suite would not notice.
#[test]
fn a_legal_word_stays_legal_inside_a_retrace() -> Result<(), TestError> {
    assert_validation("legal_retrace.cha", &chat_file("hello [/] hello dog"), true)
}

/// Control: a legal word inside an angle group stays legal.
#[test]
fn a_legal_word_stays_legal_inside_an_angle_group() -> Result<(), TestError> {
    assert_validation(
        "legal_group.cha",
        &chat_file("<hello there> [/] hello there dog"),
        true,
    )
}
