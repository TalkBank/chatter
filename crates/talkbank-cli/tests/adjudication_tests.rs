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
const FIX_PENDING_ONE_SPEAKER_ID: &str = r#"schema_version = 1

[[entries]]
session_id = "session-102-t1"
kind = "speaker-id-low-confidence"
created_at = "2026-05-27T11:00:00Z"
threshold_used = 2.0
margin = 1.82

[entries.suggested]
inserted_role = { code = "INV", tag = "Investigator" }
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

/// Pre-existing pending-adjudications fixture with a parent-role-lookup
/// entry. The pipeline identified a parent speaker but doesn't know
/// whether to label them `MOT`, `FAT`, or another role.
const FIX_PENDING_PARENT_ROLE: &str = r#"schema_version = 1

[[entries]]
session_id = "session-307-parent"
created_at = "2026-05-27T12:00:00Z"
kind = "parent-role-lookup"
donor_speaker = "PAR"

[entries.speaker_mapping]
PAR = "rename"
"#;

/// `chatter adjudicate --interactive` parses `choose CODE TAG
/// [optional note]` lines for parent-role-lookup pending entries.
/// The operator types the role on stdin; the override file records
/// the chosen role paired with the pre-set speaker mapping.
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
        .write_stdin("choose MOT Mother contributor data sheet\n")
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

/// `chatter adjudicate --interactive` parses `override CODE TAG
/// SPK=action [SPK=action ...] [note...]` lines for
/// speaker-id-low-confidence entries. This is the interactive
/// counterpart of cycle 20's `OverrideMapping` decision: the
/// operator looked at the algorithm's suggestion, decided it was
/// wrong, and supplies the mapping + inserted_role directly.
#[test]
fn adjudicate_interactive_override_mapping() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let overrides = dir.path().join("batch.overrides.toml");
    fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;

    // The cycle-18 pending fixture suggests PAR0=drop, PAR1=rename
    // with inserted_role=INV:Investigator. The operator overrides:
    // PAR0=rename, PAR1=drop, inserted_role=MOT:Mother, with a
    // multi-word note.
    harness
        .chatter_cmd()
        .arg("adjudicate")
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--interactive")
        .arg("--operator")
        .arg("override-fixture")
        .write_stdin("override MOT Mother PAR0=rename PAR1=drop audio review confirms swap\n")
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
