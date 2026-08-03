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

//! CLI integration tests for Phon's four `%x` dependent tiers.
//!
//! Top-level red/green boundary for the "fold Phon `%x` tiers into chatter as
//! first-class validated tiers" feature. These run the real `chatter validate`
//! CLI on minimal fixtures and assert that malformed Phon-extension tiers surface
//! the specific E73x diagnostic through the actual command boundary.
//!
//! Before the feature, the four `%x`-named tiers (`%xmodsyl`, `%xphosyl`,
//! `%xphoaln`, `%xphoint`) were silently accepted as generic user-defined `%x`
//! tiers with no validation, so every assertion below failed (RED). Validation is
//! on by default (no `--check-xphon` needed); `--suppress xphon` remains the
//! opt-out.
//!
//! `%xphoint` (which carries `0x15` time bullets) gets its exhaustive coverage
//! from the spec-driven validation corpus; the no-bullet syllabification and
//! alignment rules are exercised here at the CLI seam.

use predicates::prelude::*;
use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

// ============================================================================
// Fixtures (minimal, well-formed CHAT whose only defect is the named %x tier)
// ============================================================================

/// Well-formed: stripping `:CODE` from each syllabification unit reproduces the
/// `%mod`/`%pho` word; every `%xphoaln` pair concatenates back to `%mod`/`%pho`.
const PHON_CLEAN: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tcat dog .
%mod:\tkæt dɒɡ
%pho:\tkæt dɒɡ
%xmodsyl:\tk:Oæ:Nt:C d:Oɒ:Nɡ:C
%xphosyl:\tk:Oæ:Nt:C d:Oɒ:Nɡ:C
%xphoaln:\tk↔k,æ↔æ,t↔t d↔d,ɒ↔ɒ,ɡ↔ɡ
@End
";

/// `%xphosyl` uses `Z`, which is not one of the legal codes O N C L R E A D U.
const PHON_ILLEGAL_CODE: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tcat dog .
%mod:\tkæt dɒɡ
%pho:\tkæt dɒɡ
%xmodsyl:\tk:Oæ:Nt:C d:Oɒ:Nɡ:C
%xphosyl:\tk:Oæ:Nt:Z d:Oɒ:Nɡ:C
%xphoaln:\tk↔k,æ↔æ,t↔t d↔d,ɒ↔ɒ,ɡ↔ɡ
@End
";

/// `U` (Unknown) is a legal syllable-constituent code: a phone may have
/// unidentified syllabification status (Greg Hedlund, 2026-06-23; the spec's
/// "every phone gets a concrete constituent" claim was wrong). Here the actual
/// production marks `/k/` as `U` while the model keeps the concrete onset,
/// exactly the model-vs-actual asymmetry Phon emits. The only difference from
/// `PHON_CLEAN` is the one `:O` that is now `:U`; reconstruction still holds.
const PHON_UNKNOWN_CODE: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tcat dog .
%mod:\tkæt dɒɡ
%pho:\tkæt dɒɡ
%xmodsyl:\tk:Oæ:Nt:C d:Oɒ:Nɡ:C
%xphosyl:\tk:Uæ:Nt:C d:Oɒ:Nɡ:C
%xphoaln:\tk↔k,æ↔æ,t↔t d↔d,ɒ↔ɒ,ɡ↔ɡ
@End
";

/// `%xmodsyl` first word drops the `t:C` unit, so stripping codes yields `kæ`,
/// which does not reproduce the `%mod` word `kæt`.
const PHON_BAD_RECONSTRUCTION: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tcat dog .
%mod:\tkæt dɒɡ
%pho:\tkæt dɒɡ
%xmodsyl:\tk:Oæ:N d:Oɒ:Nɡ:C
%xphosyl:\tk:Oæ:Nt:C d:Oɒ:Nɡ:C
%xphoaln:\tk↔k,æ↔æ,t↔t d↔d,ɒ↔ɒ,ɡ↔ɡ
@End
";

/// `%xphoaln` opens with a `∅↔∅` pair, which is never legal (both sides null).
const PHON_PHOALN_EMPTY_BOTH: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tcat dog .
%mod:\tkæt dɒɡ
%pho:\tkæt dɒɡ
%xmodsyl:\tk:Oæ:Nt:C d:Oɒ:Nɡ:C
%xphosyl:\tk:Oæ:Nt:C d:Oɒ:Nɡ:C
%xphoaln:\t∅↔∅,k↔k,æ↔æ,t↔t d↔d,ɒ↔ɒ,ɡ↔ɡ
@End
";

fn write_fixture(
    name: &str,
    body: &str,
) -> Result<(tempfile::TempDir, std::path::PathBuf), TestError> {
    let dir = tempdir()?;
    let path = dir.path().join(name);
    fs::write(&path, body)?;
    Ok((dir, path))
}

// ============================================================================
// Tests
// ============================================================================

/// Intra-word pauses (`^`, U+005E) on `%xmodsyl`/`%xphosyl` reconstruct
/// cleanly in both legal positions: mid-word (between units) and word-final
/// (after the last unit). Fixture is `corpus/reference/tiers/
/// phon-intra-word-pause.cha`, derived from two real phon-data corpus
/// occurrences (danger rule 9: reference corpus, not an ad hoc fixture).
/// Before the fix, word-final `^` wrongly errored `MissingColon` (surfaced
/// as E735) and a mid-word `^` silently fused into the following phone.
#[test]
fn phon_intra_word_pause_validates() -> Result<(), TestError> {
    let path = crate::common::reference_fixture("corpus/reference/tiers/phon-intra-word-pause.cha");
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Valid: 1"))
        .stdout(predicate::str::contains("Invalid: 0"));
    Ok(())
}

/// A well-formed Phon export validates cleanly with NO flags (validation is on
/// by default). Post-feature this passes because the tiers are parsed and every
/// rule holds, not because they are ignored.
#[test]
fn phon_xtiers_clean_file_validates() -> Result<(), TestError> {
    let (_dir, path) = write_fixture("clean.cha", PHON_CLEAN)?;
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Valid: 1"))
        .stdout(predicate::str::contains("Invalid: 0"));
    Ok(())
}

/// An illegal syllable-constituent code on `%xphosyl` is rejected by default
/// with E736.
#[test]
fn phon_xphosyl_illegal_code_emits_e736() -> Result<(), TestError> {
    let (_dir, path) = write_fixture("illegal.cha", PHON_ILLEGAL_CODE)?;
    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--format")
        .arg("json")
        .arg(&path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("E736"));
    Ok(())
}

/// `U` (Unknown) is a legal syllable-constituent code (Greg Hedlund,
/// 2026-06-23): the spec's "every phone gets a concrete constituent" claim was
/// wrong. A `:U` on `%xphosyl` must validate cleanly, NOT trip E736.
#[test]
fn phon_xphosyl_unknown_code_validates() -> Result<(), TestError> {
    let (_dir, path) = write_fixture("unknown.cha", PHON_UNKNOWN_CODE)?;
    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--format")
        .arg("json")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("E736").not());
    Ok(())
}

/// A `%xmodsyl` word whose stripped phones do not reproduce its `%mod` word is
/// rejected by default with E737.
#[test]
fn phon_xmodsyl_reconstruction_mismatch_emits_e737() -> Result<(), TestError> {
    let (_dir, path) = write_fixture("badrecon.cha", PHON_BAD_RECONSTRUCTION)?;
    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--format")
        .arg("json")
        .arg(&path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("E737"));
    Ok(())
}

/// A `∅↔∅` alignment pair on `%xphoaln` is rejected by default with E739.
#[test]
fn phon_xphoaln_empty_both_emits_e739() -> Result<(), TestError> {
    let (_dir, path) = write_fixture("phoaln.cha", PHON_PHOALN_EMPTY_BOTH)?;
    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--format")
        .arg("json")
        .arg(&path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("E739"));
    Ok(())
}

/// The opt-out escape hatch: `--suppress xphon` silences all Phon-x diagnostics
/// even on a malformed file, so it validates "successfully".
#[test]
fn phon_suppress_xphon_silences_diagnostics() -> Result<(), TestError> {
    let (_dir, path) = write_fixture("illegal.cha", PHON_ILLEGAL_CODE)?;
    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--suppress")
        .arg("xphon")
        .arg(&path)
        .assert()
        .success();
    Ok(())
}

/// Suppression must reach the `--audit` JSONL sink, not only the human-readable
/// summary. `--suppress` is documented as "suppressed errors are not reported",
/// with no carve-out for bulk audit mode, and an audit stream is precisely where
/// a silent leak does the most damage: it is the machine-readable artifact that
/// downstream tallies and cleanup queues are built from, so a consumer that
/// trusts the flag would silently count suppressed diagnostics as real findings.
///
/// Discovered 2026-07-28 during a full-corpus assessment: a
/// `--suppress xphon --audit` run over `data/` reported `Invalid: 0` and exited
/// 0 while writing every suppressed E725-E746 record into the JSONL.
#[test]
fn phon_suppress_xphon_also_silences_the_audit_stream() -> Result<(), TestError> {
    let (dir, path) = write_fixture("illegal.cha", PHON_ILLEGAL_CODE)?;
    let audit_path = dir.path().join("audit.jsonl");

    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--suppress")
        .arg("xphon")
        .arg("--audit")
        .arg(&audit_path)
        .arg(&path)
        .assert()
        .success();

    // Absence of the file is an acceptable way to emit nothing.
    let audit = fs::read_to_string(&audit_path).unwrap_or_default();
    let leaked: Vec<&str> = audit
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    assert!(
        leaked.is_empty(),
        "--suppress xphon left {} suppressed diagnostic(s) in the audit stream: {:#?}",
        leaked.len(),
        leaked
    );
    Ok(())
}

/// A file with a NON-suppressed error must still count as invalid, and still
/// fail the run, when another file in the same directory was fully suppressed.
///
/// The fixtures are a MIX on purpose, and that is the whole difficulty of
/// reproducing this: the bug needs at least one fully suppressed file to drive
/// the count adjustment AND at least one genuinely invalid file to be
/// under-counted by it. Every single-file run reported correctly, which is why
/// a full-corpus assessment found it (2026-07-30) and the existing per-file
/// suppression tests above did not.
///
/// Both assertions are load-bearing and check different consumers of the same
/// field: the printed summary, and the exit code derived from `invalid_files`.
/// Their divergence is exactly the bug class.
#[test]
fn suppression_does_not_hide_other_files_invalid_count() -> Result<(), TestError> {
    let harness = crate::common::CliHarness::new()?;
    let corpus = tempdir()?;

    // Fully suppressed by `--suppress xphon`: contributes to the suppressed
    // count that gets moved over to the valid column.
    crate::common::write_fixture(corpus.path(), "all_suppressed.cha", PHON_ILLEGAL_CODE)?;

    // A non-Phon error that `--suppress xphon` must NOT silence. Copied from
    // the committed, spec-generated E241 fixture rather than hand-written, so
    // that narrowing or retiring the E241 rule regenerates it and this test
    // follows, instead of silently asserting against a file that no longer
    // produces an error.
    fs::copy(
        crate::common::reference_fixture(
            "crates/talkbank-parser-tests/tests/error_corpus/\
             validation_errors/E241_Illegal_Untranscribed_Marker_xx.cha",
        ),
        corpus.path().join("really_invalid.cha"),
    )?;

    let output = harness.run_validate(corpus.path(), &["--suppress", "xphon"])?;
    let stdout = crate::common::stdout_string(&output);

    assert!(
        stdout.contains("Invalid: 1"),
        "the non-suppressed file must be counted as invalid; suppression must \
         not decrement the count for OTHER files.\nsummary said:\n{stdout}"
    );
    assert!(
        stdout.contains("Valid: 1"),
        "the fully-suppressed file must MOVE to the valid column, not vanish \
         from both; valid + invalid must still equal the file count.\n\
         summary said:\n{stdout}"
    );
    crate::common::assert_failure(&output, "validate over a directory with one invalid file");

    Ok(())
}

/// A verdict produced under `--suppress` must never be served back to a run
/// without it.
///
/// This test is GREEN today and is here as a trap, not as a bug report.
/// Suppression currently runs AFTER validation, so what lands in the cache is
/// always the unsuppressed verdict and the cache key can ignore suppression
/// entirely. The planned change moves suppression into the rule set so that
/// classification happens once, and at that moment the cached verdict starts
/// depending on which codes were active. A cache key that still ignores
/// suppression would then hand a suppressed run's "valid" to an unsuppressed
/// run: the same stale-verdict shape as the rules-fingerprint defect, arrived
/// at from the other direction.
///
/// Both runs share one `CliHarness`, so they share one cache directory. That
/// is the whole point; do not split them.
#[test]
fn a_suppressed_verdict_is_not_served_to_an_unsuppressed_run() -> Result<(), TestError> {
    let harness = crate::common::CliHarness::new()?;
    let corpus = tempdir()?;

    // Its only problems are Phon %x errors, so it is invalid normally and
    // valid under `--suppress xphon`. That difference is what must not leak
    // through the cache.
    crate::common::write_fixture(corpus.path(), "only_xphon.cha", PHON_ILLEGAL_CODE)?;

    let suppressed = harness.run_validate(corpus.path(), &["--suppress", "xphon"])?;
    let suppressed_out = crate::common::stdout_string(&suppressed);
    assert!(
        suppressed_out.contains("Valid: 1"),
        "with xphon suppressed the file must count as valid.\nsummary said:\n{suppressed_out}"
    );

    // Same cache, same file bytes, different active rule set.
    let unsuppressed = harness.run_validate(corpus.path(), &[])?;
    let unsuppressed_out = crate::common::stdout_string(&unsuppressed);
    assert!(
        unsuppressed_out.contains("Invalid: 1"),
        "without suppression the SAME file must be invalid; a cached verdict \
         from the suppressed run must not be reused.\nsummary said:\n{unsuppressed_out}"
    );
    crate::common::assert_failure(
        &unsuppressed,
        "validate without suppression over a file whose only errors are xphon",
    );

    Ok(())
}

/// `--suppress` takes `Vec<String>` and is matched against code strings, so a
/// value naming no real code or group silently suppresses nothing and the run
/// still exits 0.
///
/// That is the worst possible outcome for this flag: the user believes they
/// suppressed something, the tool agrees by saying nothing, and a typo in a
/// CI invocation quietly becomes a no-op that nobody notices until the
/// suppressed diagnostics reappear in someone else's run. A suppression that
/// silently does nothing is indistinguishable from one that worked.
///
/// Verified by hand before writing this test: `chatter validate <clean file>
/// --suppress E9999` exits 0 with no diagnostic about `E9999`.
#[test]
fn an_unknown_suppress_value_is_rejected_rather_than_silently_ignored() -> Result<(), TestError> {
    let harness = crate::common::CliHarness::new()?;
    let corpus = tempdir()?;

    crate::common::write_fixture(corpus.path(), "all_suppressed.cha", PHON_ILLEGAL_CODE)?;

    // `E9999` is not a real error code and `notagroup` is not a real group.
    // Each must be refused at argument-parse time, naming the offending value.
    for bogus in ["E9999", "notagroup"] {
        let output = harness.run_validate(corpus.path(), &["--suppress", bogus])?;
        let combined = format!(
            "{}{}",
            crate::common::stdout_string(&output),
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(
            !output.status.success(),
            "--suppress {bogus} names nothing that exists, so the run must fail \
             rather than silently suppress nothing.\noutput was:\n{combined}"
        );
        assert!(
            combined.contains(bogus),
            "the rejection must name the offending value {bogus} so the user can \
             see which one was wrong.\noutput was:\n{combined}"
        );
    }

    Ok(())
}
