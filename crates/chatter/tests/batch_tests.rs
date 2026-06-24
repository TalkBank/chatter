//! Integration tests for `chatter batch`.
//!
//! User contract: the batch driver loops `chatter pipeline` over
//! a directory of matched donor/reference pairs. Per-session
//! outcomes are aggregated; low-confidence refusals don't abort
//! the batch.

use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;
use common::CliHarness;

/// Reference fixture for the batch smoke test.
const FIX_REF_CHI_FROG: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.||||Target_Child|||
@Media:\tbatch_smoke, audio
*CHI:\twhere did the frog go . \u{15}0_2000\u{15}
*CHI:\tthe frog fell in the jar . \u{15}2500_4500\u{15}
*CHI:\twhere is my frog . \u{15}5000_6500\u{15}
@End
";

/// Donor fixture for the batch smoke test (clean winner).
const FIX_DONOR_CLEAN_WINNER: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|frogstory|PAR0|||||Participant|||
@ID:\teng|frogstory|PAR1|||||Participant|||
@Media:\tbatch_smoke, audio
*PAR0:\twhere did the frog go . \u{15}0_2000\u{15}
*PAR1:\ttell me about the picture . \u{15}2000_2500\u{15}
*PAR0:\tthe frog fell in the jar . \u{15}2500_4500\u{15}
*PAR1:\tyes good . \u{15}4500_5000\u{15}
*PAR0:\twhere is my frog . \u{15}5000_6500\u{15}
*PAR1:\tthat is good . \u{15}6500_7000\u{15}
@End
";

/// `chatter batch` loops `chatter pipeline` over matched donor/
/// reference pairs by basename. A single-session clean-winner
/// batch produces one merged output file and exits 0.
#[test]
fn batch_pass1_single_session() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor_dir = dir.path().join("donor");
    let ref_dir = dir.path().join("ref");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&donor_dir)?;
    fs::create_dir_all(&ref_dir)?;
    fs::create_dir_all(&out_dir)?;
    fs::write(donor_dir.join("session-N203.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("session-N203.cha"), FIX_REF_CHI_FROG)?;

    harness
        .chatter_cmd()
        .arg("batch")
        .arg(&donor_dir)
        .arg(&ref_dir)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--retain")
        .arg("CHI")
        .arg("-o")
        .arg(&out_dir)
        .assert()
        .success();

    let merged_path = out_dir.join("session-N203.cha");
    assert!(
        merged_path.exists(),
        "batch should produce a merged file for the matched session: {}",
        merged_path.display()
    );
    let merged = fs::read_to_string(&merged_path)?;
    // Sanity-check the merged output: CHI from reference + INV from
    // relabeled donor, no anonymous codes left.
    assert!(
        merged.contains("*CHI:") && merged.contains("*INV:"),
        "merged output should contain both CHI and INV utterances:\n{merged}"
    );
    assert!(
        !merged.contains("*PAR0:") && !merged.contains("*PAR1:"),
        "merged output should not contain anonymous donor codes:\n{merged}"
    );
    Ok(())
}

/// Borderline reference fixture, same lexicon as the clean-winner
/// reference, but the donor's PAR0 and PAR1 both partially match,
/// so the Jaccard margin falls below the default 2.0× threshold.
const FIX_REF_CHI_POND: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.||||Target_Child|||
@Media:\tbatch_borderline, audio
*CHI:\tthe frog jumped in the pond . \u{15}0_2000\u{15}
*CHI:\tthe frog is in the pond . \u{15}2000_4000\u{15}
*CHI:\twhere is the frog . \u{15}4000_5500\u{15}
@End
";

/// Donor where BOTH PAR0 and PAR1 share substantial vocabulary
/// with the reference, the "Frog Where Are You?" pattern where
/// clinician + child describe the same scene.
const FIX_DONOR_BORDERLINE: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|frogstory|PAR0|||||Participant|||
@ID:\teng|frogstory|PAR1|||||Participant|||
@Media:\tbatch_borderline, audio
*PAR0:\twhere is the frog now . \u{15}0_1500\u{15}
*PAR1:\tthe frog jumped . \u{15}1500_2500\u{15}
*PAR0:\tyou see the frog . \u{15}2500_3500\u{15}
*PAR1:\tin the pond . \u{15}3500_4500\u{15}
*PAR0:\tthe frog is jumping . \u{15}4500_5500\u{15}
*PAR1:\tthe frog . \u{15}5500_6500\u{15}
@End
";

/// `chatter batch` with mixed outcomes: one clean-winner session
/// auto-decides and lands in the output directory; one borderline
/// session refuses (low confidence) and lands in the pending file.
/// The batch driver itself exits 0, "every matched session produced
/// an outcome", even though one outcome is "needs adjudication."
/// This is the operator's primary workflow: pass 1 over a batch,
/// then `chatter adjudicate` over the aggregated pending file.
#[test]
fn batch_mixed_outcomes() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor_dir = dir.path().join("donor");
    let ref_dir = dir.path().join("ref");
    let out_dir = dir.path().join("out");
    let pending = dir.path().join("pending.toml");
    fs::create_dir_all(&donor_dir)?;
    fs::create_dir_all(&ref_dir)?;
    fs::create_dir_all(&out_dir)?;

    // Session A: clean winner.
    fs::write(donor_dir.join("session-A.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("session-A.cha"), FIX_REF_CHI_FROG)?;
    // Session B: borderline (low confidence).
    fs::write(donor_dir.join("session-B.cha"), FIX_DONOR_BORDERLINE)?;
    fs::write(ref_dir.join("session-B.cha"), FIX_REF_CHI_POND)?;

    harness
        .chatter_cmd()
        .arg("batch")
        .arg(&donor_dir)
        .arg(&ref_dir)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--retain")
        .arg("CHI")
        .arg("--write-pending")
        .arg(&pending)
        .arg("-o")
        .arg(&out_dir)
        .assert()
        // Batch-level success: every session produced an outcome.
        // The pending-needing-adjudication session is not an error.
        .success();

    // Clean-winner session has a merged output.
    let merged_a = out_dir.join("session-A.cha");
    assert!(
        merged_a.exists(),
        "clean-winner session should produce a merged file: {}",
        merged_a.display()
    );

    // Borderline session does NOT have a merged output (refused).
    let merged_b = out_dir.join("session-B.cha");
    assert!(
        !merged_b.exists(),
        "borderline session should not produce a merged file: {}",
        merged_b.display()
    );

    // Pending file carries the borderline session's entry.
    assert!(pending.exists(), "--write-pending file should exist");
    let pending_text = fs::read_to_string(&pending)?;
    assert!(
        pending_text.contains("session-B"),
        "pending file should carry the borderline session:\n{pending_text}"
    );
    assert!(
        !pending_text.contains("session-A"),
        "pending file should NOT carry the clean-winner session:\n{pending_text}"
    );
    assert!(
        pending_text.contains("speaker-id-low-confidence"),
        "pending entry should carry the kind discriminator:\n{pending_text}"
    );
    Ok(())
}

/// Pre-existing override file with a resolved decision for the
/// borderline session. Models the post-adjudication state: the
/// operator ran `chatter adjudicate` on the pass-1 pending file
/// and the decision was recorded here.
const FIX_OVERRIDE_RESOLVED_SESSION_B: &str = r#"schema_version = 1

[session-B]
mode = "explicit"
inserted_role = { code = "INV", tag = "Investigator" }
mapping = { PAR0 = "drop", PAR1 = "rename" }
operator = "fixture-operator"
decided_at = "2026-05-28T11:00:00Z"
"#;

/// `chatter batch --override-file OVERRIDES` re-processes sessions
/// that have a recorded decision via the override-file replay path,
/// while still using reference mode for sessions without an entry.
/// This is the "pass 2" workflow: after `chatter adjudicate`
/// resolves the pending entries, the operator re-runs the same
/// batch command and the previously-skipped sessions now produce
/// merged outputs.
#[test]
fn batch_pass2_replay() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor_dir = dir.path().join("donor");
    let ref_dir = dir.path().join("ref");
    let out_dir = dir.path().join("out");
    let overrides = dir.path().join("batch.overrides.toml");
    fs::create_dir_all(&donor_dir)?;
    fs::create_dir_all(&ref_dir)?;
    fs::create_dir_all(&out_dir)?;

    // Session A (clean winner), no override entry, goes through
    // reference mode.
    fs::write(donor_dir.join("session-A.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("session-A.cha"), FIX_REF_CHI_FROG)?;
    // Session B (borderline), has an override entry from a prior
    // adjudication; goes through replay mode.
    fs::write(donor_dir.join("session-B.cha"), FIX_DONOR_BORDERLINE)?;
    fs::write(ref_dir.join("session-B.cha"), FIX_REF_CHI_POND)?;
    fs::write(&overrides, FIX_OVERRIDE_RESOLVED_SESSION_B)?;

    harness
        .chatter_cmd()
        .arg("batch")
        .arg(&donor_dir)
        .arg(&ref_dir)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--retain")
        .arg("CHI")
        .arg("--override-file")
        .arg(&overrides)
        .arg("-o")
        .arg(&out_dir)
        .assert()
        .success();

    // Both sessions now have merged outputs, session-A via
    // reference mode, session-B via override-file replay.
    let merged_a = out_dir.join("session-A.cha");
    let merged_b = out_dir.join("session-B.cha");
    assert!(
        merged_a.exists(),
        "clean-winner session-A should still produce a merged file via reference mode"
    );
    assert!(
        merged_b.exists(),
        "borderline session-B should now produce a merged file via override-file replay: {}",
        merged_b.display()
    );

    // Session-B's merged output reflects the override entry's
    // mapping: PAR0 dropped (the operator's verified anchor),
    // PAR1 renamed to INV.
    let merged_b_text = fs::read_to_string(&merged_b)?;
    assert!(
        merged_b_text.contains("*INV:")
            && !merged_b_text.contains("*PAR0:")
            && !merged_b_text.contains("*PAR1:"),
        "session-B merged should have INV (renamed PAR1) and no anonymous codes:\n{merged_b_text}"
    );
    Ok(())
}

/// Sentinel content for `--skip-existing` test. Distinguishable
/// from any actual merged-CHAT output so the assertion can verify
/// "this pre-existing file was preserved" vs "this file was
/// overwritten by the batch."
const SENTINEL_PREEXISTING: &str = "@@SENTINEL_PREEXISTING@@\n";

/// `chatter batch --skip-existing` preserves output files that
/// already exist in the output directory, processing only sessions
/// whose merged output is missing. Lets an operator resume an
/// interrupted batch or add new donors without redoing finished
/// work.
#[test]
fn batch_skip_existing() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor_dir = dir.path().join("donor");
    let ref_dir = dir.path().join("ref");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&donor_dir)?;
    fs::create_dir_all(&ref_dir)?;
    fs::create_dir_all(&out_dir)?;

    // Two clean-winner sessions. Pre-populate session-A's output
    // with the sentinel, the batch should leave it alone.
    fs::write(donor_dir.join("session-A.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("session-A.cha"), FIX_REF_CHI_FROG)?;
    fs::write(donor_dir.join("session-B.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("session-B.cha"), FIX_REF_CHI_FROG)?;
    fs::write(out_dir.join("session-A.cha"), SENTINEL_PREEXISTING)?;

    harness
        .chatter_cmd()
        .arg("batch")
        .arg(&donor_dir)
        .arg(&ref_dir)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--retain")
        .arg("CHI")
        .arg("--skip-existing")
        .arg("-o")
        .arg(&out_dir)
        .assert()
        .success();

    // session-A's output was preserved (sentinel still present).
    let merged_a = fs::read_to_string(out_dir.join("session-A.cha"))?;
    assert_eq!(
        merged_a, SENTINEL_PREEXISTING,
        "--skip-existing must preserve the pre-existing session-A output"
    );

    // session-B was processed because no prior output existed.
    let merged_b_path = out_dir.join("session-B.cha");
    assert!(
        merged_b_path.exists(),
        "session-B should be processed because no prior output existed"
    );
    let merged_b = fs::read_to_string(&merged_b_path)?;
    assert!(
        merged_b.contains("*CHI:") && merged_b.contains("*INV:"),
        "session-B output should be the normal merged form:\n{merged_b}"
    );
    Ok(())
}

/// `chatter batch --write-override OVERRIDES` writes one override
/// entry per clean-winner session, recording the algorithm's
/// auto-decision (mapping, scores, margin). This is the audit-trail
/// half of the pass-1 workflow: low-confidence sessions go to
/// `--write-pending` for adjudication; clean-winner sessions go to
/// `--write-override` for sanity-scan + future re-runs.
#[test]
fn batch_writes_override_for_auto_decisions() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor_dir = dir.path().join("donor");
    let ref_dir = dir.path().join("ref");
    let out_dir = dir.path().join("out");
    let overrides_path = dir.path().join("overrides.toml");
    fs::create_dir_all(&donor_dir)?;
    fs::create_dir_all(&ref_dir)?;
    fs::create_dir_all(&out_dir)?;

    // Two clean-winner sessions, both should produce auto-decision
    // entries in the override file.
    fs::write(donor_dir.join("session-A.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("session-A.cha"), FIX_REF_CHI_FROG)?;
    fs::write(donor_dir.join("session-B.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("session-B.cha"), FIX_REF_CHI_FROG)?;

    harness
        .chatter_cmd()
        .arg("batch")
        .arg(&donor_dir)
        .arg(&ref_dir)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--retain")
        .arg("CHI")
        .arg("--write-override")
        .arg(&overrides_path)
        .arg("-o")
        .arg(&out_dir)
        .assert()
        .success();

    // Both sessions produced merged outputs.
    assert!(out_dir.join("session-A.cha").exists());
    assert!(out_dir.join("session-B.cha").exists());

    // The override file exists and carries both auto-decisions with
    // mode = "auto".
    assert!(
        overrides_path.exists(),
        "--write-override should produce an override file: {}",
        overrides_path.display()
    );
    let overrides_text = fs::read_to_string(&overrides_path)?;
    assert!(
        overrides_text.contains("session-A"),
        "override file should carry session-A entry:\n{overrides_text}"
    );
    assert!(
        overrides_text.contains("session-B"),
        "override file should carry session-B entry:\n{overrides_text}"
    );
    // Both should be auto-mode (the algorithm signed off, not the
    // operator).
    let auto_count = overrides_text.matches("mode = \"auto\"").count();
    assert_eq!(
        auto_count, 2,
        "both clean-winner sessions should record as mode=auto; got:\n{overrides_text}"
    );
    // The inserted_role rides through.
    assert!(
        overrides_text.contains("INV") && overrides_text.contains("Investigator"),
        "override file should carry the inserted_role:\n{overrides_text}"
    );
    // Provenance foundation: every auto-decision is stamped
    // engine = "deterministic", so a downstream audit can tell
    // LLM-decided from deterministically-decided sessions without
    // re-running the pipeline.
    let engine_count = overrides_text.matches("engine = \"deterministic\"").count();
    assert_eq!(
        engine_count, 2,
        "both auto-decision entries should be stamped engine=deterministic; got:\n{overrides_text}"
    );
    Ok(())
}

/// Reference fixture with a *long-utterance* CHI, the misclassified
/// case the sanity-scan heuristic is designed to catch (anchor mean
/// utterance words exceeding inserted mean by ≥ threshold).
const FIX_REF_CHI_LONG: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|fakecorpus|CHI|3;06.||||Target_Child|||
@Media:\tsanity_smoke, audio
*CHI:\tI went to school yesterday and had a nice lunch with my best friend . \u{15}0_2000\u{15}
*CHI:\tthe dog ran across the field very quickly today and then jumped the fence . \u{15}2500_4500\u{15}
*CHI:\twe played outside for a long time before going home to have dinner . \u{15}5000_7000\u{15}
@End
";

/// Donor designed to pass-1 clean-winner against `FIX_REF_CHI_LONG`
/// while leaving a short-utterance INV after merge. PAR0 overlaps
/// the reference's content tokens → drop. PAR1 has short distinct
/// utterances → rename to INV. Post-merge MLU ratio is ~13×, well
/// above the 1.5× scan threshold.
const FIX_DONOR_INVERTED_MLU: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|fakecorpus|PAR0|||||Participant|||
@ID:\teng|fakecorpus|PAR1|||||Participant|||
@Media:\tsanity_smoke, audio
*PAR0:\tI went to school yesterday and had lunch with my friend . \u{15}0_2000\u{15}
*PAR0:\tthe dog ran across the field today . \u{15}2500_4500\u{15}
*PAR0:\twe played outside before going home . \u{15}5000_7000\u{15}
*PAR1:\tdog . \u{15}7100_7300\u{15}
*PAR1:\tyes . \u{15}7400_7500\u{15}
*PAR1:\tmore ? \u{15}7600_7700\u{15}
*PAR1:\twhat . \u{15}7800_7900\u{15}
@End
";

/// `chatter batch --sanity-scan` runs the per-session pipeline as
/// usual, then post-loop scans the merged outputs against the
/// audit-trailed override entries. Sessions whose merged file shows
/// anchor-vs-inserted MLU asymmetry get appended to the pending
/// file as `sanity-scan-misclassification` entries.
#[test]
fn batch_with_sanity_scan_flag_flags_inverted_mlu() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor_dir = dir.path().join("donor");
    let ref_dir = dir.path().join("ref");
    let out_dir = dir.path().join("out");
    let overrides_path = dir.path().join("overrides.toml");
    let pending_path = dir.path().join("pending.toml");
    fs::create_dir_all(&donor_dir)?;
    fs::create_dir_all(&ref_dir)?;
    fs::create_dir_all(&out_dir)?;

    // Session normal: clean-winner with no MLU asymmetry (existing
    // fixture pair). Session misclass: clean-winner BUT with the
    // inverted-MLU shape the scan should catch.
    fs::write(donor_dir.join("session-normal.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("session-normal.cha"), FIX_REF_CHI_FROG)?;
    fs::write(
        donor_dir.join("session-misclass.cha"),
        FIX_DONOR_INVERTED_MLU,
    )?;
    fs::write(ref_dir.join("session-misclass.cha"), FIX_REF_CHI_LONG)?;

    harness
        .chatter_cmd()
        .arg("batch")
        .arg(&donor_dir)
        .arg(&ref_dir)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--retain")
        .arg("CHI")
        .arg("--write-override")
        .arg(&overrides_path)
        .arg("--write-pending")
        .arg(&pending_path)
        .arg("--sanity-scan")
        // Tighter than default 1.5×; the normal fixture sits at
        // ~1.5× by accident and would false-positive otherwise. The
        // misclass session sits at ~13× and trips this easily.
        .arg("--sanity-scan-threshold")
        .arg("2.0")
        .arg("-o")
        .arg(&out_dir)
        .assert()
        // Exit 4 mirrors sanity-scan's "completed but flagged" code.
        // The pipeline loop itself succeeded; the scan flagged one
        // session that needs adjudication.
        .code(4);

    // Both sessions produced merged outputs.
    assert!(out_dir.join("session-normal.cha").exists());
    assert!(out_dir.join("session-misclass.cha").exists());

    // Override file carries both auto-decisions.
    let overrides_text = fs::read_to_string(&overrides_path)?;
    assert!(overrides_text.contains("session-normal"));
    assert!(overrides_text.contains("session-misclass"));

    // Pending file carries the misclass session as a sanity-scan
    // entry; the normal session is NOT flagged.
    let pending_text = fs::read_to_string(&pending_path)?;
    assert!(
        pending_text.contains("sanity-scan-misclassification"),
        "pending file should carry the sanity-scan kind discriminator:\n{pending_text}"
    );
    assert!(
        pending_text.contains("session-misclass"),
        "pending file should flag the inverted-MLU session:\n{pending_text}"
    );
    assert!(
        !pending_text.contains("session-normal"),
        "pending file should NOT flag the symmetric-MLU session:\n{pending_text}"
    );
    Ok(())
}

/// Validate-clean reference (corrected `@ID`: `Target_Child` in the
/// 8th/role field via four pipes after the age). Distinct from
/// `FIX_REF_CHI_FROG`, whose `@ID` had only three pipes and so fails
/// `chatter validate` (E515/E546).
const FIX_REF_CHI_VALID: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.||||Target_Child|||
@Media:\tbatch_smoke, audio
*CHI:\twhere did the frog go . \u{15}0_2000\u{15}
*CHI:\tthe frog fell in the jar . \u{15}2500_4500\u{15}
*CHI:\twhere is my frog . \u{15}5000_6500\u{15}
@End
";

/// Invalid reference: missing space before the `[/]` retracing
/// bracket (`the[/]`), which `chatter validate` rejects with E375.
/// A real error class observed in legacy hand-typed transcripts.
const FIX_REF_INVALID_E375: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.||||Target_Child|||
@Media:\tbatch_smoke, audio
*CHI:\tthe[/] frog fell in the jar . \u{15}0_2000\u{15}
@End
";

/// Pre-flight input-validation gate (decided 2026-06-10): `chatter
/// batch` must validate EVERY donor and reference up front and, if
/// any input is invalid CHAT, refuse to do ANY merge work: the
/// whole batch aborts before anything is attempted. A fully-valid
/// session present alongside an invalid reference must therefore NOT
/// be merged. This is distinct from a low-confidence refusal (which
/// does not abort the batch) and from the existing per-session
/// precondition error (which still merged the other sessions):
/// invalid CHAT is cleaned upstream, never fed to the merge.
#[test]
fn batch_aborts_whole_run_when_any_input_invalid() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let donor_dir = dir.path().join("donor");
    let ref_dir = dir.path().join("ref");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&donor_dir)?;
    fs::create_dir_all(&ref_dir)?;
    fs::create_dir_all(&out_dir)?;

    // Fully-valid session: valid donor + validate-clean reference.
    fs::write(donor_dir.join("good.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("good.cha"), FIX_REF_CHI_VALID)?;
    // Session with a valid donor but an INVALID reference (E375).
    fs::write(donor_dir.join("bad.cha"), FIX_DONOR_CLEAN_WINNER)?;
    fs::write(ref_dir.join("bad.cha"), FIX_REF_INVALID_E375)?;

    let output = harness
        .chatter_cmd()
        .arg("batch")
        .arg(&donor_dir)
        .arg(&ref_dir)
        .arg("--anchor")
        .arg("CHI")
        .arg("--inserted-role")
        .arg("INV:Investigator")
        .arg("--retain")
        .arg("CHI")
        .arg("--output")
        .arg(&out_dir)
        .arg("--write-override")
        .arg(dir.path().join("override.toml"))
        .arg("--tui-mode")
        .arg("disable")
        .output()?;

    // Fail closed: any invalid input is a non-zero exit.
    assert!(
        !output.status.success(),
        "batch must fail when any input is invalid CHAT"
    );

    // The invalid file is named in the diagnostics so the operator
    // knows what to clean.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("bad.cha"),
        "batch must report the invalid input file by name:\n{stderr}"
    );

    // Crucially: NO merge work was attempted. Even the fully-valid
    // session must NOT be merged, because validation gates the whole
    // run before anything else.
    assert!(
        !out_dir.join("good.cha").exists(),
        "no merged output may be written when any input is invalid (whole-batch abort)"
    );
    assert!(
        !out_dir.join("bad.cha").exists(),
        "invalid-reference session must not be merged"
    );
    Ok(())
}
