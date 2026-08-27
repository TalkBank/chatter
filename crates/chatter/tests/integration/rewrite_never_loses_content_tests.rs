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

//! No command that rewrites a transcript may DROP part of it.
//!
//! These drive the real CLI subprocess, which is the boundary the defect lives
//! at: every case below was found by running the shipped binary, and every one
//! of them exited 0 with a success line while destroying data.
//!
//! Measured on 2026-08-27 against the 0.15.0 release binary and the 0.16.0
//! candidate. `chatter to-json` already refused all three of the `normalize`
//! inputs; the two commands read the same model and disagreed, which is what
//! made this findable.
//!
//! What is deliberately NOT asserted here: that a rewrite is FAITHFUL, or that
//! the file is VALID. `normalize` is entitled to canonicalise a transcript the
//! validator rejects, and it legitimately changes whitespace in six files of
//! the reference corpus. The claim is narrower and is the one that matters for
//! a tool that writes over somebody's data: nothing disappears.

use crate::common::{CliHarness, combined_output, write_fixture};
use talkbank_parser_tests::test_error::TestError;

/// A minimal CHAT header used by the inline fixtures below.
const HEADER: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|2;||||Target_Child|||\n";

/// A file of ordinary prose is not CHAT, and `normalize` must not "canonicalise"
/// it into nothing.
///
/// `chatter normalize notes.cha -o notes.cha` is the documented in-place idiom.
/// On 2026-08-27 the candidate binary turned a 25-byte file into a ZERO-BYTE
/// file, exited 0, and printed `✓ Normalized`. The 0.15.0 release refused it.
/// This is the regression that made the whole class visible.
#[test]
fn normalize_refuses_a_file_it_would_empty() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let source = "notes about this session\n";
    let path = write_fixture(harness.home_dir(), "prose.cha", source)?;

    let output = harness.run_output(&["normalize", path.to_str().unwrap()])?;

    assert!(
        !output.status.success(),
        "normalize must REFUSE a file whose model holds none of it; got exit 0 with:\n{}",
        combined_output(&output)
    );
    let on_disk = std::fs::read_to_string(&path)?;
    assert_eq!(
        on_disk, source,
        "a refused normalize must leave the source untouched"
    );
    Ok(())
}

/// A transcript missing its `@End` must not lose its last utterance.
///
/// A file truncated mid-transfer is exactly this shape, and it is the
/// population least able to afford silent loss. `chatter validate` reports
/// E502 on it; `normalize` never consulted that and wrote the short model.
#[test]
fn normalize_refuses_to_drop_the_last_utterance_of_a_truncated_file() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let source = format!("{HEADER}*CHI:\tone .\n*CHI:\ttwo .\n*CHI:\tthree .\n");
    let path = write_fixture(harness.home_dir(), "truncated.cha", &source)?;

    let output = harness.run_output(&["normalize", path.to_str().unwrap()])?;
    let rendered = combined_output(&output);

    assert!(
        !output.status.success(),
        "normalize dropped an utterance and reported success:\n{rendered}"
    );
    assert!(
        rendered.contains("three"),
        "the refusal must name the content it would have dropped, so the operator \
         does not have to hunt for it; got:\n{rendered}"
    );
    Ok(())
}

/// A malformed `%gra` tier must not be emptied.
///
/// The candidate wrote `%gra:\t` with the entire tier body deleted, exit 0, and
/// the file it produced was one it then REFUSED to read again (E600). A tool
/// whose output its own next run rejects has not normalized anything.
#[test]
fn normalize_refuses_to_empty_a_malformed_dependent_tier() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let source = format!("{HEADER}*CHI:\thello world .\n%gra:\t|2|SUBJ 2|0|ROOT\n");
    let path = write_fixture(harness.home_dir(), "gra.cha", &source)?;

    let output = harness.run_output(&["normalize", path.to_str().unwrap()])?;

    assert!(
        !output.status.success(),
        "normalize emptied a dependent tier and reported success:\n{}",
        combined_output(&output)
    );
    Ok(())
}

/// `debug retag-language` must not delete a region it could not parse.
///
/// This is the sharpest case, because the command takes DIRECTORIES, recurses,
/// rewrites every match in place, and has no `--dry-run` and no backup. On
/// 2026-08-27 it turned `hello [[[[ test ]]]] world .` into `world .`, exit 0,
/// under a line reading `Retagged`. Its only warning said a parse diagnostic
/// existed, not that content would be discarded.
#[test]
fn retag_language_refuses_a_file_it_cannot_reproduce() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let source = "@UTF8\n@Begin\n@Languages:\tswe, sun\n@Participants:\tCHI Target_Child\n\
                  @ID:\tswe|corpus|CHI|2;||||Target_Child|||\n\
                  *CHI:\tankka@s:sun .\n*CHI:\thello [[[[ test ]]]] world .\n@End\n";
    let path = write_fixture(harness.home_dir(), "mix.cha", source)?;

    let output = harness.run_output(&[
        "debug",
        "retag-language",
        "--from",
        "sun",
        "--to",
        "fin",
        path.to_str().unwrap(),
    ])?;

    let on_disk = std::fs::read_to_string(&path)?;
    assert!(
        on_disk.contains("hello") && on_disk.contains("test"),
        "retag-language deleted transcript content it could not parse.\nexit: {:?}\noutput:\n{}\nfile now:\n{on_disk}",
        output.status.code(),
        combined_output(&output)
    );
    Ok(())
}

/// `debug fix-s` shares the rewriter, so it must share the refusal.
///
/// Reported as the same loss on the same fixture. Kept as its own case because
/// "the other command happens to be fixed too" is the assumption that let this
/// class spread to a third command in the first place.
#[test]
fn fix_s_refuses_a_file_it_cannot_reproduce() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let source = "@UTF8\n@Begin\n@Languages:\tswe, sun\n@Participants:\tCHI Target_Child\n\
                  @ID:\tswe|corpus|CHI|2;||||Target_Child|||\n\
                  *CHI:\tankka@s:sun .\n*CHI:\thello [[[[ test ]]]] world .\n@End\n";
    let path = write_fixture(harness.home_dir(), "fixs.cha", source)?;

    let output = harness.run_output(&["debug", "fix-s", path.to_str().unwrap()])?;

    let on_disk = std::fs::read_to_string(&path)?;
    assert!(
        on_disk.contains("hello") && on_disk.contains("test"),
        "fix-s deleted transcript content it could not parse.\nexit: {:?}\noutput:\n{}\nfile now:\n{on_disk}",
        output.status.code(),
        combined_output(&output)
    );
    Ok(())
}

/// `debug sanitize` must refuse to write over its own input.
///
/// Sanitize is DELIBERATELY lossy: it strips contributor lexical content and
/// keeps the structure. That is correct for what it is for, and it is exactly
/// why it must never be aimed at the source. `run_sanitize` wrote to whatever
/// `-o` named, with no check that the path differed from the input, so
/// `chatter debug sanitize x.cha -o x.cha` replaced a protected-corpus original
/// with its own redaction, unrecoverably, at exit 0.
///
/// This one is not a rewriter, which is the point: it EMITS to a destination,
/// and the destination simply must not be the source.
#[test]
fn sanitize_refuses_to_overwrite_its_own_input() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let source = format!("{HEADER}*CHI:\thello world .\n@End\n");
    let path = write_fixture(harness.home_dir(), "protected.cha", &source)?;

    let output = harness.run_output(&[
        "debug",
        "sanitize",
        path.to_str().unwrap(),
        "-o",
        path.to_str().unwrap(),
    ])?;

    let on_disk = std::fs::read_to_string(&path)?;
    assert_eq!(
        on_disk,
        source,
        "sanitize overwrote its own input with a redaction of itself.\nexit: {:?}\noutput:\n{}",
        output.status.code(),
        combined_output(&output)
    );
    assert!(
        !output.status.success(),
        "sanitize must REFUSE this, not silently succeed having written nothing:\n{}",
        combined_output(&output)
    );
    Ok(())
}

/// The guard must not fire on the rewrites `normalize` is FOR.
///
/// Six reference-corpus files legitimately change under `normalize`, every one
/// of them in whitespace only: a doubled space collapsed, a trailing space
/// trimmed inside an `@ID`, `spa , eng` tightened, a space inserted before a
/// terminator, and a wrapped `@Participants` header joined onto one line. A
/// content-loss guard that refuses those has replaced a data-loss bug with a
/// useless tool, so this case is as load-bearing as the refusals above.
#[test]
fn normalize_still_accepts_the_whitespace_rewrites_it_exists_for() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    // The real rewrites, taken from the six `corpus/reference` files that
    // change under `normalize`. Two of these rows were empty strings skipped
    // by an `if body.is_empty() { continue; }` when this test was written, so
    // it claimed five normalizations and exercised one. A table with a runtime
    // guard over unwritten rows reads as coverage that is not there.
    let cases: [(&str, &str); 2] = [
        (
            "a doubled space collapsed",
            "*CHI:\twe need to  do something .\n",
        ),
        (
            "a space inserted before a terminator",
            "*CHI:\tyeah sure &*INV:ah.\n",
        ),
    ];
    for (label, body) in cases {
        let source = format!("{HEADER}{body}@End\n");
        let path = write_fixture(harness.home_dir(), "ws.cha", &source)?;
        let output = harness.run_output(&["normalize", path.to_str().unwrap()])?;
        assert!(
            output.status.success(),
            "normalize refused a legitimate whitespace rewrite ({label}):\n{}",
            combined_output(&output)
        );
    }

    // The `@ID` cases need their own header, since HEADER's own `@ID` is the
    // line under test.
    let spaced_id = "@UTF8\n@Begin\n@Languages:\tspa, eng\n@Participants:\tCHI Target_Child\n\
                     @ID:\tspa , eng|corpus|CHI|2;08.20||||Target_Child|||\n*CHI:\thola .\n@End\n";
    let path = write_fixture(harness.home_dir(), "spaced_id.cha", spaced_id)?;
    let output = harness.run_output(&["normalize", path.to_str().unwrap()])?;
    assert!(
        output.status.success(),
        "normalize refused `spa , eng` tightening to `spa, eng`:\n{}",
        combined_output(&output)
    );

    // The wrapped-header join, which is why the guard asks for CONTAINMENT
    // rather than equality: the continuation's text survives inside the joined
    // line and must not read as a dropped line.
    let wrapped = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child,\n\tMOT Mother\n\
                   @ID:\teng|corpus|CHI|2;||||Target_Child|||\n\
                   @ID:\teng|corpus|MOT|||||Mother|||\n*CHI:\thi .\n@End\n";
    let path = write_fixture(harness.home_dir(), "wrapped.cha", wrapped)?;
    let output = harness.run_output(&["normalize", path.to_str().unwrap()])?;
    assert!(
        output.status.success(),
        "normalize refused a wrapped @Participants header, which it is supposed to join:\n{}",
        combined_output(&output)
    );
    Ok(())
}
