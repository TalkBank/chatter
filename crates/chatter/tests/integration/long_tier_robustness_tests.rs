//! Long main tiers must never crash the validator.
//!
//! Field finding (2026-07-11 CHECK-parity grounding, code 10): a single
//! `*CHI:` turn of ~21,000 characters (thousands of plain words) makes
//! `chatter validate` die with `fatal runtime error: stack overflow`
//! (SIGABRT), and validation time grows superlinearly with tier length
//! well before the crash. CLAN's CHECK caps tier text at UTTLINELEN
//! (18,000 bytes, code 10); chatter deliberately has no such arbitrary
//! cap, so it MUST handle arbitrarily long tiers gracefully instead.
//!
//! The test drives the real binary at the CLI boundary: whatever the
//! validation verdict is, the process must terminate normally (exit
//! with a status, not a signal).

use std::fs;
use std::process::Command;

/// One utterance of `n` whitespace-separated plain words plus a
/// terminator: the smallest shape that reproduces the crash.
fn long_turn_fixture(n: usize) -> String {
    let mut turn = String::from("*CHI:\t");
    for _ in 0..n {
        turn.push_str("word ");
    }
    turn.push('.');
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|3;00.|male|||Target_Child|||\n\
         @Options:\tbullets\n{turn}\n@End\n"
    )
}

/// The validator must not be killed by a signal on a ~21K-char turn
/// (3,500 words), the empirically crashing size from the field fixture.
#[test]
fn validate_survives_a_21k_char_main_tier() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cha = dir.path().join("long.cha");
    fs::write(&cha, long_turn_fixture(3_500)).expect("write fixture");

    let out = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args(["validate"])
        .arg(&cha)
        .output()
        .expect("spawn chatter validate");

    // Any normal exit code is acceptable (valid or invalid is the
    // rule-10 adjudication's business); dying on a SIGNAL is the bug.
    assert!(
        out.status.code().is_some(),
        "chatter validate was killed by a signal on a long tier: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
}
