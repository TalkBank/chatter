//! Integration tests for `chatter debug join-retrace`.
//!
//! Wave 1 auto-repair of the OBVIOUS subset of E370 ("dangling retrace")
//! errors: an utterance whose last main-tier content is a partial-repetition
//! retrace marker (`[/]`) with nothing after it, followed by a same-speaker
//! utterance whose leading words repeat the retraced material. The repair
//! joins the two utterances into one.
//!
//! Wave 3a extends this to correction retraces (`[//]`/`[///]`/`[/-]`) when
//! `--scope corrections` is passed. Wave 3b (broadest, `--scope all`) joins
//! any dangling retrace kind including `[/]` where the successor does NOT
//! repeat the retraced material.
//!
//! These drive the real CLI subprocess seam (`chatter debug join-retrace`),
//! the highest-level boundary the feature lives at, then assert that
//! `chatter validate` accepts the joined output.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_Scope>

mod common;

use common::{CliHarness, combined_output, stdout_string, write_fixture};
use talkbank_parser_tests::test_error::TestError;

/// A minimal CHAT header used by the inline fixtures below.
const HEADER: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n";

/// Build a full CHAT document from a header and a body.
fn doc(body: &str) -> String {
    format!("{HEADER}{body}@End\n")
}

/// The obvious case with NO dependent tiers: U ends with a partial retrace
/// `and [/]` and the next same-speaker utterance leads with the repeated
/// word `and`. The two must be joined into one utterance, and the result
/// must validate.
#[test]
fn joins_obvious_dangling_retrace_no_dependent_tiers() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let input = doc("*CHI:\tI want and [/] .\n*CHI:\tand the cat .\n");
    let fixture = write_fixture(harness.home_dir(), "obvious.cha", &input)?;

    // Run the repair in place.
    let join = harness.run_output(&["debug", "join-retrace", fixture.to_str().unwrap()])?;
    assert!(
        join.status.success(),
        "join-retrace should succeed; output: {}",
        combined_output(&join)
    );

    let joined = std::fs::read_to_string(&fixture)?;
    // The two utterances become one: U content (including the trailing
    // retrace marker) followed by V content, terminated by V's terminator.
    assert!(
        joined.contains("*CHI:\tI want and [/] and the cat ."),
        "expected joined single utterance, got:\n{joined}"
    );
    // V is gone as a separate line.
    assert_eq!(
        joined.matches("*CHI:").count(),
        1,
        "expected exactly one *CHI line after join, got:\n{joined}"
    );

    // The joined file must be valid CHAT.
    let validate = harness.run_validate(&fixture, &["--force"])?;
    assert!(
        validate.status.success(),
        "joined file must validate; output: {}",
        combined_output(&validate)
    );

    Ok(())
}

/// The obvious case WHERE U/V carry dependent tiers (%mor/%gra): the main
/// tiers are joined, the dependent tiers are DROPPED on the joined utterance
/// (a naive %gra merge would produce two ROOT relations, chatter E723), the
/// file still validates, and the run reports it as "needs re-morphotag".
#[test]
fn joins_obvious_case_dropping_dependent_tiers() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let input = doc(
        "*CHI:\t<the dog> [/] .\n%mor:\tdet:art|the noun|dog .\n%gra:\t1|2|DET 2|0|ROOT 3|2|PUNCT\n*CHI:\tthe dog runs .\n%mor:\tdet:art|the noun|dog verb|run-3S .\n%gra:\t1|2|DET 2|3|SUBJ 3|0|ROOT 4|3|PUNCT\n",
    );
    let fixture = write_fixture(harness.home_dir(), "deptier.cha", &input)?;

    let join = harness.run_output(&["debug", "join-retrace", fixture.to_str().unwrap()])?;
    assert!(
        join.status.success(),
        "join-retrace should succeed; output: {}",
        combined_output(&join)
    );

    let joined = std::fs::read_to_string(&fixture)?;
    assert!(
        joined.contains("*CHI:\t<the dog> [/] the dog runs ."),
        "expected joined single utterance, got:\n{joined}"
    );
    // The joined utterance must have NO dependent tiers.
    assert!(
        !joined.contains("%mor:") && !joined.contains("%gra:"),
        "joined utterance must drop dependent tiers, got:\n{joined}"
    );

    // The run must report the dropped dependent tiers as needing re-morphotag.
    let report = stdout_string(&join);
    assert!(
        report.contains("re-morphotag"),
        "report must flag needs-re-morphotag, got:\n{report}"
    );

    let validate = harness.run_validate(&fixture, &["--force"])?;
    assert!(
        validate.status.success(),
        "joined file must validate; output: {}",
        combined_output(&validate)
    );

    Ok(())
}

/// Negative: a `[//]` correction (full retrace) is NOT joined; only partial
/// repetition `[/]` qualifies for Wave 1.
#[test]
fn does_not_join_correction_retrace() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let input = doc("*CHI:\tthe cat [//] .\n*CHI:\tthe dog runs .\n");
    let fixture = write_fixture(harness.home_dir(), "correction.cha", &input)?;

    let join = harness.run_output(&["debug", "join-retrace", fixture.to_str().unwrap()])?;
    assert!(
        join.status.success(),
        "join-retrace should run; output: {}",
        combined_output(&join)
    );

    let after = std::fs::read_to_string(&fixture)?;
    assert_eq!(
        after, input,
        "a [//] correction must be left untouched, got:\n{after}"
    );

    Ok(())
}

/// Negative: a partial retrace `[/]` whose successor does NOT repeat the
/// retraced material is NOT joined (that is a later wave, not OBVIOUS).
#[test]
fn does_not_join_non_repeating_successor() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let input = doc("*CHI:\tthe dog [/] .\n*CHI:\twhat happened next .\n");
    let fixture = write_fixture(harness.home_dir(), "nonrepeat.cha", &input)?;

    let join = harness.run_output(&["debug", "join-retrace", fixture.to_str().unwrap()])?;
    assert!(
        join.status.success(),
        "join-retrace should run; output: {}",
        combined_output(&join)
    );

    let after = std::fs::read_to_string(&fixture)?;
    assert_eq!(
        after, input,
        "a non-repeating successor must be left untouched, got:\n{after}"
    );

    Ok(())
}

// --- Wave 3a: --scope corrections ---

/// With `--scope corrections`, a dangling `[//]` correction retrace is
/// joined with its same-speaker successor, and the result validates.
#[test]
fn scope_corrections_joins_full_correction_retrace() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let input = doc("*CHI:\tthe cat [//] .\n*CHI:\tthe dog runs .\n");
    let fixture = write_fixture(harness.home_dir(), "correction_join.cha", &input)?;

    let join = harness.run_output(&[
        "debug",
        "join-retrace",
        "--scope",
        "corrections",
        fixture.to_str().unwrap(),
    ])?;
    assert!(
        join.status.success(),
        "join-retrace --scope corrections should succeed; output: {}",
        combined_output(&join)
    );

    let joined = std::fs::read_to_string(&fixture)?;
    assert!(
        joined.contains("*CHI:\tthe cat [//] the dog runs ."),
        "expected joined correction utterance, got:\n{joined}"
    );
    assert_eq!(
        joined.matches("*CHI:").count(),
        1,
        "expected exactly one *CHI line after join, got:\n{joined}"
    );

    // The joined file must be valid CHAT.
    let validate = harness.run_validate(&fixture, &["--force"])?;
    assert!(
        validate.status.success(),
        "joined correction file must validate; output: {}",
        combined_output(&validate)
    );

    Ok(())
}

/// Without `--scope corrections` (using the default), the same `[//]` dangling
/// case is NOT joined (the default RepetitionOnly scope preserves Wave-1
/// behavior).
#[test]
fn default_scope_does_not_join_full_correction() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let input = doc("*CHI:\tthe cat [//] .\n*CHI:\tthe dog runs .\n");
    let fixture = write_fixture(harness.home_dir(), "correction_nojoin.cha", &input)?;

    let join = harness.run_output(&["debug", "join-retrace", fixture.to_str().unwrap()])?;
    assert!(
        join.status.success(),
        "join-retrace should run; output: {}",
        combined_output(&join)
    );

    let after = std::fs::read_to_string(&fixture)?;
    assert_eq!(
        after, input,
        "a [//] correction must be left untouched under default scope, got:\n{after}"
    );

    Ok(())
}

/// With `--scope corrections --dry-run`, the proposed join is reported
/// without modifying the file.
#[test]
fn scope_corrections_dry_run_reports_without_modifying() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let input = doc("*CHI:\tthe cat [//] .\n*CHI:\tthe dog runs .\n");
    let fixture = write_fixture(harness.home_dir(), "correction_dryrun.cha", &input)?;

    let join = harness.run_output(&[
        "debug",
        "join-retrace",
        "--scope",
        "corrections",
        "--dry-run",
        fixture.to_str().unwrap(),
    ])?;
    assert!(
        join.status.success(),
        "join-retrace --scope corrections --dry-run should succeed; output: {}",
        combined_output(&join)
    );

    // The report must mention the proposed join.
    let report = stdout_string(&join);
    assert!(
        report.contains("would join")
            || report.contains("Would join")
            || report.contains("[dry-run]"),
        "dry-run output should report the proposed join, got:\n{report}"
    );

    // The file must be unchanged.
    let after = std::fs::read_to_string(&fixture)?;
    assert_eq!(
        after, input,
        "--dry-run must not modify the file, got:\n{after}"
    );

    Ok(())
}

// --- Wave 3b: --scope all ---

/// With `--scope all`, a non-repeating `[/]` (successor does NOT begin with
/// the retraced material) IS joined and the result validates.
/// Under `--scope corrections` or the default, the same fixture is left
/// untouched.
///
/// Fixture: retraced material "要 去"; successor leads with "我" (not "要"),
/// so the prefix match fails under `repetition` and `corrections` scopes.
#[test]
fn scope_all_joins_nonrepeat_partial_retrace() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let input = doc("*CHI:\t要 去 [/] .\n*CHI:\t我 要 去 公 園 .\n");

    // Default scope: must NOT join.
    {
        let fixture = write_fixture(harness.home_dir(), "nonrepeat_default.cha", &input)?;
        let join = harness.run_output(&["debug", "join-retrace", fixture.to_str().unwrap()])?;
        assert!(join.status.success(), "{}", combined_output(&join));
        let after = std::fs::read_to_string(&fixture)?;
        assert_eq!(
            after, input,
            "default scope must not join non-repeat [/], got:\n{after}"
        );
    }

    // --scope corrections: must NOT join a non-repeat [/].
    {
        let fixture = write_fixture(harness.home_dir(), "nonrepeat_corrections.cha", &input)?;
        let join = harness.run_output(&[
            "debug",
            "join-retrace",
            "--scope",
            "corrections",
            fixture.to_str().unwrap(),
        ])?;
        assert!(join.status.success(), "{}", combined_output(&join));
        let after = std::fs::read_to_string(&fixture)?;
        assert_eq!(
            after, input,
            "--scope corrections must not join non-repeat [/], got:\n{after}"
        );
    }

    // --scope all: MUST join.
    {
        let fixture = write_fixture(harness.home_dir(), "nonrepeat_all.cha", &input)?;
        let join = harness.run_output(&[
            "debug",
            "join-retrace",
            "--scope",
            "all",
            fixture.to_str().unwrap(),
        ])?;
        assert!(
            join.status.success(),
            "join-retrace --scope all should succeed; output: {}",
            combined_output(&join)
        );

        let joined = std::fs::read_to_string(&fixture)?;
        assert!(
            joined.contains("*CHI:\t要 去 [/] 我 要 去 公 園 ."),
            "expected joined non-repeat [/], got:\n{joined}"
        );
        assert_eq!(
            joined.matches("*CHI:").count(),
            1,
            "expected exactly one *CHI line, got:\n{joined}"
        );

        // The joined file must be valid CHAT.
        let validate = harness.run_validate(&fixture, &["--force"])?;
        assert!(
            validate.status.success(),
            "joined non-repeat [/] file must validate; output: {}",
            combined_output(&validate)
        );
    }

    Ok(())
}

/// With `--scope all`, a different-speaker successor is still NOT joined.
#[test]
fn scope_all_does_not_join_different_speaker() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let input = doc("*CHI:\t要 去 [/] .\n*MOT:\t要 去 公 園 .\n");
    let fixture = write_fixture(harness.home_dir(), "diffspk_all.cha", &input)?;

    let join = harness.run_output(&[
        "debug",
        "join-retrace",
        "--scope",
        "all",
        fixture.to_str().unwrap(),
    ])?;
    assert!(join.status.success(), "{}", combined_output(&join));

    let after = std::fs::read_to_string(&fixture)?;
    assert_eq!(
        after, input,
        "--scope all must not join different-speaker successor, got:\n{after}"
    );

    Ok(())
}
