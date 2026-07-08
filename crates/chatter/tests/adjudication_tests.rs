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

//! Integration tests for `chatter adjudicate`.
//!
//! User contract: `book/src/architecture/adjudication-workflow.md`.
//! Each test exercises a real user-observable behavior via
//! `assert_cmd` subprocess.

use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;
use common::CliHarness;

/// Pre-existing pending-adjudications fixture: one
/// speaker-id-low-confidence entry whose suggested mapping has
/// PAR0=drop, PAR1=rename to INV/Investigator.
const FIX_PENDING_ONE_SPEAKER_ID: &str = r#"schema_version = 2

[[entries]]
session_id = "session-102-t1"
kind = "speaker-id-low-confidence"
created_at = "2026-05-27T11:00:00Z"
threshold_used = 2.0
margin = 1.82

[entries.suggested]
adult_roles = { PAR1 = { code = "INV", tag = "Investigator" } }
mapping = { PAR0 = "drop", PAR1 = "rename" }

[entries.scores]
PAR0 = 0.6286
PAR1 = 0.3457
"#;

/// Scripted decision fixture: accept-suggested for the same
/// session, no note.
const FIX_SCRIPTED_ACCEPT_SUGGESTED: &str = r#"schema_version = 1

[[decisions]]
session_id = "session-102-t1"
kind = "speaker-id-low-confidence"
choice = { kind = "accept-suggested" }
"#;

/// `chatter adjudicate` with `--scripted` consumes the canned
/// decisions in order, writes the resolved entry to the override
/// file, and updates the pending file to remove the resolved entry.
/// The CLI dispatch is a thin wrapper around `run_adjudication`;
/// this L3 test validates the file-I/O glue.
#[test]
fn adjudicate_scripted_accepts_suggested() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let scripted = dir.path().join("scripted.toml");
    let overrides = dir.path().join("batch.overrides.toml");
    fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;
    fs::write(&scripted, FIX_SCRIPTED_ACCEPT_SUGGESTED)?;

    harness
        .chatter_cmd()
        .arg("adjudicate")
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--scripted")
        .arg(&scripted)
        .arg("--operator")
        .arg("test-fixture")
        .assert()
        .success();

    // Override file exists with the resolved entry.
    assert!(overrides.exists(), "override file should be written");
    let overrides_text = fs::read_to_string(&overrides)?;
    assert!(
        overrides_text.contains("session-102-t1"),
        "override file should carry the resolved session ID:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"explicit\""),
        "AcceptSuggested decision should record as mode=explicit:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"INV\""),
        "override file should carry the suggested inserted_role.code INV:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"Investigator\""),
        "override file should carry the suggested inserted_role.tag Investigator:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("test-fixture"),
        "override file should carry the --operator value:\n{overrides_text}"
    );

    // Pending file has been rewritten without the resolved entry.
    let pending_text = fs::read_to_string(&pending)?;
    assert!(
        !pending_text.contains("session-102-t1"),
        "pending file should no longer carry the resolved session:\n{pending_text}"
    );
    assert!(
        pending_text.contains("schema_version"),
        "pending file should still declare schema_version after rewrite:\n{pending_text}"
    );
    Ok(())
}

/// `chatter adjudicate --interactive` reads operator responses from
/// stdin one line per pending entry. `"accept"` (or `"a"`) on a
/// speaker-id-low-confidence entry signals `AcceptSuggested`. The
/// adjudication core, override file write, and pending rewrite all
/// behave exactly the same as the scripted-mode path (cycle 18),
/// only the `Prompter` implementation differs.
#[test]
fn adjudicate_interactive_accepts_suggested() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let overrides = dir.path().join("batch.overrides.toml");
    fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;

    harness
        .chatter_cmd()
        .arg("adjudicate")
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--interactive")
        .arg("--operator")
        .arg("interactive-fixture")
        .write_stdin("accept\n")
        .assert()
        .success();

    assert!(overrides.exists(), "override file should be written");
    let overrides_text = fs::read_to_string(&overrides)?;
    assert!(
        overrides_text.contains("session-102-t1"),
        "override file should carry the resolved session ID:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"explicit\""),
        "interactive AcceptSuggested should record as mode=explicit:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("interactive-fixture"),
        "override file should carry the --operator value:\n{overrides_text}"
    );

    let pending_text = fs::read_to_string(&pending)?;
    assert!(
        !pending_text.contains("session-102-t1"),
        "pending file should no longer carry the resolved session:\n{pending_text}"
    );
    Ok(())
}

/// Pre-existing pending-adjudications fixture: one
/// speaker-id-low-confidence entry with TWO adult verdicts in a single
/// entry's `adult_roles` map, PAR0 renamed to INV/Investigator and
/// PAR1 renamed to FAT/Father (Task 8's multi-adult representation,
/// which replaced the earlier single shared `inserted_role` field).
const FIX_PENDING_MULTI_ADULT: &str = r#"schema_version = 2

[[entries]]
session_id = "session-410-multi-adult"
kind = "speaker-id-low-confidence"
created_at = "2026-05-27T11:00:00Z"
threshold_used = 2.0
margin = 1.82

[entries.suggested]
adult_roles = { PAR0 = { code = "INV", tag = "Investigator" }, PAR1 = { code = "FAT", tag = "Father" } }
mapping = { PAR0 = "rename", PAR1 = "rename" }

[entries.scores]
PAR0 = 0.6286
PAR1 = 0.5457
"#;

/// Scripted decision fixture: accept-suggested for the multi-adult
/// session, no note.
const FIX_SCRIPTED_ACCEPT_SUGGESTED_MULTI_ADULT: &str = r#"schema_version = 1

[[decisions]]
session_id = "session-410-multi-adult"
kind = "speaker-id-low-confidence"
choice = { kind = "accept-suggested" }
"#;

/// `chatter adjudicate --scripted` on a multi-adult
/// speaker-id-low-confidence entry (Task 8's `adult_roles` map) must
/// carry BOTH adult role assignments into the override file, not
/// collapse to a single shared role. This is the subprocess-level
/// counterpart to the unit-level
/// `apply_decision_accept_suggested_preserves_full_adult_roles_map`
/// test in `talkbank-transform`; this test exercises the real CLI
/// binary's file-I/O glue end to end.
#[test]
fn adjudicate_scripted_accepts_suggested_multi_adult() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let scripted = dir.path().join("scripted.toml");
    let overrides = dir.path().join("batch.overrides.toml");
    fs::write(&pending, FIX_PENDING_MULTI_ADULT)?;
    fs::write(&scripted, FIX_SCRIPTED_ACCEPT_SUGGESTED_MULTI_ADULT)?;

    harness
        .chatter_cmd()
        .arg("adjudicate")
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--scripted")
        .arg(&scripted)
        .arg("--operator")
        .arg("test-fixture-multi-adult")
        .assert()
        .success();

    // Override file exists with the resolved entry.
    assert!(overrides.exists(), "override file should be written");
    let overrides_text = fs::read_to_string(&overrides)?;
    assert!(
        overrides_text.contains("session-410-multi-adult"),
        "override file should carry the resolved session ID:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"explicit\""),
        "AcceptSuggested decision should record as mode=explicit:\n{overrides_text}"
    );

    // Both adult roles must survive intact: PAR0 -> INV/Investigator
    // and PAR1 -> FAT/Father, not collapsed to a single shared role.
    assert!(
        overrides_text.contains("\"INV\""),
        "override file should carry PAR0's inserted_role.code INV:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"Investigator\""),
        "override file should carry PAR0's inserted_role.tag Investigator:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"FAT\""),
        "override file should carry PAR1's inserted_role.code FAT:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"Father\""),
        "override file should carry PAR1's inserted_role.tag Father:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("test-fixture-multi-adult"),
        "override file should carry the --operator value:\n{overrides_text}"
    );

    // Pending file has been rewritten without the resolved entry.
    let pending_text = fs::read_to_string(&pending)?;
    assert!(
        !pending_text.contains("session-410-multi-adult"),
        "pending file should no longer carry the resolved session:\n{pending_text}"
    );
    assert!(
        pending_text.contains("schema_version"),
        "pending file should still declare schema_version after rewrite:\n{pending_text}"
    );
    Ok(())
}

/// Pre-existing pending-adjudications fixture with a parent-role-lookup
/// entry. The pipeline identified a parent speaker but doesn't know
/// whether to label them `MOT`, `FAT`, or another role.
const FIX_PENDING_PARENT_ROLE: &str = r#"schema_version = 2

[[entries]]
session_id = "session-307-parent"
created_at = "2026-05-27T12:00:00Z"
kind = "parent-role-lookup"
donor_speaker = "PAR"

[entries.speaker_mapping]
PAR = "rename"
"#;

/// `chatter adjudicate --interactive` parses `choose SPK:CODE:TAG
/// [SPK:CODE:TAG ...] [optional note]` lines for parent-role-lookup
/// pending entries. The operator types the role on stdin; the override
/// file records the chosen role paired with the pre-set speaker mapping.
#[test]
fn adjudicate_interactive_chooses_role() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let overrides = dir.path().join("batch.overrides.toml");
    fs::write(&pending, FIX_PENDING_PARENT_ROLE)?;

    harness
        .chatter_cmd()
        .arg("adjudicate")
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--interactive")
        .arg("--operator")
        .arg("parent-role-fixture")
        .write_stdin("choose PAR:MOT:Mother contributor data sheet\n")
        .assert()
        .success();

    let overrides_text = fs::read_to_string(&overrides)?;
    assert!(
        overrides_text.contains("session-307-parent"),
        "override file should carry the resolved parent-role session:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"MOT\"") && overrides_text.contains("\"Mother\""),
        "override file should record the chosen role (MOT/Mother):\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("contributor data sheet"),
        "operator note should be recorded:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("parent-role-fixture"),
        "operator field should be the --operator value:\n{overrides_text}"
    );

    let pending_text = fs::read_to_string(&pending)?;
    assert!(
        !pending_text.contains("session-307-parent"),
        "pending file should no longer carry the resolved session:\n{pending_text}"
    );
    Ok(())
}

/// `chatter adjudicate --interactive` parses `override SPK:CODE:TAG
/// [SPK:CODE:TAG ...] SPK=action [SPK=action ...] [note...]` lines for
/// speaker-id-low-confidence entries. This is the interactive
/// counterpart of cycle 20's `OverrideMapping` decision: the
/// operator looked at the algorithm's suggestion, decided it was
/// wrong, and supplies the mapping + per-speaker roles directly.
#[test]
fn adjudicate_interactive_override_mapping() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let overrides = dir.path().join("batch.overrides.toml");
    fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;

    // The cycle-18 pending fixture suggests PAR0=drop, PAR1=rename
    // with PAR1's role INV:Investigator. The operator overrides:
    // PAR0=rename, PAR1=drop, and assigns PAR0 the role MOT:Mother
    // (SPK:CODE:TAG syntax), with a multi-word note.
    harness
        .chatter_cmd()
        .arg("adjudicate")
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--interactive")
        .arg("--operator")
        .arg("override-fixture")
        .write_stdin("override PAR0:MOT:Mother PAR0=rename PAR1=drop audio review confirms swap\n")
        .assert()
        .success();

    let overrides_text = fs::read_to_string(&overrides)?;
    assert!(
        overrides_text.contains("session-102-t1"),
        "override file should carry the resolved session:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"explicit\""),
        "interactive OverrideMapping should record as mode=explicit:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"MOT\"") && overrides_text.contains("\"Mother\""),
        "override file should record the operator's inserted_role:\n{overrides_text}"
    );
    assert!(
        // PAR0 should now be "rename", the inverse of the suggestion.
        overrides_text.contains("PAR0 = \"rename\"")
            || overrides_text.contains("PAR0=\"rename\"")
            || overrides_text.contains("\"PAR0\" = \"rename\""),
        "override file should record PAR0 as rename per the override mapping:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("PAR1 = \"drop\"")
            || overrides_text.contains("PAR1=\"drop\"")
            || overrides_text.contains("\"PAR1\" = \"drop\""),
        "override file should record PAR1 as drop per the override mapping:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("audio review confirms swap"),
        "multi-word note should be recorded:\n{overrides_text}"
    );

    let pending_text = fs::read_to_string(&pending)?;
    assert!(
        !pending_text.contains("session-102-t1"),
        "pending file should no longer carry the resolved session:\n{pending_text}"
    );
    Ok(())
}
