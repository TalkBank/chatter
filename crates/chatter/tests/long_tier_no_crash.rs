// Test code: the panic-family clippy lints are relaxed by policy.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Regression: a very long main tier must not crash or hang the validator.
//!
//! Found 2026-07-11 while grounding CLAN CHECK error code 10 (tier text
//! longer than 18000 chars). `chatter validate` on a ~4000-word single
//! utterance aborted with a stack overflow (deep recursion past even the
//! 16 MiB program stack), and much shorter tiers took absurd wall-clock
//! time (~11s at 500 words), a separate superlinear-time defect with the
//! same root. The validator must instead terminate normally and quickly,
//! whatever it decides about the file's validity.

mod common;

use common::CliHarness;

/// Build a valid CHAT document whose single utterance has `words` tokens.
fn long_tier_doc(words: usize) -> String {
    let mut body = String::with_capacity(words * 5 + 64);
    body.push_str(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t",
    );
    for i in 0..words {
        if i > 0 {
            body.push(' ');
        }
        body.push_str("word");
    }
    body.push_str(" .\n@End\n");
    body
}

/// A tier long enough to trigger the recursion-depth crash must validate
/// without dying to a signal (stack overflow aborts the process, so the
/// subprocess exit code is absent when the bug is present).
#[test]
fn long_main_tier_does_not_crash() {
    let harness = CliHarness::new().expect("harness");
    let path = harness.home_dir().join("long_tier.cha");
    std::fs::create_dir_all(harness.home_dir()).expect("home dir");
    std::fs::write(&path, long_tier_doc(4000)).expect("write fixture");

    let output = harness
        .run_validate(&path, &["--force"])
        .expect("validate runs");

    assert!(
        output.status.code().is_some(),
        "validate died to a signal (stack overflow) on a long tier; \
         exit status: {:?}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
}
