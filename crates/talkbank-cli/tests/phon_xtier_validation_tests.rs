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

/// A well-formed Phon export validates cleanly with NO flags (validation is on
/// by default). Post-feature this passes because the tiers are parsed and every
/// rule holds, not because they are ignored.
#[test]
fn phon_xtiers_clean_file_validates() -> Result<(), TestError> {
    let (_dir, path) = write_fixture("clean.cha", PHON_CLEAN)?;
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
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
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
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
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
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
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
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
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
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
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg("--suppress")
        .arg("xphon")
        .arg(&path)
        .assert()
        .success();
    Ok(())
}
