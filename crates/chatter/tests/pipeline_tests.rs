//! Integration tests for `chatter pipeline`.
//!
//! User contract: `book/src/chatter/user-guide/pipeline.md`
//! (to be written; currently the in-code rustdoc on `run_pipeline`
//! is authoritative). The pipeline subcommand is the per-session
//! end-to-end shortcut for the common case `speaker-id → merge`.

use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;
use common::CliHarness;

/// Reference fixture: monolingual English CHI with a distinctive
/// frog-narrative lexicon.
const FIX_REF_CHI_FROG: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.||||Target_Child|||
@Media:\tpipeline_smoke, audio
*CHI:\twhere did the frog go . \u{15}0_2000\u{15}
*CHI:\tthe frog fell in the jar . \u{15}2500_4500\u{15}
*CHI:\twhere is my frog . \u{15}5000_6500\u{15}
@End
";

/// Donor fixture: anonymous 2-speaker ASR output. PAR0 matches the
/// child's lexicon (CHI in the reference); PAR1 is the
/// clinician.
const FIX_DONOR_CLEAN_WINNER: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|frogstory|PAR0|||||Participant|||
@ID:\teng|frogstory|PAR1|||||Participant|||
@Media:\tpipeline_smoke, audio
*PAR0:\twhere did the frog go . \u{15}0_2000\u{15}
*PAR1:\ttell me about the picture . \u{15}2000_2500\u{15}
*PAR0:\tthe frog fell in the jar . \u{15}2500_4500\u{15}
*PAR1:\tyes good . \u{15}4500_5000\u{15}
*PAR0:\twhere is my frog . \u{15}5000_6500\u{15}
*PAR1:\tthat is good . \u{15}6500_7000\u{15}
@End
";

/// `chatter pipeline` is the per-session end-to-end shortcut: run
/// speaker-id (reference mode) to relabel the donor's anonymous
/// speakers, then merge the relabeled donor with the reference into
/// the final output. One CLI invocation instead of two, the common
/// case for an operator processing a single session manually.
#[test]
fn pipeline_clean_winner_end_to_end() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor = dir.path().join("donor.cha");
    let reference = dir.path().join("ref.cha");
    let out = dir.path().join("merged.cha");
    fs::write(&donor, FIX_DONOR_CLEAN_WINNER)?;
    fs::write(&reference, FIX_REF_CHI_FROG)?;

    harness
        .chatter_cmd()
        .arg("pipeline")
        .arg(&donor)
        .arg(&reference)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--retain")
        .arg("CHI")
        .arg("-o")
        .arg(&out)
        .assert()
        .success();

    assert!(out.exists(), "merged output should be written");
    let merged = fs::read_to_string(&out)?;

    // Reference's CHI utterances survive byte-stably.
    assert!(
        merged.contains("*CHI:\twhere did the frog go ."),
        "merged output should contain CHI utterances from reference:\n{merged}"
    );

    // Donor's PAR1 is renamed to INV (PAR0 was the anchor match,
    // dropped); no donor anonymous codes remain.
    assert!(
        merged.contains("*INV:\ttell me about the picture ."),
        "merged output should contain INV utterances renamed from PAR1:\n{merged}"
    );
    assert!(
        !merged.contains("*PAR0:") && !merged.contains("*PAR1:"),
        "merged output should not contain anonymous donor codes:\n{merged}"
    );

    // @Participants reconciles: CHI (from ref) + INV (from
    // relabeled donor).
    let participants_line = merged
        .lines()
        .find(|l| l.starts_with("@Participants:"))
        .expect("merged output missing @Participants header");
    assert!(
        participants_line.contains("CHI") && participants_line.contains("INV"),
        "@Participants should contain both CHI and INV: {participants_line}"
    );
    assert!(
        !participants_line.contains("PAR0") && !participants_line.contains("PAR1"),
        "@Participants should not contain anonymous codes: {participants_line}"
    );
    Ok(())
}

/// Reference that PARSES cleanly but fails `chatter validate`: a
/// well-formed-but-wrong `@ID` where `Target_Child` lands in the SES
/// field (3 pipes after the age, role field left empty), triggering
/// E515/E546. This is validation-only invalidity, NOT a parse error,
/// so it proves the pre-flight gate runs full validation, not just a
/// parse-success check. NOTE: kept deliberately invalid via field
/// POSITION (a clean 10-field parse), so the recovery-node backstop
/// does not turn it into a parse error (E342). Do NOT run
/// `scripts/cleanup/fix_malformed_chi_id_fixtures.py` over this fixture:
/// it would normalize the `@ID` and silently make the reference valid,
/// breaking this refusal test (the test is the guard if that happens).
const FIX_REF_INVALID_ID: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.|||Target_Child||||
@Media:\tpipeline_smoke, audio
*CHI:\twhere did the frog go . \u{15}0_2000\u{15}
@End
";

/// Pre-flight gate (decided 2026-06-10): `chatter pipeline`
/// validates its donor and reference up front and refuses if either
/// is invalid CHAT, before doing any speaker-id or merge work. This
/// holds even for validation-only invalidity (parseable but failing
/// `chatter validate`); no merged output may be produced.
#[test]
fn pipeline_refuses_validation_invalid_reference() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor = dir.path().join("donor.cha");
    let reference = dir.path().join("ref.cha");
    let out = dir.path().join("merged.cha");
    fs::write(&donor, FIX_DONOR_CLEAN_WINNER)?;
    fs::write(&reference, FIX_REF_INVALID_ID)?;

    let output = harness
        .chatter_cmd()
        .arg("pipeline")
        .arg(&donor)
        .arg(&reference)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--retain")
        .arg("CHI")
        .arg("-o")
        .arg(&out)
        .output()?;

    assert!(
        !output.status.success(),
        "pipeline must refuse a validation-invalid reference"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ref.cha"),
        "pipeline must name the invalid input file:\n{stderr}"
    );
    assert!(
        !out.exists(),
        "no merged output may be written when an input is invalid CHAT"
    );
    Ok(())
}
