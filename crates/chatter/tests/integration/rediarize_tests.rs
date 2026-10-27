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

use crate::common::CliHarness;

#[test]
fn rediarize_refuses_unspellable_tracks_before_output() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let source = include_str!("../../../../corpus/reference/content/media-bullets.cha");
    let input = dir.path().join("media-bullets.cha");
    let turns = dir.path().join("turns.json");
    let output = dir.path().join("output.cha");
    let summary = dir.path().join("summary.json");
    fs::write(&input, source)?;
    let mut failures = Vec::new();
    for track in [
        "", "A B", "CH:I", "A|B", "\n@End", "\u{15}", "CHILDREN", "🦀", "A.B", "A\"B",
    ] {
        fs::write(&output, "retained CHAT output")?;
        fs::write(&summary, "retained summary")?;
        fs::write(
            &turns,
            serde_json::to_vec(&serde_json::json!({"turns":[{
                "track": track, "start_ms": 0, "end_ms": 6000000
            }]}))?,
        )?;
        let result = harness
            .chatter_cmd()
            .args(["experimental", "speakers", "rediarize"])
            .arg(&input)
            .arg("--turns")
            .arg(&turns)
            .arg("-o")
            .arg(&output)
            .arg("--summary-json")
            .arg(&summary)
            .output()?;
        if result.status.code() != Some(2)
            || !String::from_utf8_lossy(&result.stderr).contains("track at index 0")
            || fs::read_to_string(&output)? != "retained CHAT output"
            || fs::read_to_string(&summary)? != "retained summary"
        {
            failures.push(format!(
                "{track:?}: status {:?}, stderr {}",
                result.status.code(),
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        assert_eq!(fs::read_to_string(&input)?, source);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}

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
        "experimental",
        "speakers",
        "rediarize",
        input.to_str().expect("utf-8 temp path"),
        "--turns",
        turns.to_str().expect("utf-8 temp path"),
        "-o",
        out.to_str().expect("utf-8 temp path"),
    ])?;
    crate::common::assert_success(&output, "chatter rediarize");

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
    let summary = crate::common::combined_output(&output);
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

/// `--summary-json` writes a machine-readable outcome summary, the
/// contract batch drivers consume instead of scraping the stderr text.
/// Shape: `{"source": ..., "reassigned": N, "unchanged": N,
/// "flagged": [{"utterance_index": N, "kept_speaker": "...",
/// "reason": "no_bullet" | "no_overlapping_turn"}]}`.
#[test]
fn rediarize_writes_summary_json() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("undercounted.cha");
    let turns = dir.path().join("partial-turns.json");
    let out = dir.path().join("rediarized.cha");
    let summary_path = dir.path().join("summary.json");
    fs::write(&input, FIX_UNDERCOUNTED_2SPK)?;
    // Turns cover only the first second: 1 unchanged (PAR0 already
    // correct), 2 flagged (no overlapping turn), 0 reassigned.
    fs::write(
        &turns,
        r#"{"source": "pyannote/speaker-diarization-community-1",
            "turns": [{"track": "PAR0", "start_ms": 0, "end_ms": 1000}]}"#,
    )?;

    let output = harness.run_output(&[
        "experimental",
        "speakers",
        "rediarize",
        input.to_str().expect("utf-8 temp path"),
        "--turns",
        turns.to_str().expect("utf-8 temp path"),
        "-o",
        out.to_str().expect("utf-8 temp path"),
        "--summary-json",
        summary_path.to_str().expect("utf-8 temp path"),
    ])?;
    crate::common::assert_success(&output, "chatter rediarize --summary-json");

    let summary: serde_json::Value = serde_json::from_str(&fs::read_to_string(&summary_path)?)?;
    assert_eq!(
        summary["source"], "pyannote/speaker-diarization-community-1",
        "summary should carry the turns-file provenance: {summary}"
    );
    assert_eq!(summary["reassigned"], 0, "no reassignments: {summary}");
    assert_eq!(
        summary["unchanged"], 3,
        "unchanged includes flagged (they kept their speaker): {summary}"
    );
    let flagged = summary["flagged"]
        .as_array()
        .expect("flagged should be an array");
    assert_eq!(
        flagged.len(),
        2,
        "two utterances overlap no turn: {summary}"
    );
    assert_eq!(flagged[0]["utterance_index"], 1, "{summary}");
    assert_eq!(flagged[0]["kept_speaker"], "PAR1", "{summary}");
    assert_eq!(flagged[0]["reason"], "no_overlapping_turn", "{summary}");
    Ok(())
}

/// The JSON boundary reports union-held time for repeated turns from one
/// track. This is a wire-format measurement: the ordered timeline type keeps
/// callers from bypassing ordering, but no type can prohibit a diarizer from
/// legitimately emitting overlapping segments for the same speaker.
#[test]
fn rediarize_counts_same_track_overlap_once_in_summary_json() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("overlapping-turns.cha");
    let turns = dir.path().join("overlapping-turns.json");
    let out = dir.path().join("rediarized.cha");
    let summary_path = dir.path().join("summary.json");
    fs::write(&input, FIX_UNDERCOUNTED_2SPK)?;
    fs::write(
        &turns,
        r#"{"turns": [
            {"track": "PAR0", "start_ms": 0, "end_ms": 1000},
            {"track": "PAR2", "start_ms": 1000, "end_ms": 1600},
            {"track": "PAR2", "start_ms": 1400, "end_ms": 1800},
            {"track": "PAR1", "start_ms": 1800, "end_ms": 3000}
        ]}"#,
    )?;

    let output = harness.run_output(&[
        "experimental",
        "speakers",
        "rediarize",
        input.to_str().expect("utf-8 temp path"),
        "--turns",
        turns.to_str().expect("utf-8 temp path"),
        "-o",
        out.to_str().expect("utf-8 temp path"),
        "--contested-at",
        "0.19",
        "--summary-json",
        summary_path.to_str().expect("utf-8 temp path"),
    ])?;
    crate::common::assert_success(&output, "chatter rediarize with overlapping turns");

    let summary: serde_json::Value = serde_json::from_str(&fs::read_to_string(&summary_path)?)?;
    let contested = summary["contested"]
        .as_array()
        .expect("contested should be an array");
    assert_eq!(
        contested.len(),
        1,
        "PAR1 holds 200 of 1000 track-ms, so the second utterance is contested: {summary}"
    );
    assert_eq!(
        contested[0]["ownership"]["shares"],
        serde_json::json!([["PAR2", 800], ["PAR1", 200]]),
        "same-track overlap must count once: {summary}"
    );
    assert_eq!(contested[0]["ownership"]["total_ms"], 1000, "{summary}");
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
        "experimental",
        "speakers",
        "rediarize",
        input.to_str().expect("utf-8 temp path"),
        "--turns",
        turns.to_str().expect("utf-8 temp path"),
        "-o",
        out.to_str().expect("utf-8 temp path"),
    ])?;
    crate::common::assert_failure(&output, "chatter rediarize with inverted span");
    assert!(
        !out.exists(),
        "no output file should be written on a rejected turns file"
    );
    let diagnostics = crate::common::combined_output(&output);
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
        "experimental",
        "speakers",
        "rediarize",
        input.to_str().expect("utf-8 temp path"),
        "--turns",
        turns.to_str().expect("utf-8 temp path"),
        "-o",
        out.to_str().expect("utf-8 temp path"),
    ])?;
    crate::common::assert_success(&output, "chatter rediarize with partial coverage");

    let summary = crate::common::combined_output(&output);
    assert!(
        summary.contains("2 flagged"),
        "summary should report the flagged count:\n{summary}"
    );
    Ok(())
}

#[test]
fn rediarize_refuses_source_and_output_aliases_before_truncation() -> Result<(), TestError> {
    #[derive(Clone, Copy)]
    enum Alias {
        Direct,
        Hard,
        #[cfg(unix)]
        Symbolic,
    }
    fn alias(
        source: &std::path::Path,
        destination: &std::path::Path,
        kind: Alias,
    ) -> std::path::PathBuf {
        match kind {
            Alias::Direct => source.to_owned(),
            Alias::Hard => {
                fs::hard_link(source, destination).unwrap();
                destination.to_owned()
            }
            #[cfg(unix)]
            Alias::Symbolic => {
                std::os::unix::fs::symlink(source, destination).unwrap();
                destination.to_owned()
            }
        }
    }
    let harness = CliHarness::new()?;
    for kind in [
        Alias::Direct,
        Alias::Hard,
        #[cfg(unix)]
        Alias::Symbolic,
    ] {
        #[derive(Clone, Copy)]
        enum Destination {
            Chat,
            Summary,
        }
        for (destination, source_name) in [
            (Destination::Chat, "input.cha"),
            (Destination::Chat, "turns.json"),
            (Destination::Summary, "input.cha"),
            (Destination::Summary, "turns.json"),
            (Destination::Summary, "output.cha"),
        ] {
            let dir = tempdir()?;
            let input = dir.path().join("input.cha");
            let turns = dir.path().join("turns.json");
            let output = dir.path().join("output.cha");
            let summary = dir.path().join("summary.json");
            let source = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
            fs::write(&input, source)?;
            fs::write(&turns, r#"{"turns":[]}"#)?;
            fs::write(&output, "existing CHAT output")?;
            fs::write(&summary, "existing summary")?;
            let aliased = alias(
                &dir.path().join(source_name),
                &dir.path().join("alias"),
                kind,
            );
            let (output_path, summary_path) = match destination {
                Destination::Chat => (&aliased, &summary),
                Destination::Summary => (&output, &aliased),
            };
            let result = harness.run_output(&[
                "experimental",
                "speakers",
                "rediarize",
                input.to_str().unwrap(),
                "--turns",
                turns.to_str().unwrap(),
                "-o",
                output_path.to_str().unwrap(),
                "--summary-json",
                summary_path.to_str().unwrap(),
            ])?;
            assert_eq!(result.status.code(), Some(1));
            assert!(String::from_utf8_lossy(&result.stderr).contains("alias"));
            assert!(result.stdout.is_empty());
            assert_eq!(fs::read_to_string(&input)?, source);
            assert_eq!(fs::read_to_string(&turns)?, r#"{"turns":[]}"#);
            assert_eq!(fs::read_to_string(&output)?, "existing CHAT output");
            assert_eq!(fs::read_to_string(&summary)?, "existing summary");
        }
    }
    Ok(())
}

#[test]
fn rediarize_checks_summary_destination_before_stdout() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("input.cha");
    let turns = dir.path().join("turns.json");
    let source = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
    fs::write(&input, source)?;
    fs::write(&turns, r#"{"turns":[]}"#)?;
    let result = harness.run_output(&[
        "experimental",
        "speakers",
        "rediarize",
        input.to_str().unwrap(),
        "--turns",
        turns.to_str().unwrap(),
        "--summary-json",
        input.to_str().unwrap(),
    ])?;
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read_to_string(input)?, source);
    Ok(())
}

#[test]
fn rediarize_prepares_both_destinations_before_replacing_existing_outputs() -> Result<(), TestError>
{
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let input = dir.path().join("input.cha");
    let turns = dir.path().join("turns.json");
    let output = dir.path().join("output.cha");
    let missing_parent = dir.path().join("absent/summary.json");
    let summary = dir.path().join("summary.json");
    let source = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
    fs::write(&input, source)?;
    fs::write(&turns, r#"{"turns":[]}"#)?;
    let old = "prior output must survive preparation failure".repeat(100);
    fs::write(&output, &old)?;
    let run = |summary: &std::path::Path| {
        harness.run_output(&[
            "experimental",
            "speakers",
            "rediarize",
            input.to_str().unwrap(),
            "--turns",
            turns.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--summary-json",
            summary.to_str().unwrap(),
        ])
    };
    let refused = run(&missing_parent)?;
    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(fs::read_to_string(&output)?, old);
    fs::write(&summary, &old)?;
    let accepted = run(&summary)?;
    crate::common::assert_success(&accepted, "distinct rediarize destinations");
    let actual = fs::read_to_string(&output)?;
    assert!(actual.ends_with("@End\n"));
    assert!(!actual.contains("prior output"));
    let receipt: serde_json::Value = serde_json::from_str(&fs::read_to_string(summary)?).unwrap();
    assert_eq!(receipt["unchanged"], 4);
    assert_eq!(fs::read_to_string(input)?, source);
    assert_eq!(fs::read_to_string(turns)?, r#"{"turns":[]}"#);
    Ok(())
}
