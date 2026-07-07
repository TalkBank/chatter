//! Integration tests for `chatter speaker-id`.
//!
//! User contract: `book/src/chatter/user-guide/speaker-id.md`.
//! Each test exercises a real user-observable behavior via
//! `assert_cmd` subprocess.

use predicates::prelude::*;
use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;
use common::CliHarness;

/// Anonymous-2-speaker donor fixture for the explicit-mapping smoke
/// test. ASR systems commonly emit `PAR0`, `PAR1`, ... as
/// placeholder speaker codes; the operator listens to the audio and
/// supplies the role assignment via `--mapping`.
const FIX_ASR_ANON_2SPK: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|corpus|PAR0|||||Participant|||
@ID:\teng|corpus|PAR1|||||Participant|||
@Media:\tspkid_smoke, audio
*PAR0:\thello there . \u{15}0_1000\u{15}
*PAR1:\twhat did you say . \u{15}1500_2500\u{15}
*PAR0:\tgoodbye . \u{15}3000_4000\u{15}
@End
";

/// `chatter speaker-id` end-to-end smoke test for explicit-mapping
/// mode.
///
/// Given an anonymous-2-speaker donor and `--mapping
/// "PAR0=drop,PAR1=INV:Investigator"`:
///  - exit 0
///  - PAR0 utterances are dropped
///  - PAR1 utterances become *INV:
///  - @Participants reconciles: PAR0 entry gone, PAR1 entry rewritten
///    to INV Investigator
///  - @ID for PAR0 dropped; PAR1 @ID rewritten to INV / Investigator
#[test]
fn speaker_id_explicit_basic() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("asr-anon.cha");
    let out = dir.path().join("relabeled.cha");
    fs::write(&input, FIX_ASR_ANON_2SPK)?;

    harness
        .chatter_cmd()
        .arg("speaker-id")
        .arg(&input)
        .arg("--mapping")
        .arg("PAR0=drop,PAR1=INV:Investigator")
        .arg("-o")
        .arg(&out)
        .assert()
        .success();

    let relabeled = fs::read_to_string(&out)?;

    // PAR0's utterances are dropped (both of them).
    assert!(
        !relabeled.contains("*PAR0:"),
        "relabeled output should contain no *PAR0: lines (PAR0 was dropped):\n{relabeled}"
    );

    // PAR1's utterance is renamed to *INV:.
    assert!(
        !relabeled.contains("*PAR1:"),
        "relabeled output should contain no *PAR1: lines (PAR1 was renamed):\n{relabeled}"
    );
    assert!(
        relabeled.contains("*INV:\twhat did you say ."),
        "relabeled output should contain *INV: (renamed from *PAR1:):\n{relabeled}"
    );

    // @Participants reconciles: no PAR0, no PAR1, has INV.
    let participants_line = relabeled
        .lines()
        .find(|l| l.starts_with("@Participants:"))
        .expect("relabeled output missing @Participants header");
    assert!(
        !participants_line.contains("PAR0"),
        "@Participants should not contain PAR0 (dropped): {participants_line}"
    );
    assert!(
        !participants_line.contains("PAR1"),
        "@Participants should not contain PAR1 (renamed): {participants_line}"
    );
    assert!(
        participants_line.contains("INV"),
        "@Participants should contain INV (PAR1 renamed): {participants_line}"
    );
    assert!(
        participants_line.contains("Investigator"),
        "@Participants should carry INV's role-tag Investigator: {participants_line}"
    );

    // @ID rows: PAR0 dropped, PAR1 rewritten.
    let id_lines: Vec<&str> = relabeled
        .lines()
        .filter(|l| l.starts_with("@ID:"))
        .collect();
    assert_eq!(
        id_lines.len(),
        1,
        "relabeled should have exactly one @ID row (PAR0 dropped). got: {id_lines:?}"
    );
    assert!(
        id_lines[0].contains("|INV|"),
        "remaining @ID row should be INV's. got: {}",
        id_lines[0]
    );
    assert!(
        id_lines[0].contains("|Investigator|"),
        "INV @ID row should carry Investigator role-tag. got: {}",
        id_lines[0]
    );
    Ok(())
}

/// `--session-context` is only consulted by `--judgment holistic`. When the
/// operator configures it on a deterministic run, the CLI must say so on
/// stderr (a warning, not an error: deterministic runs must keep working) so
/// the configured context is not silently ignored.
#[test]
fn speaker_id_deterministic_warns_session_context_ignored() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("asr-anon.cha");
    let context = dir.path().join("session-context.json");
    let out = dir.path().join("relabeled.cha");
    fs::write(&input, FIX_ASR_ANON_2SPK)?;
    fs::write(
        &context,
        r#"{ "asr-anon": { "sample_type": "clinician interview" } }"#,
    )?;

    harness
        .chatter_cmd()
        .arg("speaker-id")
        .arg(&input)
        .arg("--mapping")
        .arg("PAR0=drop,PAR1=INV:Investigator")
        .arg("--session-context")
        .arg(&context)
        .arg("-o")
        .arg(&out)
        .assert()
        .success()
        .stderr(
            predicate::str::contains("--session-context").and(predicate::str::contains("holistic")),
        );

    assert!(
        out.exists(),
        "the deterministic run must still produce its output (warning, not error)"
    );
    Ok(())
}

/// Reference fixture for the CLI reference-mode low-confidence test:
/// CHI describes a "frog jumped in the pond" scene with enough
/// lexical overlap that both donor speakers will partially match.
const FIX_REF_CHI_POND: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.||||Target_Child|||
@Media:\tcli_borderline, audio
*CHI:\tthe frog jumped in the pond . \u{15}0_2000\u{15}
*CHI:\tthe frog is in the pond . \u{15}2000_4000\u{15}
*CHI:\twhere is the frog . \u{15}4000_5500\u{15}
@End
";

/// Donor fixture where BOTH donor speakers share enough vocabulary
/// with the reference that the Jaccard margin sits below the default
/// 2.0× threshold, mirroring the L2 borderline test in
/// `talkbank-transform/tests/speaker_id_tests.rs`.
const FIX_DONOR_BORDERLINE: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|frogstory|PAR0|||||Participant|||
@ID:\teng|frogstory|PAR1|||||Participant|||
@Media:\tcli_borderline, audio
*PAR0:\twhere is the frog now . \u{15}0_1500\u{15}
*PAR1:\tthe frog jumped . \u{15}1500_2500\u{15}
*PAR0:\tyou see the frog . \u{15}2500_3500\u{15}
*PAR1:\tin the pond . \u{15}3500_4500\u{15}
*PAR0:\tthe frog is jumping . \u{15}4500_5500\u{15}
*PAR1:\tthe frog . \u{15}5500_6500\u{15}
@End
";

/// `chatter speaker-id` in reference mode exits with code 4 (not 2)
/// when the winner→runner-up Jaccard margin is below the confidence
/// threshold. Exit 4 is reserved for the adjudication-required
/// outcome so operator pipelines can distinguish "auto-decide
/// refused" from "input invalid" (exit 1) and "precondition
/// violated" (exit 2). Stderr carries the per-speaker scores so the
/// operator can inspect without re-running.
#[test]
fn speaker_id_reference_low_confidence_exits_4() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor = dir.path().join("donor.cha");
    let reference = dir.path().join("ref.cha");
    let out = dir.path().join("relabeled.cha");
    fs::write(&donor, FIX_DONOR_BORDERLINE)?;
    fs::write(&reference, FIX_REF_CHI_POND)?;

    harness
        .chatter_cmd()
        .arg("speaker-id")
        .arg(&donor)
        .arg("--reference")
        .arg(&reference)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("-o")
        .arg(&out)
        .assert()
        .failure()
        .code(4)
        .stderr(
            predicate::str::contains("PAR0")
                .and(predicate::str::contains("PAR1"))
                .and(
                    predicate::str::contains("confidence")
                        .or(predicate::str::contains("margin"))
                        .or(predicate::str::contains("score")),
                ),
        );

    // No output file is written when the auto-decision is refused,
    // the operator must adjudicate before any relabeling happens.
    assert!(
        !out.exists(),
        "low-confidence refusal must not produce an output file"
    );
    Ok(())
}

/// On a low-confidence reference-mode run, `--write-pending` records
/// a `PendingEntry` so the orchestrator can hand it to
/// `chatter adjudicate` in a later step. Exit code is still 4, the
/// pending file is the audit trail of refusals, not a success path.
#[test]
fn speaker_id_reference_writes_pending_on_low_confidence() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor = dir.path().join("donor.cha");
    let reference = dir.path().join("ref.cha");
    let pending = dir.path().join("pending.toml");
    let out = dir.path().join("relabeled.cha");
    fs::write(&donor, FIX_DONOR_BORDERLINE)?;
    fs::write(&reference, FIX_REF_CHI_POND)?;

    harness
        .chatter_cmd()
        .arg("speaker-id")
        .arg(&donor)
        .arg("--reference")
        .arg(&reference)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--write-pending")
        .arg(&pending)
        .arg("-o")
        .arg(&out)
        .assert()
        .failure()
        .code(4);

    // No relabeled output (low confidence = refused).
    assert!(
        !out.exists(),
        "low-confidence refusal must not produce an output file"
    );
    // Pending file exists with the would-have-been-suggested mapping.
    assert!(
        pending.exists(),
        "--write-pending must write the pending file on low confidence"
    );
    let pending_text = fs::read_to_string(&pending)?;
    assert!(
        pending_text.contains("schema_version"),
        "pending file must declare schema_version:\n{pending_text}"
    );
    assert!(
        pending_text.contains("speaker-id-low-confidence"),
        "pending entry must record the kind:\n{pending_text}"
    );
    assert!(
        pending_text.contains("PAR0") && pending_text.contains("PAR1"),
        "pending entry must list both donor speakers in the suggested mapping:\n{pending_text}"
    );
    assert!(
        pending_text.contains("\"drop\"") && pending_text.contains("\"rename\""),
        "suggested mapping must mark winner=drop and other=rename:\n{pending_text}"
    );
    assert!(
        pending_text.contains("\"INV\"") && pending_text.contains("\"Investigator\""),
        "suggested inserted_role must come from --inserted-role:\n{pending_text}"
    );
    Ok(())
}

/// Reference fixture for the clean-winner CLI test: distinct
/// child-frogstory lexicon that one donor speaker will overwhelmingly
/// share and the other will not.
const FIX_REF_CHI_FROG: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.||||Target_Child|||
@Media:\tcli_write_override, audio
*CHI:\twhere did the frog go . \u{15}0_2000\u{15}
*CHI:\tthe frog fell in the jar . \u{15}2500_4500\u{15}
*CHI:\twhere is my frog . \u{15}5000_6500\u{15}
@End
";

/// Donor fixture where PAR0 matches the child's lexicon
/// overwhelmingly (margin ≥ 3.0×) and PAR1 carries clinician-only
/// content. Mirrors the L2 `identify_mapping_clean_winner` fixture.
const FIX_DONOR_CLEAN_WINNER: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|frogstory|PAR0|||||Participant|||
@ID:\teng|frogstory|PAR1|||||Participant|||
@Media:\tcli_write_override, audio
*PAR0:\twhere did the frog go . \u{15}0_2000\u{15}
*PAR1:\ttell me about the picture . \u{15}2000_2500\u{15}
*PAR0:\tthe frog fell in the jar . \u{15}2500_4500\u{15}
*PAR1:\tyes good . \u{15}4500_5000\u{15}
*PAR0:\twhere is my frog . \u{15}5000_6500\u{15}
*PAR1:\tthat is good . \u{15}6500_7000\u{15}
@End
";

/// `chatter speaker-id` in reference mode with `--write-override`
/// appends the auto-decided record to the named override file,
/// creating it if absent. The file is the durable audit trail of
/// batch runs: years later the entry tells a researcher *why* PAR0
/// was dropped and PAR1 became INV, on the basis of which Jaccard
/// scores.
#[test]
fn speaker_id_reference_writes_override() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor = dir.path().join("donor.cha");
    let reference = dir.path().join("ref.cha");
    let overrides = dir.path().join("batch.overrides.toml");
    let out = dir.path().join("relabeled.cha");
    fs::write(&donor, FIX_DONOR_CLEAN_WINNER)?;
    fs::write(&reference, FIX_REF_CHI_FROG)?;

    harness
        .chatter_cmd()
        .arg("speaker-id")
        .arg(&donor)
        .arg("--reference")
        .arg(&reference)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--write-override")
        .arg(&overrides)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();

    // Output file exists with PAR1 renamed to INV (PAR0 dropped).
    assert!(out.exists(), "relabeled output should be written");
    let relabeled = fs::read_to_string(&out)?;
    assert!(
        !relabeled.contains("*PAR0:"),
        "PAR0 utterances should be dropped (anchor match):\n{relabeled}"
    );
    assert!(
        !relabeled.contains("*PAR1:"),
        "PAR1 utterances should be renamed (no *PAR1: left):\n{relabeled}"
    );
    assert!(
        relabeled.contains("*INV:"),
        "PAR1 should be renamed to *INV: per --inserted-role:\n{relabeled}"
    );

    // Override file exists and carries the decision.
    assert!(overrides.exists(), "override file should be written");
    let overrides_text = fs::read_to_string(&overrides)?;
    assert!(
        overrides_text.contains("schema_version"),
        "override file must declare schema_version:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("PAR0") && overrides_text.contains("PAR1"),
        "override file should record both donor speakers in mapping:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"INV\""),
        "override file should record inserted role code INV:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"Investigator\""),
        "override file should record inserted role tag Investigator:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"drop\"") && overrides_text.contains("\"rename\""),
        "override file should record both rename and drop actions:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("\"auto\""),
        "override file should record mode=auto for reference-mode decision:\n{overrides_text}"
    );
    Ok(())
}

/// Pre-existing override-file fixture for the replay test. Records
/// a prior auto-decision for session `donor`: PAR0 was dropped (the
/// anchor match), PAR1 was renamed to INV. This is the kind of
/// entry that a batch orchestrator writes after a successful
/// reference-mode run; the replay path applies it verbatim without
/// re-running the Jaccard step.
const FIX_OVERRIDE_FILE: &str = r#"schema_version = 2

[donor]
mode = "auto"
adult_roles = { PAR1 = { code = "INV", tag = "Investigator" } }
mapping = { PAR0 = "drop", PAR1 = "rename" }
operator = "fixture"
decided_at = "2026-01-01T00:00:00Z"
"#;

/// Donor fixture (anonymous 2-speaker ASR output). Reused from the
/// explicit-mode smoke test; replay produces the identical mapping
/// without invoking the Jaccard algorithm.
const FIX_DONOR_REPLAY: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|corpus|PAR0|||||Participant|||
@ID:\teng|corpus|PAR1|||||Participant|||
@Media:\tcli_replay, audio
*PAR0:\tanchor turn one . \u{15}0_1000\u{15}
*PAR1:\tclinician turn . \u{15}1500_2500\u{15}
*PAR0:\tanchor turn two . \u{15}3000_4000\u{15}
@End
";

/// `chatter speaker-id` in override-file mode applies the recorded
/// decision for the named session: PAR0 utterances are dropped,
/// PAR1 utterances are renamed to *INV:. No reference file, no
/// Jaccard work, the prior adjudication is the source of truth.
#[test]
fn speaker_id_override_file_replay() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor = dir.path().join("donor.cha");
    let overrides = dir.path().join("batch.overrides.toml");
    let out = dir.path().join("relabeled.cha");
    fs::write(&donor, FIX_DONOR_REPLAY)?;
    fs::write(&overrides, FIX_OVERRIDE_FILE)?;

    harness
        .chatter_cmd()
        .arg("speaker-id")
        .arg(&donor)
        .arg("--override-file")
        .arg(&overrides)
        .arg("--session-id")
        .arg("donor")
        .arg("-o")
        .arg(&out)
        .assert()
        .success();

    assert!(out.exists(), "relabeled output should be written");
    let relabeled = fs::read_to_string(&out)?;
    assert!(
        !relabeled.contains("*PAR0:"),
        "PAR0 utterances should be dropped per replay:\n{relabeled}"
    );
    assert!(
        !relabeled.contains("*PAR1:"),
        "PAR1 utterances should be renamed (no *PAR1: left):\n{relabeled}"
    );
    assert!(
        relabeled.contains("*INV:\tclinician turn ."),
        "PAR1 should be renamed to *INV: per replay's inserted_role:\n{relabeled}"
    );

    let participants_line = relabeled
        .lines()
        .find(|l| l.starts_with("@Participants:"))
        .expect("relabeled output missing @Participants header");
    assert!(
        !participants_line.contains("PAR0") && !participants_line.contains("PAR1"),
        "@Participants should not contain PAR0/PAR1 after replay: {participants_line}"
    );
    assert!(
        participants_line.contains("INV"),
        "@Participants should contain INV after replay: {participants_line}"
    );
    Ok(())
}
