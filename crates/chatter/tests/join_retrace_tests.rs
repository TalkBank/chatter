//! Integration tests for `chatter debug join-retrace`.
//!
//! Wave 1 auto-repair of the OBVIOUS subset of E370 ("dangling retrace")
//! errors: an utterance whose last main-tier content is a partial-repetition
//! retrace marker (`[/]`) with nothing after it, followed by a same-speaker
//! utterance whose leading words repeat the retraced material. The repair
//! joins the two utterances into one.
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
