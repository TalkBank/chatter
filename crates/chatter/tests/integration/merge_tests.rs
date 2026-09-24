//! Removed structural workflow commands must refuse before reading or writing.
use predicates::prelude::*;

const REMOVED: &[&str] = &["merge", "pipeline", "batch"];

#[test]
fn removed_workflow_commands_are_rejected_before_io() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let output = dir.path().join("merged.cha");
    for command in REMOVED {
        crate::common::chatter_cmd()
            .args([*command, "missing-reference.cha", "missing-donor.cha", "-o"])
            .arg(&output)
            .assert()
            .code(2)
            .stderr(predicate::str::contains(format!(
                "unrecognized subcommand '{command}'"
            )));
        assert!(!output.exists());
    }
    Ok(())
}

#[test]
fn removed_workflow_commands_are_absent_from_help() {
    let output = crate::common::chatter_cmd()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8_lossy(&output);
    for command in REMOVED {
        assert!(
            !help
                .lines()
                .any(|line| line.split_whitespace().next() == Some(command))
        );
    }
}
