//! Speaker mapping admission and participant-reference subprocess contracts.

use predicates::prelude::*;
use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

use crate::common::CliHarness;

#[test]
fn replay_missing_decision_file_is_not_an_empty_decision_set() {
    let harness = CliHarness::new().unwrap();
    let dir = tempdir().unwrap();
    let input = dir.path().join("speakers.cha");
    let output = dir.path().join("output.cha");
    fs::write(
        &input,
        include_str!("../../../../corpus/reference/core/headers-speaker-info.cha"),
    )
    .unwrap();
    fs::write(&output, "retained output").unwrap();
    harness
        .chatter_cmd()
        .args(["experimental", "speakers", "replay"])
        .arg(&input)
        .arg("--override-file")
        .arg(dir.path().join("missing.toml"))
        .args(["--session-id", "missing", "-o"])
        .arg(&output)
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("override-file I/O error"));
    assert_eq!(fs::read_to_string(output).unwrap(), "retained output");
}

#[test]
fn speaker_id_invalid_replacements_preserve_output() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("speakers.cha");
    let output = dir.path().join("relabeled.cha");
    let source = fs::read_to_string(
        talkbank_parser_tests::repo_paths::workspace_root()
            .join("corpus/reference/core/headers-speaker-info.cha"),
    )?;
    fs::write(&input, &source)?;
    fs::write(&output, "existing output")?;
    for mapping in [
        "CHI=:Child",
        "CHI=A B:Child",
        "CHI=CHILDREN:Child",
        "CHI=INV:UnknownRole",
        "CHI=INV:Child\n@End",
    ] {
        harness
            .chatter_cmd()
            .args(["experimental", "speakers", "map"])
            .arg(&input)
            .arg("--mapping")
            .arg(mapping)
            .arg("-o")
            .arg(&output)
            .assert()
            .code(2)
            .stdout("")
            .stderr(predicate::str::contains("mapping for CHI"));
        assert_eq!(fs::read_to_string(&output)?, "existing output");
        assert_eq!(fs::read_to_string(&input)?, source);
    }
    Ok(())
}

/// The CLI keeps personal-header references aligned with renamed/dropped speakers.
#[test]
fn speaker_id_personal_headers_follow_assignment() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("speakers.cha");
    let source = fs::read_to_string(
        talkbank_parser_tests::repo_paths::workspace_root()
            .join("corpus/reference/core/headers-speaker-info.cha"),
    )?;
    fs::write(&input, &source)?;
    for (name, mapping, expected) in [
        (
            "rename",
            "CHI=INV:Investigator,MOT=ADU:Adult,F_A_T=PAR:Participant",
            vec![
                "@Birth of INV:\t28-JUN-2001",
                "@Birth of ADU:\t15-MAR-1975",
                "@Birthplace of ADU:\tTaipei, Taiwan",
                "@L1 of PAR:\tara",
            ],
        ),
        (
            "drop",
            "MOT=drop,F_A_T=drop",
            vec!["@Birth of CHI:\t28-JUN-2001"],
        ),
    ] {
        let output = dir.path().join(format!("{name}.cha"));
        harness
            .chatter_cmd()
            .args(["experimental", "speakers", "map"])
            .arg(&input)
            .arg("--mapping")
            .arg(mapping)
            .arg("-o")
            .arg(&output)
            .assert()
            .success();
        let text = fs::read_to_string(&output)?;
        let actual: Vec<_> = text
            .lines()
            .filter(|line| {
                line.starts_with("@Birth of ")
                    || line.starts_with("@Birthplace of ")
                    || line.starts_with("@L1 of ")
            })
            .collect();
        assert_eq!(actual, expected);
        harness
            .chatter_cmd()
            .arg("validate")
            .arg(&output)
            .assert()
            .success();
    }
    assert_eq!(fs::read_to_string(&input)?, source);
    Ok(())
}

#[test]
fn speaker_id_target_collisions_preserve_output() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("speakers.cha");
    let output = dir.path().join("relabeled.cha");
    let source = fs::read_to_string(
        talkbank_parser_tests::repo_paths::workspace_root()
            .join("corpus/reference/core/headers-speaker-info.cha"),
    )?;
    fs::write(&input, &source)?;
    fs::write(&output, "existing output")?;
    for mapping in ["CHI=MOT:Child", "CHI=INV:Child,MOT=INV:Mother"] {
        harness
            .chatter_cmd()
            .args(["experimental", "speakers", "map"])
            .arg(&input)
            .arg("--mapping")
            .arg(mapping)
            .arg("-o")
            .arg(&output)
            .assert()
            .code(2)
            .stdout("")
            .stderr(predicate::str::contains(
                "speaker mapping would assign both CHI and MOT",
            ));
        assert_eq!(fs::read_to_string(&output)?, "existing output");
        assert_eq!(fs::read_to_string(&input)?, source);
    }
    Ok(())
}
