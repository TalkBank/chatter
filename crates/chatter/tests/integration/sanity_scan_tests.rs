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

//! Integration tests for `chatter sanity-scan`.
//!
//! User contract: a post-merge scan that reads merged CHAT files
//! produced by `chatter batch` (alongside the override file that
//! pass-1 wrote) and flags sessions where the anchor's mean
//! utterance word count exceeds an inserted speaker's by the
//! configured ratio. Flagged sessions become
//! `SanityScanMisclassification` pending-adjudication entries with
//! a swapped suggested mapping.

use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

use crate::common::CliHarness;

#[test]
fn scan_missing_inputs_never_claim_complete_or_change_pending() {
    enum Missing {
        Directory,
        Decisions,
        Transcript,
    }
    let harness = CliHarness::new().unwrap();
    for missing in [Missing::Directory, Missing::Decisions, Missing::Transcript] {
        let dir = tempdir().unwrap();
        let merged = dir.path().join("merged");
        let decisions = dir.path().join("overrides.toml");
        let pending = dir.path().join("pending.toml");
        match missing {
            Missing::Directory => {
                fs::write(&decisions, "schema_version = 2\n").unwrap();
            }
            Missing::Decisions => {
                fs::create_dir(&merged).unwrap();
            }
            Missing::Transcript => {
                fs::create_dir(&merged).unwrap();
                fs::write(&decisions, FIX_OVERRIDE_PASS1_AUTO).unwrap();
            }
        }
        fs::write(&pending, "schema_version = 2\nentries = []\n").unwrap();
        harness
            .chatter_cmd()
            .args(["experimental", "speakers", "scan"])
            .arg(&merged)
            .arg("--override-file")
            .arg(&decisions)
            .args(["--anchor", "CHI", "--write-pending"])
            .arg(&pending)
            .assert()
            .code(1)
            .stdout("");
        assert_eq!(
            fs::read_to_string(&pending).unwrap(),
            "schema_version = 2\nentries = []\n"
        );
    }
}

#[test]
fn scan_pending_aliases_cannot_replace_decisions_or_transcripts() {
    let harness = CliHarness::new().unwrap();
    let dir = tempdir().unwrap();
    let merged = dir.path().join("merged");
    fs::create_dir(&merged).unwrap();
    let transcript = merged.join("session-misclass.cha");
    let decisions = dir.path().join("overrides.toml");
    fs::write(&transcript, FIX_MERGED_INVERTED_MLU).unwrap();
    fs::write(&decisions, FIX_OVERRIDE_PASS1_AUTO).unwrap();
    for source in [&decisions, &transcript] {
        let alias = dir.path().join("pending-alias.toml");
        fs::hard_link(source, &alias).unwrap();
        for destination in [source, &alias] {
            harness
                .chatter_cmd()
                .args(["experimental", "speakers", "scan"])
                .arg(&merged)
                .arg("--override-file")
                .arg(&decisions)
                .args(["--anchor", "CHI", "--write-pending"])
                .arg(destination)
                .assert()
                .code(1)
                .stdout("")
                .stderr(predicates::str::contains("output aliases input"));
        }
        fs::remove_file(alias).unwrap();
    }
    assert_eq!(
        fs::read_to_string(transcript).unwrap(),
        FIX_MERGED_INVERTED_MLU
    );
    assert_eq!(
        fs::read_to_string(decisions).unwrap(),
        FIX_OVERRIDE_PASS1_AUTO
    );
}

/// Invalid heuristic parameters must fail before any input or output I/O.
#[test]
fn sanity_scan_rejects_invalid_threshold_before_io() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let pending = dir.path().join("pending.toml");
    let original = "# preserve review evidence\nschema_version = 2\nentries = []\n";
    fs::write(&pending, original)?;
    for threshold in ["NaN", "inf", "-inf", "0", "-0", "-1"] {
        harness
            .chatter_cmd()
            .args(["experimental", "speakers", "scan"])
            .arg(dir.path().join("missing-inputs"))
            .arg("--override-file")
            .arg(dir.path().join("missing-overrides.toml"))
            .arg("--anchor")
            .arg("CHI")
            .arg(format!("--threshold={threshold}"))
            .arg("--write-pending")
            .arg(&pending)
            .assert()
            .code(2)
            .stderr(predicates::str::contains("threshold"));
        assert_eq!(fs::read_to_string(&pending)?, original);
    }
    Ok(())
}

/// A merged CHAT file with inverted MLU asymmetry: CHI has long
/// utterances (>5 words each) and INV has short ones (1-2 words).
/// Mean ratio CHI:INV ≈ 4×, well above the default 1.5× threshold.
/// Models the case where pass-1 confidently mapped the wrong donor
/// speaker as the anchor's match, child-like donor utterances got
/// renamed to INV while adult-like donor utterances got dropped in
/// favor of the (mis-coded) reference CHI.
const FIX_MERGED_INVERTED_MLU: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, INV Investigator
@ID:\teng|fakecorpus|CHI|3;06.||||Target_Child|||
@ID:\teng|fakecorpus|INV|||||Investigator|||
*CHI:\tthe dog ran across the field very quickly today .
*CHI:\tI went to school yesterday and then I had a nice lunch with my best friend .
*CHI:\twe played outside for a long time before going home to have dinner .
*INV:\tdog .
*INV:\tyes .
*INV:\tmore ?
*INV:\twhat .
@End
";

/// Pre-existing override file produced by pass-1 reference mode.
/// The auto-decision picked PAR0 as the winner (mapped to drop) and
/// renamed PAR1 to INV, but the sanity scan will detect that this
/// looks inverted and suggest swapping.
const FIX_OVERRIDE_PASS1_AUTO: &str = "schema_version = 2

[session-misclass]
mode = \"auto\"
adult_roles = { PAR1 = { code = \"INV\", tag = \"Investigator\" } }
mapping = { PAR0 = \"drop\", PAR1 = \"rename\" }
operator = \"pass1-auto\"
decided_at = \"2026-05-28T11:00:00Z\"
";

/// `chatter sanity-scan` on a merged-dir + override-file containing
/// one auto-decided session with inverted anchor/inserted MLU writes
/// a `SanityScanMisclassification` pending entry whose suggested
/// mapping swaps the original (PAR0 = rename, PAR1 = drop).
#[test]
fn sanity_scan_flags_inverted_mlu() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let merged_dir = dir.path().join("merged");
    let overrides_path = dir.path().join("overrides.toml");
    let pending_path = dir.path().join("pending.toml");
    fs::create_dir_all(&merged_dir)?;
    fs::write(
        merged_dir.join("session-misclass.cha"),
        FIX_MERGED_INVERTED_MLU,
    )?;
    fs::write(&overrides_path, FIX_OVERRIDE_PASS1_AUTO)?;

    harness
        .chatter_cmd()
        .args(["experimental", "speakers", "scan"])
        .arg(&merged_dir)
        .arg("--override-file")
        .arg(&overrides_path)
        .arg("--anchor")
        .arg("CHI")
        .arg("--threshold")
        .arg("1.5")
        .arg("--write-pending")
        .arg(&pending_path)
        .assert()
        // Exit code 4 mirrors speaker-id low-confidence: scan
        // completed but flagged at least one session that needs
        // operator adjudication.
        .code(4)
        .stdout(predicates::str::contains(
            "1 auto-decided session(s) scanned; 1 flagged",
        ));

    assert!(
        pending_path.exists(),
        "pending file should be written when the scan flags a session: {}",
        pending_path.display()
    );
    let pending_text = fs::read_to_string(&pending_path)?;
    assert!(
        pending_text.contains("sanity-scan-misclassification"),
        "pending entry should carry the sanity-scan-misclassification kind discriminator:\n\
         {pending_text}"
    );
    assert!(
        pending_text.contains("session-misclass"),
        "pending entry should carry the session_id of the flagged session:\n{pending_text}"
    );
    // The suggested mapping should be the swap of the override's
    // mapping: PAR0 was drop → suggested rename; PAR1 was rename →
    // suggested drop.
    assert!(
        pending_text.contains("PAR0") && pending_text.contains("PAR1"),
        "pending entry should reference both PAR0 and PAR1 in its suggested mapping:\n\
         {pending_text}"
    );
    // The suggested.inserted_role should ride through from the
    // override entry.
    assert!(
        pending_text.contains("INV") && pending_text.contains("Investigator"),
        "pending entry should carry the inserted_role from the override:\n{pending_text}"
    );
    Ok(())
}
