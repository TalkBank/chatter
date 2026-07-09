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

//! Integration tests for `chatter rediarize`.
//!
//! User contract: `book/src/chatter/user-guide/rediarize.md`.
//! Each test exercises a real user-observable behavior via
//! `assert_cmd` subprocess: given a CHAT file with time bullets and a
//! turns JSON from an external diarizer, utterance speakers are
//! reassigned to the maximum-overlap diarization track.

use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;
use common::CliHarness;

/// Anonymous-2-speaker donor whose SECOND track really holds two
/// different voices across time (the Rev under-count shape): a good
/// diarizer splits `PAR1`'s two utterances into `PAR1` + `PAR2`.
const FIX_UNDERCOUNTED_2SPK: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|corpus|PAR0|||||Participant|||
@ID:\teng|corpus|PAR1|||||Participant|||
@Media:\trediarize_smoke, audio
*PAR0:\thello there . \u{15}0_1000\u{15}
*PAR1:\thi yourself . \u{15}1000_2000\u{15}
*PAR1:\tand goodbye . \u{15}2000_3000\u{15}
@End
";

/// Turns JSON for the fixture above: three distinct voices, the third
/// owning the 2-3s span Rev had lumped into `PAR1`.
const TURNS_THREE_VOICES: &str = r#"{
 "source": "pyannote/speaker-diarization-community-1",
 "turns": [
  {"track": "PAR0", "start_ms": 0, "end_ms": 1000},
  {"track": "PAR1", "start_ms": 1000, "end_ms": 2000},
  {"track": "PAR2", "start_ms": 2000, "end_ms": 3000}
 ]
}"#;

/// `chatter rediarize` end-to-end smoke test.
///
/// Given the under-counted donor and a 3-voice turns file:
///  - exit 0
///  - the third utterance moves from *PAR1: to *PAR2:
///  - the first two utterances keep their tracks
///  - @Participants / @ID reconcile to declare PAR2
///  - the summary reports the reassignment count
#[test]
fn rediarize_splits_merged_track() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("undercounted.cha");
    let turns = dir.path().join("turns.json");
    let out = dir.path().join("rediarized.cha");
    fs::write(&input, FIX_UNDERCOUNTED_2SPK)?;
    fs::write(&turns, TURNS_THREE_VOICES)?;

    let output = harness.run_output(&[
        "rediarize",
        input.to_str().expect("utf-8 temp path"),
        "--turns",
        turns.to_str().expect("utf-8 temp path"),
        "-o",
        out.to_str().expect("utf-8 temp path"),
    ])?;
    common::assert_success(&output, "chatter rediarize");

    let rewritten = fs::read_to_string(&out)?;

    // The third utterance moved to the third voice's track.
    assert!(
        rewritten.contains("*PAR2:\tand goodbye ."),
        "third utterance should be reattributed to PAR2:\n{rewritten}"
    );
    // The first two utterances keep their (correct) tracks.
    assert!(
        rewritten.contains("*PAR0:\thello there ."),
        "first utterance should stay on PAR0:\n{rewritten}"
    );
    assert!(
        rewritten.contains("*PAR1:\thi yourself ."),
        "second utterance should stay on PAR1:\n{rewritten}"
    );

    // Headers reconciled: PAR2 declared in @Participants and @ID.
    let participants_line = rewritten
        .lines()
        .find(|l| l.starts_with("@Participants:"))
        .expect("output missing @Participants header");
    assert!(
        participants_line.contains("PAR2"),
        "@Participants should declare PAR2: {participants_line}"
    );
    assert!(
        rewritten.contains("eng|corpus|PAR2|"),
        "PAR2 should get an @ID row:\n{rewritten}"
    );

    // The summary reports what happened (1 reassigned, 0 flagged).
    let summary = common::combined_output(&output);
    assert!(
        summary.contains("1 reassigned"),
        "summary should report the reassignment count:\n{summary}"
    );

    // The output must remain structurally valid CHAT: the appended
    // @ID row for PAR2 belongs with the other headers, and nothing
    // may follow @End (validate's E501; caught on the first real
    // corpus file, 2026-07-08).
    let last_line = rewritten
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .expect("output should have content");
    assert_eq!(
        last_line, "@End",
        "@End must be the final line; headers must not be appended after it:\n{rewritten}"
    );
    let id_line_numbers: Vec<usize> = rewritten
        .lines()
        .enumerate()
        .filter(|(_, l)| l.starts_with("@ID:"))
        .map(|(n, _)| n)
        .collect();
    let first_utterance_line = rewritten
        .lines()
        .position(|l| l.starts_with('*'))
        .expect("output should contain utterances");
    assert!(
        id_line_numbers.iter().all(|&n| n < first_utterance_line),
        "every @ID row (including the appended PAR2 row) must precede the first utterance:\n{rewritten}"
    );
    Ok(())
}

/// A turns file with an inverted span (`end_ms < start_ms`) is a
/// defective diarization input: the command must refuse it with a
/// diagnostic naming the problem, not silently proceed.
#[test]
fn rediarize_rejects_inverted_span() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("undercounted.cha");
    let turns = dir.path().join("bad-turns.json");
    let out = dir.path().join("never-written.cha");
    fs::write(&input, FIX_UNDERCOUNTED_2SPK)?;
    fs::write(
        &turns,
        r#"{"turns": [{"track": "PAR0", "start_ms": 5000, "end_ms": 4000}]}"#,
    )?;

    let output = harness.run_output(&[
        "rediarize",
        input.to_str().expect("utf-8 temp path"),
        "--turns",
        turns.to_str().expect("utf-8 temp path"),
        "-o",
        out.to_str().expect("utf-8 temp path"),
    ])?;
    common::assert_failure(&output, "chatter rediarize with inverted span");
    assert!(
        !out.exists(),
        "no output file should be written on a rejected turns file"
    );
    let diagnostics = common::combined_output(&output);
    assert!(
        diagnostics.contains("start_ms"),
        "diagnostic should name the inverted span:\n{diagnostics}"
    );
    Ok(())
}

/// Utterances the transform cannot place (no overlapping turn) keep
/// their speaker and are REPORTED, never silently guessed.
#[test]
fn rediarize_reports_flagged_utterances() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("undercounted.cha");
    let turns = dir.path().join("partial-turns.json");
    let out = dir.path().join("rediarized.cha");
    fs::write(&input, FIX_UNDERCOUNTED_2SPK)?;
    // Turns cover only the first second; the two later utterances
    // overlap no turn.
    fs::write(
        &turns,
        r#"{"turns": [{"track": "PAR0", "start_ms": 0, "end_ms": 1000}]}"#,
    )?;

    let output = harness.run_output(&[
        "rediarize",
        input.to_str().expect("utf-8 temp path"),
        "--turns",
        turns.to_str().expect("utf-8 temp path"),
        "-o",
        out.to_str().expect("utf-8 temp path"),
    ])?;
    common::assert_success(&output, "chatter rediarize with partial coverage");

    let summary = common::combined_output(&output);
    assert!(
        summary.contains("2 flagged"),
        "summary should report the flagged count:\n{summary}"
    );
    Ok(())
}
