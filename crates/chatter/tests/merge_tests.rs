// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! Integration tests for `chatter merge` and `chatter speaker-id`.
//!
//! These tests follow the TDD authoring sequence in
//! `book/src/architecture/merge-test-plan.md`. Each test is a real
//! user-observable behavior; subprocess-driven via `assert_cmd`.
//!
//! Phase A cycle 1: `merge_basic_smoke`, the simplest possible
//! end-to-end check that `chatter merge` exists, accepts two
//! positional inputs + `--retain`, and produces an output file
//! containing the merged contents of both inputs.

use predicates::prelude::*;
use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

// ============================================================================
// Fixtures, Phase A cycle 1
// ============================================================================
//
// FIX_REF_TWO_UTT_NO_MARKUP is the minimal valid CHAT pair for the
// smoke test: two CHI utterances, plain words, time bullets, no markup
// beyond a clean terminator. FIX_ASR_LABELED_TWO_UTT is its companion:
// two INV utterances at different time positions, already labeled (i.e.
// post-speaker-id form, so `chatter merge` doesn't need to call
// speaker-id transitively).
//
// The bullets use the canonical NAK-delimited CHAT form (`\x15...\x15`).

const FIX_REF_TWO_UTT_NO_MARKUP: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@Media:\tsmoke, audio
*CHI:\thello world . \u{15}0_1000\u{15}
*CHI:\tgoodbye now . \u{15}3000_4000\u{15}
@End
";

const FIX_ASR_LABELED_TWO_UTT: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tsmoke, audio
*INV:\twhat did you say . \u{15}1000_2000\u{15}
*INV:\tsee you later . \u{15}4000_5000\u{15}
@End
";

// ============================================================================
// Phase A, cycle 1
// ============================================================================

/// `chatter merge` end-to-end smoke test.
///
/// Given two minimal valid CHAT files covering the same media and a
/// `--retain CHI` flag, `chatter merge` must:
///  - exit 0
///  - write an output file at the path given by `-o`
///  - that file must contain both CHI utterances (byte-stable from
///    File 1) and both INV utterances (from File 2)
///  - utterances must appear in start-time order
///
/// This is the **smallest possible** end-to-end test exercising every
/// layer (CLI parsing → parser → transform → serialization → file I/O).
/// More precise invariants (byte-stable preservation of disfluency
/// markup, tier stripping, header reconciliation rules, exit-code
/// semantics on each precondition) are covered by later cycles per the
/// authoring sequence.
#[test]
fn merge_basic_smoke() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file1 = dir.path().join("ref.cha");
    let file2 = dir.path().join("asr.cha");
    let out = dir.path().join("merged.cha");
    fs::write(&file1, FIX_REF_TWO_UTT_NO_MARKUP)?;
    fs::write(&file2, FIX_ASR_LABELED_TWO_UTT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("merge")
        .arg(&file1)
        .arg(&file2)
        .arg("--retain")
        .arg("CHI")
        .arg("-o")
        .arg(&out)
        .assert()
        .success();

    let merged = fs::read_to_string(&out)?;

    // Both CHI utterances present (byte-stable from File 1).
    assert!(
        merged.contains("*CHI:\thello world ."),
        "merged output missing first CHI utterance: {merged}"
    );
    assert!(
        merged.contains("*CHI:\tgoodbye now ."),
        "merged output missing second CHI utterance: {merged}"
    );

    // Both INV utterances present (from File 2).
    assert!(
        merged.contains("*INV:\twhat did you say ."),
        "merged output missing first INV utterance: {merged}"
    );
    assert!(
        merged.contains("*INV:\tsee you later ."),
        "merged output missing second INV utterance: {merged}"
    );

    // Utterances appear in start-time order.
    // Expected order by start_ms: CHI@0 < INV@1000 < CHI@3000 < INV@4000.
    let chi1_pos = merged
        .find("*CHI:\thello world .")
        .expect("CHI@0 must be present");
    let inv1_pos = merged
        .find("*INV:\twhat did you say .")
        .expect("INV@1000 must be present");
    let chi2_pos = merged
        .find("*CHI:\tgoodbye now .")
        .expect("CHI@3000 must be present");
    let inv2_pos = merged
        .find("*INV:\tsee you later .")
        .expect("INV@4000 must be present");
    assert!(
        chi1_pos < inv1_pos && inv1_pos < chi2_pos && chi2_pos < inv2_pos,
        "merged utterances not in start-time order: \
         CHI@0={chi1_pos}, INV@1000={inv1_pos}, CHI@3000={chi2_pos}, INV@4000={inv2_pos}"
    );

    // Headers preserved.
    assert!(merged.contains("@Begin"));
    assert!(merged.contains("@End"));
    assert!(merged.contains("@Languages:\teng"));

    // Silence unused-import warning if predicates is unused at this
    // point in the impl; later cycles add stdout/stderr predicates.
    let _ = predicate::str::contains("");
    Ok(())
}

// ============================================================================
// Phase A, cycle 9a, preconditions: RetainSpeakersMissing
// ============================================================================

/// File 1 declares no utterances for any speaker in `--retain`.
///
/// Fixture: File 1 has only `*PAR:` utterances; File 2 has only
/// `*INV:` utterances. Running with `--retain CHI` should refuse
/// rather than emit a degenerate file with no retained content.
const FIX_REF_PAR_ONLY: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|corpus|PAR|||||Participant|||
@Media:\tprecond, audio
*PAR:\tsome utterance . \u{15}0_1000\u{15}
@End
";

const FIX_ASR_INV_PRECOND: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tprecond, audio
*INV:\tasr turn . \u{15}500_1500\u{15}
@End
";

/// `chatter merge` refuses with exit code 2 when File 1 has no
/// utterances for any speaker in the retain set. The stderr message
/// must name the precondition specifically, a future operator
/// reading the output should know *why* the merge refused, not just
/// that it failed.
#[test]
fn merge_no_retain_speakers_in_file1() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file1 = dir.path().join("ref.cha");
    let file2 = dir.path().join("asr.cha");
    let out = dir.path().join("merged.cha");
    fs::write(&file1, FIX_REF_PAR_ONLY)?;
    fs::write(&file2, FIX_ASR_INV_PRECOND)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("merge")
        .arg(&file1)
        .arg(&file2)
        .arg("--retain")
        .arg("CHI")
        .arg("-o")
        .arg(&out)
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("retain").or(predicate::str::contains("CHI")));

    // The output file must NOT have been written.
    assert!(
        !out.exists(),
        "merged output file should not exist on precondition failure"
    );
    Ok(())
}

// ============================================================================
// Phase A, cycle 9b, preconditions: NoTimelineInFile1
// ============================================================================

/// File 1 with retained CHI utterances that lack time bullets. With
/// no bulleted utterance to anchor the shared timeline, the merge
/// cannot order File 2's content meaningfully and must refuse.
const FIX_REF_CHI_NO_BULLETS: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@Media:\tprecond, audio
*CHI:\thello there .
*CHI:\tgoodbye .
@End
";

/// `chatter merge` refuses with exit code 2 when File 1 has no
/// time-bulleted utterances. The merge needs a shared media timeline
/// to position File 2's content; without bullets there is nothing to
/// align against and a silent "merge" would produce a meaningless
/// concatenation of speakers.
#[test]
fn merge_no_timeline_in_file1() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file1 = dir.path().join("ref.cha");
    let file2 = dir.path().join("asr.cha");
    let out = dir.path().join("merged.cha");
    fs::write(&file1, FIX_REF_CHI_NO_BULLETS)?;
    fs::write(&file2, FIX_ASR_INV_PRECOND)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("merge")
        .arg(&file1)
        .arg(&file2)
        .arg("--retain")
        .arg("CHI")
        .arg("-o")
        .arg(&out)
        .assert()
        .failure()
        .code(2)
        .stderr(
            predicate::str::contains("timeline")
                .or(predicate::str::contains("bullet"))
                .or(predicate::str::contains("time-bulleted")),
        );

    assert!(
        !out.exists(),
        "merged output file should not exist on precondition failure"
    );
    Ok(())
}

/// File 1 declares both `*CHI:` (retained) and `*INV:` (an
/// already-attributed clinician turn). File 2's ASR output also
/// attributes utterances to `*INV:`. With `--retain CHI`, the merge
/// has no rule to pick which file's INV utterances to keep, File 1's
/// hand-coded INV vs File 2's ASR INV are two different conventions
/// for the same speaker code. Refuse rather than guess.
const FIX_REF_CHI_PLUS_INV: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, INV Investigator
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tambig, audio
*CHI:\thello there . \u{15}0_1000\u{15}
*INV:\thand-coded clinician turn . \u{15}1500_2500\u{15}
@End
";

const FIX_ASR_INV_AMBIG: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tambig, audio
*INV:\tasr generated clinician turn . \u{15}3000_4000\u{15}
@End
";

/// `chatter merge` refuses with exit code 2 when a non-retained
/// speaker code appears in BOTH files. The user must disambiguate by
/// adding the speaker to `--retain` (preferring File 1's version) or
/// preprocessing File 2 to rename the conflicting code.
#[test]
fn merge_ambiguous_speaker() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file1 = dir.path().join("ref.cha");
    let file2 = dir.path().join("asr.cha");
    let out = dir.path().join("merged.cha");
    fs::write(&file1, FIX_REF_CHI_PLUS_INV)?;
    fs::write(&file2, FIX_ASR_INV_AMBIG)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("merge")
        .arg(&file1)
        .arg(&file2)
        .arg("--retain")
        .arg("CHI")
        .arg("-o")
        .arg(&out)
        .assert()
        .failure()
        .code(2)
        .stderr(
            predicate::str::contains("INV")
                .or(predicate::str::contains("ambiguous"))
                .or(predicate::str::contains("--retain")),
        );

    assert!(
        !out.exists(),
        "merged output file should not exist on precondition failure"
    );
    Ok(())
}

/// File 1 is monolingual English; File 2 is monolingual Cantonese.
/// The merge contract treats `@Languages` as a hard precondition,
/// silently merging across languages would corrupt downstream
/// language-aware tooling (morphotag, alignment, segmentation), so
/// the merge must refuse rather than emit a cross-language file.
const FIX_REF_CHI_ENG: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@Media:\tprecond_lang, audio
*CHI:\thello there . \u{15}0_1000\u{15}
@End
";

const FIX_ASR_INV_YUE: &str = "@UTF8
@Begin
@Languages:\tyue
@Participants:\tINV Investigator
@ID:\tyue|corpus|INV|||||Investigator|||
@Media:\tprecond_lang, audio
*INV:\t你好 . \u{15}500_1500\u{15}
@End
";

/// `chatter merge` refuses with exit code 2 when the two input files'
/// `@Languages` headers disagree. The stderr message must name the
/// precondition specifically so an operator can identify the cause
/// without re-reading either input file.
#[test]
fn merge_language_mismatch() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file1 = dir.path().join("ref.cha");
    let file2 = dir.path().join("asr.cha");
    let out = dir.path().join("merged.cha");
    fs::write(&file1, FIX_REF_CHI_ENG)?;
    fs::write(&file2, FIX_ASR_INV_YUE)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("merge")
        .arg(&file1)
        .arg(&file2)
        .arg("--retain")
        .arg("CHI")
        .arg("-o")
        .arg(&out)
        .assert()
        .failure()
        .code(2)
        .stderr(
            predicate::str::contains("language")
                .or(predicate::str::contains("Languages"))
                .or(predicate::str::contains("@Languages")),
        );

    assert!(
        !out.exists(),
        "merged output file should not exist on precondition failure"
    );
    Ok(())
}
