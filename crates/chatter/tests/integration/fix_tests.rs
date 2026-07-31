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

//! CLI-boundary tests for `chatter fix`: the batch-safety tiering that is
//! the whole point of the command, and byte-identity for a broken region
//! elsewhere in the file.
//!
//! Fixtures here are deliberately hand-built rather than pulled from
//! `corpus/reference/`: each one exists to make one exact, named diagnostic
//! fire, and every fixture below was verified against a real `chatter
//! validate` run before being pinned into an assertion here, per the
//! project rule against assuming which code a construct triggers.

use std::fs;
use std::path::PathBuf;

use talkbank_parser_tests::test_error::TestError;

use crate::common::{CliHarness, write_fixture};

/// Create a fresh CLI harness and write `contents` to `name` under its home
/// dir, returning both. Collapses the `CliHarness::new()` boilerplate every
/// test below needs before it can act on a fixture; the write itself is
/// `common::write_fixture`, not a second copy of it.
fn harness_with_fixture(name: &str, contents: &str) -> Result<(CliHarness, PathBuf), TestError> {
    let harness = CliHarness::new()?;
    let path = write_fixture(harness.home_dir(), name, contents)?;
    Ok((harness, path))
}

/// One clean utterance (`*CHI:\txx .`, E241 `IllegalUntranscribed`, a
/// `BatchSafety::Mechanical` catalog fix) and one utterance whose main tier
/// does not parse (`*MOT:\t&- .`: a bare filler prefix with no following
/// word body, which the grammar cannot fold into a `standalone_word`).
///
/// Verified against `chatter validate`: exactly E316 `UnparsableContent`
/// (the MOT line, no catalog entry), two E504 `MissingRequiredHeader`
/// (missing `@Languages` and `@Participants`; the former has no catalog
/// entry, the latter is `BatchSafety::Semantic`), and E241 (the CHI line,
/// `BatchSafety::Mechanical`) fire, and no other code does.
const PARTIAL_FIXTURE: &str = "@UTF8\n@Begin\n*CHI:\txx .\n*MOT:\t&- .\n@End\n";

/// A single clean utterance carrying one `BatchSafety::Mechanical`
/// diagnostic (E241 `IllegalUntranscribed`, `"xx"` is not legal) and one
/// `BatchSafety::Semantic` diagnostic (E259 `CommaAfterNonSpokenContent`,
/// the leading comma has no spoken word before it in the utterance).
///
/// Verified against `chatter validate`: exactly E241 and E259 fire, and no
/// other code does, so bare `--apply` versus `--apply --code E259` gates
/// on exactly these two, with no other diagnostic muddying either result.
const MECHANICAL_AND_SEMANTIC_FIXTURE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|test|CHI|||||Child|||\n*CHI:\t, xx .\n@End\n";

/// Convert a fixture path to the `&str` the CLI takes, or fail the test
/// with a real reason instead of a bare `Option`-unwrap panic.
fn path_arg(path: &std::path::Path) -> Result<&str, TestError> {
    path.to_str()
        .ok_or_else(|| TestError::Failure(format!("non-UTF8 test path: {}", path.display())))
}

/// The ruling, at the CLI boundary: a broken region must not block a fix
/// elsewhere, and must itself come out byte-identical.
#[test]
fn a_broken_region_does_not_block_a_fix_elsewhere() -> Result<(), TestError> {
    let (harness, path) = harness_with_fixture("partial.cha", PARTIAL_FIXTURE)?;

    let output = harness.run_output(&["fix", path_arg(&path)?, "--apply"])?;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let after = fs::read_to_string(&path)?;
    assert!(
        after.contains("*CHI:\txxx ."),
        "the clean utterance was not fixed: {after}"
    );
    assert!(
        after.contains("*MOT:\t&- ."),
        "the broken utterance was rewritten: {after}"
    );
    Ok(())
}

/// Bare `--apply` must not touch a semantic code.
#[test]
fn bare_apply_writes_mechanical_codes_only() -> Result<(), TestError> {
    let (harness, path) = harness_with_fixture("mixed.cha", MECHANICAL_AND_SEMANTIC_FIXTURE)?;

    let output = harness.run_output(&["fix", path_arg(&path)?, "--apply"])?;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let after = fs::read_to_string(&path)?;
    assert!(
        after.contains("*CHI:\t, xxx ."),
        "expected the mechanical E241 fix applied and the leading comma \
         (the semantic E259 site) left untouched, got: {after}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("E259"),
        "the semantic E259 diagnostic was not reported: {stdout}"
    );
    assert!(
        stdout.contains("--code"),
        "the report did not explain how to opt the semantic fix in: {stdout}"
    );
    Ok(())
}

/// Naming the code opts in to the semantic tier.
#[test]
fn naming_a_semantic_code_opts_into_it() -> Result<(), TestError> {
    let (harness, path) = harness_with_fixture("mixed.cha", MECHANICAL_AND_SEMANTIC_FIXTURE)?;

    let output = harness.run_output(&["fix", path_arg(&path)?, "--apply", "--code", "E259"])?;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let after = fs::read_to_string(&path)?;
    // Not "*CHI:\t xx .": the comma sat directly against the tab that opens
    // the tier, so deleting only the comma byte would leave a leading space
    // on the tier, which is itself invalid (E758 `LeadingSpaceOnMainTier`).
    // `chatter fix`'s own post-splice re-parse check caught exactly this
    // when it was first wired up (2026-07-31), which is why the catalog fix
    // widens a tier-initial comma's deletion to consume that space too.
    assert!(
        after.contains("*CHI:\txx ."),
        "the semantic E259 fix (comma removal) was not applied cleanly: {after}"
    );
    assert!(
        !after.contains("xxx"),
        "--code E259 must not also apply the un-named E241 fix: {after}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("E259"),
        "the opted-in semantic fix was not reported: {stdout}"
    );
    Ok(())
}

/// A typo'd `--code` must FAIL CLOSED, not silently widen the run to every
/// code. Regression for the 2026-05-06-shaped incident this review finding
/// names: `resolve_requested_codes` used to warn and drop an unrecognized
/// value, leaving an empty `HashSet` that the narrowing check read as "no
/// narrowing at all", so `--code E2411` (a typo for `E241`) behaved
/// identically to bare `--apply` and applied every mechanical fix in the
/// file.
#[test]
fn typo_code_fails_closed_and_writes_nothing() -> Result<(), TestError> {
    let (harness, path) = harness_with_fixture("typo.cha", MECHANICAL_AND_SEMANTIC_FIXTURE)?;

    let output = harness.run_output(&["fix", path_arg(&path)?, "--apply", "--code", "E2411"])?;
    assert!(
        !output.status.success(),
        "a typo'd --code must exit non-zero, not silently widen to every code"
    );

    let after = fs::read_to_string(&path)?;
    assert_eq!(
        after, MECHANICAL_AND_SEMANTIC_FIXTURE,
        "a typo'd --code must write nothing, not apply the mechanical fix anyway"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("E2411"),
        "the error must name the offending value: {stderr}"
    );
    Ok(())
}

/// A REAL code that the file simply never fires must select nothing: it is
/// not a typo (so it must not fail closed like the case above), and it must
/// not fall back to "every code" either.
#[test]
fn a_real_code_absent_from_the_file_selects_nothing() -> Result<(), TestError> {
    let (harness, path) = harness_with_fixture("absent.cha", MECHANICAL_AND_SEMANTIC_FIXTURE)?;

    // E316 UnparsableContent never fires against this fixture.
    let output = harness.run_output(&["fix", path_arg(&path)?, "--apply", "--code", "E316"])?;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let after = fs::read_to_string(&path)?;
    assert_eq!(
        after, MECHANICAL_AND_SEMANTIC_FIXTURE,
        "naming a real code the file never fires must not touch the file"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 fix(es) applied across 0 file(s)"),
        "nothing should have been selected: {stdout}"
    );
    Ok(())
}

/// `--dry-run` (which requires `--apply`) previews without writing.
#[test]
fn dry_run_reports_without_writing() -> Result<(), TestError> {
    let (harness, path) = harness_with_fixture("mixed.cha", MECHANICAL_AND_SEMANTIC_FIXTURE)?;

    let output = harness.run_output(&["fix", path_arg(&path)?, "--apply", "--dry-run"])?;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let after = fs::read_to_string(&path)?;
    assert_eq!(
        after, MECHANICAL_AND_SEMANTIC_FIXTURE,
        "--dry-run must not write to disk"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("E241"),
        "dry run did not name what would change: {stdout}"
    );
    Ok(())
}

/// The post-write safety net (the write gate plus the re-parse check in
/// `crates/chatter/src/commands/fix.rs::verify_fix_result`) must not block a
/// legitimate fix, and its own claim that the targeted diagnostic is gone
/// must hold up under an INDEPENDENT `chatter validate` run on the written
/// file, not merely under `fix`'s own report line.
#[test]
fn fixed_file_clears_the_targeted_code_under_independent_validation() -> Result<(), TestError> {
    let (harness, path) = harness_with_fixture("mechanical.cha", MECHANICAL_AND_SEMANTIC_FIXTURE)?;

    let fix_output = harness.run_output(&["fix", path_arg(&path)?, "--apply"])?;
    assert!(
        fix_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&fix_output.stderr)
    );
    let fix_stdout = String::from_utf8_lossy(&fix_output.stdout);
    assert!(
        !fix_stdout.contains("safety check refused"),
        "the post-write safety check refused a legitimate mechanical fix: {fix_stdout}"
    );

    let validate_output = harness.run_output(&["validate", path_arg(&path)?, "--force"])?;
    let validate_stderr = String::from_utf8_lossy(&validate_output.stderr);
    assert!(
        !validate_stderr.contains("E241"),
        "E241 still fires under independent validation after the fix that was \
         supposed to clear it: {validate_stderr}"
    );
    Ok(())
}

/// The deleted command must be gone, not silently accepted.
#[test]
fn lint_subcommand_no_longer_exists() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let output = harness.run_output(&["lint", "--help"])?;
    assert!(
        !output.status.success(),
        "lint should no longer be a subcommand"
    );
    Ok(())
}

/// A fix that is SELECTED but fails to WRITE must not be counted as
/// applied. Regression for the review finding that `run_fix` incremented
/// its running total before attempting the write: a run whose only
/// writable file failed to write printed a nonzero "fix(es) applied"
/// count against zero files.
#[cfg(unix)]
#[test]
fn write_failure_does_not_inflate_the_applied_count() -> Result<(), TestError> {
    use std::os::unix::fs::PermissionsExt;

    let (harness, path) = harness_with_fixture("readonly.cha", MECHANICAL_AND_SEMANTIC_FIXTURE)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444))?;

    let run = harness.run_output(&["fix", path_arg(&path)?, "--apply"]);
    // Restore write permission unconditionally so the harness's temp-dir
    // cleanup can remove the file regardless of how the assertions below
    // turn out.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))?;
    let output = run?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot write"),
        "expected a write-failure error, got stderr: {stderr}"
    );
    assert!(
        stdout.contains("0 fix(es) applied across 0 file(s)"),
        "a failed write must not be counted as applied: {stdout}"
    );
    Ok(())
}
