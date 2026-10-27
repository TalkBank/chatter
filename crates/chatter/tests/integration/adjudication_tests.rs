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

use predicates::prelude::*;
use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

use crate::common::CliHarness;

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

#[derive(Clone, Copy)]
enum Alias {
    SamePath,
    DotPath,
    HardLink,
    #[cfg(unix)]
    Symlink,
}

const ALIASES: &[Alias] = &[
    Alias::SamePath,
    Alias::DotPath,
    Alias::HardLink,
    #[cfg(unix)]
    Alias::Symlink,
];

impl Alias {
    fn create(self, source: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
        let alias = source.with_file_name("alias.toml");
        match self {
            Self::SamePath => Ok(source.to_path_buf()),
            Self::DotPath => Ok(source
                .parent()
                .unwrap()
                .join(".")
                .join(source.file_name().unwrap())),
            Self::HardLink => {
                fs::hard_link(source, &alias)?;
                Ok(alias)
            }
            #[cfg(unix)]
            Self::Symlink => {
                std::os::unix::fs::symlink(source, &alias)?;
                Ok(alias)
            }
        }
    }
}

/// A combined document is readable by both readers, but must not be rewritten
/// as pending output while it also serves as the operator's decision script.
#[test]
fn adjudicate_refuses_pending_script_aliases() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    for alias in ALIASES {
        let dir = tempdir()?;
        let pending = dir.path().join("pending.toml");
        let overrides = dir.path().join("overrides.toml");
        let combined = format!(
            "{}\n{}",
            FIX_PENDING_ONE_SPEAKER_ID,
            FIX_SCRIPTED_ACCEPT_SUGGESTED.split_once('\n').unwrap().1,
        );
        fs::write(&pending, &combined)?;
        let script = alias.create(&pending)?;
        let output = harness
            .chatter_cmd()
            .args(["experimental", "review", "scripted"])
            .arg(&pending)
            .arg("--override-file")
            .arg(&overrides)
            .arg("--script")
            .arg(&script)
            .arg("--operator")
            .arg("operator")
            .output()?;
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("alias"));
        assert_eq!(fs::read_to_string(&pending)?, combined);
        assert_eq!(fs::read_to_string(&script)?, combined);
        assert!(
            !overrides.exists(),
            "refusal must not create an override file"
        );
    }
    Ok(())
}

/// Neither output may alias another role, even when the contents would fail
/// later schema admission. Interactive rejection must happen before prompting.
#[test]
fn adjudicate_refuses_override_aliases_before_reading_or_prompting() -> Result<(), TestError> {
    enum ProtectedRole {
        Pending,
        Script,
    }
    let harness = CliHarness::new()?;
    for role in [ProtectedRole::Pending, ProtectedRole::Script] {
        for alias in ALIASES {
            let dir = tempdir()?;
            let pending = dir.path().join("pending.toml");
            let script = dir.path().join("script.toml");
            fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;
            fs::write(&script, FIX_SCRIPTED_ACCEPT_SUGGESTED)?;
            let protected = match role {
                ProtectedRole::Pending => &pending,
                ProtectedRole::Script => &script,
            };
            let overrides = alias.create(protected)?;
            let mut command = harness.chatter_cmd();
            command
                .args(["experimental", "review"])
                .arg(match role {
                    ProtectedRole::Pending => "interactive",
                    ProtectedRole::Script => "scripted",
                })
                .arg(&pending)
                .arg("--override-file")
                .arg(&overrides)
                .arg("--operator")
                .arg("operator");
            match role {
                ProtectedRole::Pending => {}
                ProtectedRole::Script => {
                    command.arg("--script").arg(&script);
                }
            }
            let output = command.output()?;
            assert_eq!(output.status.code(), Some(1));
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("alias"), "{stderr}");
            assert!(output.stdout.is_empty(), "must refuse before prompting");
            assert_eq!(fs::read_to_string(&pending)?, FIX_PENDING_ONE_SPEAKER_ID);
            assert_eq!(fs::read_to_string(&script)?, FIX_SCRIPTED_ACCEPT_SUGGESTED);
        }
    }
    Ok(())
}

/// Missing inputs and identity inspection failures cannot manufacture an
/// admitted destination or create an override file.
#[test]
fn adjudicate_identity_failures_preserve_inputs() -> Result<(), TestError> {
    enum Failure {
        MissingPending,
        MissingScript,
        UninspectableOverrides,
    }
    let harness = CliHarness::new()?;
    for failure in [
        Failure::MissingPending,
        Failure::MissingScript,
        Failure::UninspectableOverrides,
    ] {
        let dir = tempdir()?;
        let pending = dir.path().join("pending.toml");
        let script = dir.path().join("script.toml");
        let overrides = dir.path().join("overrides.toml");
        fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;
        fs::write(&script, FIX_SCRIPTED_ACCEPT_SUGGESTED)?;
        let (pending_arg, script_arg, override_arg) = match failure {
            Failure::MissingPending => (
                dir.path().join("missing.toml"),
                script.clone(),
                overrides.clone(),
            ),
            Failure::MissingScript => (
                pending.clone(),
                dir.path().join("missing.toml"),
                overrides.clone(),
            ),
            Failure::UninspectableOverrides => (
                pending.clone(),
                script.clone(),
                script.join("overrides.toml"),
            ),
        };
        let output = harness
            .chatter_cmd()
            .args(["experimental", "review", "scripted"])
            .arg(pending_arg)
            .arg("--override-file")
            .arg(override_arg)
            .arg("--script")
            .arg(script_arg)
            .arg("--operator")
            .arg("operator")
            .output()?;
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("identity"), "{stderr}");
        assert_eq!(fs::read_to_string(&pending)?, FIX_PENDING_ONE_SPEAKER_ID);
        assert_eq!(fs::read_to_string(&script)?, FIX_SCRIPTED_ACCEPT_SUGGESTED);
        assert!(!overrides.exists());
    }
    Ok(())
}

/// Predictable staging names must not claim or follow existing sibling files.
#[test]
fn adjudicate_preserves_existing_temporary_siblings() -> Result<(), TestError> {
    enum ExistingSibling {
        File,
        HardLink,
        #[cfg(unix)]
        Symlink,
    }
    let harness = CliHarness::new()?;
    for kind in [
        ExistingSibling::File,
        ExistingSibling::HardLink,
        #[cfg(unix)]
        ExistingSibling::Symlink,
    ] {
        let dir = tempdir()?;
        let pending = dir.path().join("pending.toml");
        let overrides = dir.path().join("overrides.toml");
        let scripted = dir.path().join("scripted.toml");
        fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;
        fs::write(&scripted, FIX_SCRIPTED_ACCEPT_SUGGESTED)?;
        let evidence = b"unrelated retained evidence";
        let victim = dir.path().join("retained.txt");
        fs::write(&victim, evidence)?;
        let siblings = [
            pending.with_extension("toml.tmp"),
            overrides.with_extension("toml.tmp"),
        ];
        for sibling in &siblings {
            match kind {
                ExistingSibling::File => fs::write(sibling, evidence)?,
                ExistingSibling::HardLink => fs::hard_link(&victim, sibling)?,
                #[cfg(unix)]
                ExistingSibling::Symlink => std::os::unix::fs::symlink(&victim, sibling)?,
            }
        }
        harness
            .chatter_cmd()
            .args(["experimental", "review", "scripted"])
            .arg(&pending)
            .arg("--override-file")
            .arg(&overrides)
            .arg("--script")
            .arg(&scripted)
            .arg("--operator")
            .arg("operator")
            .assert()
            .success();
        assert_eq!(fs::read(&victim)?, evidence);
        for sibling in &siblings {
            assert_eq!(
                fs::read(sibling)?,
                evidence,
                "existing sibling was changed or removed"
            );
        }
        assert_eq!(
            fs::read_to_string(&scripted)?,
            FIX_SCRIPTED_ACCEPT_SUGGESTED
        );
        assert!(
            talkbank_transform::adjudication::PendingAdjudications::read(&pending)
                .expect("published pending document")
                .entries
                .is_empty()
        );
        assert!(
            talkbank_transform::speaker_id::OverrideFile::read_or_default(&overrides)
                .expect("published overrides")
                .get("session-102-t1")
                .is_some()
        );
    }
    Ok(())
}

/// A complete pending queue cannot silently discard extra scripted decisions.
#[test]
fn adjudicate_unused_script_decisions_preserve_files() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let scripted = dir.path().join("scripted.toml");
    let overrides = dir.path().join("overrides.toml");
    let existing = "# retain this evidence on refusal\nschema_version = 2\n";
    let decision = FIX_SCRIPTED_ACCEPT_SUGGESTED
        .split_once("[[decisions]]")
        .expect("retained decision fixture")
        .1;
    let trailing = format!("{FIX_SCRIPTED_ACCEPT_SUGGESTED}\n[[decisions]]{decision}");
    for (requests, script) in [
        (FIX_PENDING_ONE_SPEAKER_ID, trailing.as_str()),
        (
            "schema_version = 2\nentries = []\n",
            FIX_SCRIPTED_ACCEPT_SUGGESTED,
        ),
    ] {
        fs::write(&pending, requests)?;
        fs::write(&scripted, script)?;
        fs::write(&overrides, existing)?;
        harness
            .chatter_cmd()
            .args(["experimental", "review", "scripted"])
            .arg(&pending)
            .arg("--override-file")
            .arg(&overrides)
            .arg("--script")
            .arg(&scripted)
            .arg("--operator")
            .arg("operator")
            .assert()
            .code(2)
            .stderr(predicates::str::contains("unconsumed scripted decision"));
        assert_eq!(fs::read_to_string(&pending)?, requests);
        assert_eq!(fs::read_to_string(&overrides)?, existing);
        assert_eq!(fs::read_to_string(&scripted)?, script);
    }
    // No pending requests and no scripted decisions is a completed empty run.
    fs::write(&pending, "schema_version = 2\nentries = []\n")?;
    fs::write(&scripted, "schema_version = 2\ndecisions = []\n")?;
    harness
        .chatter_cmd()
        .args(["experimental", "review", "scripted"])
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--script")
        .arg(&scripted)
        .arg("--operator")
        .arg("operator")
        .assert()
        .success();
    Ok(())
}

/// Script versions are admitted before either persistent document can change.
#[test]
fn adjudicate_script_schema_admission_preserves_files() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let scripted = dir.path().join("scripted.toml");
    let overrides = dir.path().join("overrides.toml");
    let existing = "# retain this evidence on refusal\nschema_version = 2\n";

    for version in ["0", "3", "4294967295", "-1", "\"2\"", "missing"] {
        fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;
        fs::write(&overrides, existing)?;
        let header = match version {
            "missing" => String::new(),
            _ => format!("schema_version = {version}"),
        };
        fs::write(
            &scripted,
            FIX_SCRIPTED_ACCEPT_SUGGESTED.replacen("schema_version = 1", &header, 1),
        )?;
        harness
            .chatter_cmd()
            .args([
                "experimental",
                "review",
                "scripted",
                "--operator",
                "fixture",
            ])
            .arg(&pending)
            .arg("--override-file")
            .arg(&overrides)
            .arg("--script")
            .arg(&scripted)
            .assert()
            .code(1)
            .stderr(predicates::str::contains("scripted-decisions"));
        assert_eq!(fs::read_to_string(&pending)?, FIX_PENDING_ONE_SPEAKER_ID);
        assert_eq!(fs::read_to_string(&overrides)?, existing);
    }

    for version in [1, 2] {
        fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;
        fs::write(&overrides, existing)?;
        fs::write(
            &scripted,
            FIX_SCRIPTED_ACCEPT_SUGGESTED.replacen(
                "schema_version = 1",
                &format!("schema_version = {version}"),
                1,
            ),
        )?;
        harness
            .chatter_cmd()
            .args([
                "experimental",
                "review",
                "scripted",
                "--operator",
                "fixture",
            ])
            .arg(&pending)
            .arg("--override-file")
            .arg(&overrides)
            .arg("--script")
            .arg(&scripted)
            .assert()
            .success();
        let result = talkbank_transform::adjudication::PendingAdjudications::read(&pending)
            .expect("accepted pending output");
        assert!(result.entries.is_empty(), "supported version {version}");
        let result = talkbank_transform::speaker_id::OverrideFile::read_or_default(&overrides)
            .expect("accepted override output");
        assert!(result.get("session-102-t1").is_some());
    }
    Ok(())
}

/// Wire-boundary refusal must not publish a decision lacking a rename role.
#[test]
fn adjudicate_missing_role_preserves_files() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let scripted = dir.path().join("scripted.toml");
    let overrides = dir.path().join("batch.overrides.toml");
    let invalid = FIX_PENDING_ONE_SPEAKER_ID.replace(
        "adult_roles = { PAR1 = { code = \"INV\", tag = \"Investigator\" } }",
        "adult_roles = {}",
    );
    fs::write(&pending, &invalid)?;
    fs::write(&scripted, FIX_SCRIPTED_ACCEPT_SUGGESTED)?;
    harness
        .chatter_cmd()
        .args(["experimental", "review", "scripted"])
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--script")
        .arg(&scripted)
        .arg("--operator")
        .arg("test-fixture")
        .assert()
        .code(2)
        .stderr(predicates::str::contains("invalid decision mapping"));
    assert_eq!(fs::read_to_string(&pending)?, invalid);
    assert!(
        !overrides.exists(),
        "refused override must not be published"
    );
    Ok(())
}

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
        .args(["experimental", "review", "scripted"])
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--script")
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
#[cfg(unix)]
#[test]
fn adjudicate_interactive_accepts_suggested() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let overrides = dir.path().join("batch.overrides.toml");
    fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;

    let status = harness
        .chatter_cmd()
        .args(["experimental", "review", "interactive"])
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--operator")
        .arg("interactive-fixture")
        .at_terminal()
        .answer_after(b"Decision [", b"accept\n")
        .wait();
    assert!(status.success(), "{status:?}");

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
        .args(["experimental", "review", "scripted"])
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--script")
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
#[cfg(unix)]
const FIX_PENDING_PARENT_ROLE: &str = r#"schema_version = 2

[[entries]]
session_id = "session-307-parent"
created_at = "2026-05-27T12:00:00Z"
kind = "parent-role-lookup"
donor_speaker = "PAR"

[entries.speaker_mapping]
PAR = "rename"
"#;

/// `chatter experimental review interactive` parses `choose SPK:CODE:TAG
/// [SPK:CODE:TAG ...] [optional note]` lines for parent-role-lookup
/// pending entries. The operator types the role on stdin; the override
/// file records the chosen role paired with the pre-set speaker mapping.
#[cfg(unix)]
#[test]
fn adjudicate_interactive_chooses_role() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let overrides = dir.path().join("batch.overrides.toml");
    fs::write(&pending, FIX_PENDING_PARENT_ROLE)?;

    let status = harness
        .chatter_cmd()
        .args(["experimental", "review", "interactive"])
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--operator")
        .arg("parent-role-fixture")
        .at_terminal()
        .answer_after(
            b"Decision [",
            b"choose PAR:MOT:Mother contributor data sheet\n",
        )
        .wait();
    assert!(status.success(), "{status:?}");

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
#[cfg(unix)]
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
    let status = harness
        .chatter_cmd()
        .args(["experimental", "review", "interactive"])
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--operator")
        .arg("override-fixture")
        .at_terminal()
        .answer_after(
            b"Decision [",
            b"override PAR0:MOT:Mother PAR0=rename PAR1=drop audio review confirms swap\n",
        )
        .wait();
    assert!(status.success(), "{status:?}");

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

// A retained PTY prevents EOF from racing the operator response.
#[cfg(unix)]
trait TerminalReview {
    fn at_terminal(&self) -> crate::common::terminal::RunningTerminal;
}
#[cfg(unix)]
impl TerminalReview for assert_cmd::Command {
    fn at_terminal(&self) -> crate::common::terminal::RunningTerminal {
        let mut command = std::process::Command::new(self.get_program());
        command.args(self.get_args());
        for (name, value) in self.get_envs() {
            match value {
                Some(value) => {
                    command.env(name, value);
                }
                None => {
                    command.env_remove(name);
                }
            }
        }
        if let Some(directory) = self.get_current_dir() {
            command.current_dir(directory);
        }
        crate::common::terminal::Pty::open()
            .launch(command, crate::common::terminal::TerminalOutput::Terminal)
    }
}

#[test]
fn interactive_review_refuses_a_pipe_without_touching_inputs() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let directory = tempdir()?;
    let pending = directory.path().join("pending.toml");
    let overrides = directory.path().join("overrides.toml");
    fs::write(&pending, FIX_PENDING_ONE_SPEAKER_ID)?;
    harness
        .chatter_cmd()
        .args(["experimental", "review", "interactive"])
        .arg(&pending)
        .arg("--override-file")
        .arg(&overrides)
        .args(["--operator", "fixture"])
        .write_stdin("accept\n")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("requires a terminal"));
    assert_eq!(fs::read_to_string(&pending)?, FIX_PENDING_ONE_SPEAKER_ID);
    assert!(!overrides.exists());
    Ok(())
}
