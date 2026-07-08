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

//! Behavioral CHECK-validity parity suite.
//!
//! This is the formal, drift-detecting test suite for "does chatter's validator
//! agree with CLAN CHECK on whether a file is valid CHAT". It exists because the
//! parity audit (`bin/audit_check_parity.rs`) only *maps* CHECK numbers to
//! chatter codes by name; it never runs the two validators on the same input.
//! Here each CLAN CHECK number is grounded against a real `.cha` fixture and the
//! two validators' behaviour is compared.
//!
//! Both sides evolve independently (Leonid maintains CLAN CHECK, we maintain
//! chatter), so two tests guard the ledger in `check_parity/manifest.json`:
//!
//! - [`chatter_matches_check`] (CI, no CLAN needed) asserts chatter's behaviour
//!   on each fixture matches the manifest `status`. Catches OUR drift, and is the
//!   gating test.
//! - [`clan_check_grounding`] (`#[ignore]`, CLAN-gated) runs the REAL CLAN CHECK
//!   on each fixture via the file-mode pty wrapper named by `CHATTER_CLAN_RUN`
//!   and asserts it still emits the manifest `check_code`. Catches LEONID's
//!   drift; re-run it whenever a new CLAN bundle lands. CLAN CHECK must be run in
//!   file mode (the wrapper allocates a pty); stdin mode silently runs a weaker
//!   validation, which is why a naive runner would be wrong here.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;
use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_model::Severity;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// How chatter is expected to behave on a fixture relative to CLAN CHECK.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ParityStatus {
    /// chatter flags an equivalent error: `expected_chatter_codes` must appear.
    Parity,
    /// CLAN flags it but chatter does not yet: the fixture must validate clean.
    Gap,
    /// Intentional divergence from CLAN, in one of two shapes:
    /// - chatter rejects differently or more strictly: `expected_chatter_codes`
    ///   lists the codes it must emit (asserted present, like `Parity`).
    /// - chatter intentionally ACCEPTS what CLAN rejects, because the CHECK code
    ///   is a CLAN-internal concern rather than a CHAT-validity rule (e.g. CHECK
    ///   109, a postcode on a dependent tier: CLAN's own analysis tools do not
    ///   choke on it, only CHECK flags it): `expected_chatter_codes` is empty and
    ///   the fixture must validate clean. Unlike `Gap`, this clean state is a
    ///   permanent intentional choice, not a defect to close.
    Divergence,
}

/// One CHECK number grounded against a fixture.
#[derive(Deserialize)]
struct ParityEntry {
    check_code: u16,
    fixture: String,
    status: ParityStatus,
    #[serde(default)]
    expected_chatter_codes: Vec<String>,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
struct ParityManifest {
    entries: Vec<ParityEntry>,
}

/// `tests/check_parity/` under this crate.
fn parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/check_parity")
}

fn load_manifest() -> Result<ParityManifest, TestError> {
    let path = parity_dir().join("manifest.json");
    let text = fs::read_to_string(&path)
        .map_err(|e| TestError::Failure(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| TestError::Failure(format!("parse {}: {e}", path.display())))
}

/// Collect the error-severity diagnostic codes from a collector.
///
/// Parity is about VALIDITY: does chatter reject the file? Only error-severity
/// diagnostics invalidate a file (a `chatter validate` run reports `Valid: 0`
/// iff at least one Error is present). Warnings do NOT make a file invalid, so
/// they must NOT count as "chatter flags this CLAN rule": collecting warnings
/// would let a warning-only diagnostic masquerade as parity (e.g. E605 was a
/// Warning, so an undeclared dependent tier looked matched while the file still
/// validated clean).
fn error_severity_codes(collector: &ErrorCollector) -> Vec<String> {
    collector
        .to_vec()
        .iter()
        .filter(|e| e.severity == Severity::Error)
        .map(|e| e.code.to_string())
        .collect()
}

/// Run chatter's parser + validator on a fixture and return the diagnostic codes.
fn chatter_codes(parser: &TreeSitterParser, fixture: &str) -> Result<Vec<String>, TestError> {
    let path = parity_dir().join("fixtures").join(fixture);
    let content = fs::read_to_string(&path)
        .map_err(|e| TestError::Failure(format!("read fixture {fixture}: {e}")))?;
    let parse_errors = ErrorCollector::new();
    let outcome = parser.parse_chat_file_fragment(&content, 0, &parse_errors);
    let mut codes = error_severity_codes(&parse_errors);
    if let ParseOutcome::Parsed(mut chat_file) = outcome {
        let validation_errors = ErrorCollector::new();
        let stem = path.file_stem().and_then(|s| s.to_str());
        chat_file.validate_with_alignment(&validation_errors, stem);
        codes.extend(error_severity_codes(&validation_errors));
    }
    Ok(codes)
}

/// Push a failure for every code in `expected_chatter_codes` that chatter did
/// not emit. Shared by the `Parity` arm and the chatter-stricter `Divergence`
/// shape, which assert the same thing.
fn report_missing_expected_codes(
    entry: &ParityEntry,
    codes: &[String],
    failures: &mut Vec<String>,
) {
    for expected in &entry.expected_chatter_codes {
        if !codes.contains(expected) {
            failures.push(format!(
                "CHECK {} ({}): expected chatter {} but got {:?} [{}]",
                entry.check_code, entry.fixture, expected, codes, entry.note
            ));
        }
    }
}

/// Push a failure if chatter emitted any error code on a fixture that must
/// validate clean. Shared by the `Gap` arm and the chatter-accepts `Divergence`
/// shape; `marked` and `hint` carry each status's distinct message (a gap is a
/// defect to close, an accept-divergence is a permanent intentional state).
fn report_unexpected_codes(
    entry: &ParityEntry,
    codes: &[String],
    failures: &mut Vec<String>,
    marked: &str,
    hint: &str,
) {
    if !codes.is_empty() {
        failures.push(format!(
            "CHECK {} ({}): {} but chatter now emits {:?} -- {} [{}]",
            entry.check_code, entry.fixture, marked, codes, hint, entry.note
        ));
    }
}

/// CI gate: chatter's behaviour on every grounded fixture must match its
/// manifest `status`. No CLAN binary required.
#[test]
fn chatter_matches_check() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|e| TestError::ParserInit(e.to_string()))?;
    let manifest = load_manifest()?;
    assert!(!manifest.entries.is_empty(), "parity manifest is empty");

    let mut failures = Vec::new();
    for entry in &manifest.entries {
        let codes = chatter_codes(&parser, &entry.fixture)?;
        match entry.status {
            ParityStatus::Parity => report_missing_expected_codes(entry, &codes, &mut failures),
            ParityStatus::Divergence => {
                // Two shapes; see `ParityStatus::Divergence`. Empty expected codes
                // means chatter intentionally accepts (must validate clean);
                // non-empty means it rejects via those codes (like parity).
                if entry.expected_chatter_codes.is_empty() {
                    report_unexpected_codes(
                        entry,
                        &codes,
                        &mut failures,
                        "marked intentional `divergence` (chatter accepts)",
                        "reassess the divergence, not a gap",
                    );
                } else {
                    report_missing_expected_codes(entry, &codes, &mut failures);
                }
            }
            // chatter does not yet catch this CLAN rule, so the fixture must
            // validate clean; emitting a code here fails the test and prompts a
            // flip of the manifest entry to `parity`.
            ParityStatus::Gap => report_unexpected_codes(
                entry,
                &codes,
                &mut failures,
                "marked `gap`",
                "close the gap and flip the manifest entry to `parity`",
            ),
        }
    }

    if !failures.is_empty() {
        return Err(TestError::Failure(format!(
            "{} CHECK-parity fixture(s) drifted from the manifest:\n  {}",
            failures.len(),
            failures.join("\n  ")
        )));
    }
    Ok(())
}

/// Re-grounding gate (CLAN-gated, `#[ignore]`): the REAL CLAN CHECK must still
/// emit each fixture's `check_code`. Set `CHATTER_CLAN_RUN` to the path of the
/// file-mode pty wrapper (`clan-run.sh`); when unset the test no-ops so CI and
/// non-CLAN machines stay green. Run on every new CLAN bundle to catch CLAN-side
/// drift.
#[test]
#[ignore = "requires real CLAN CHECK via CHATTER_CLAN_RUN (file-mode pty wrapper)"]
fn clan_check_grounding() -> Result<(), TestError> {
    let Some(wrapper) = std::env::var_os("CHATTER_CLAN_RUN") else {
        eprintln!("CHATTER_CLAN_RUN unset; skipping CLAN-side grounding.");
        return Ok(());
    };
    let manifest = load_manifest()?;
    let mut failures = Vec::new();
    for entry in &manifest.entries {
        let fixture = parity_dir().join("fixtures").join(&entry.fixture);
        let output = Command::new("bash")
            .arg(&wrapper)
            .arg("check")
            .arg(&fixture)
            .output()
            .map_err(|e| TestError::Failure(format!("run CLAN wrapper: {e}")))?;
        // The pty emits CRLF; strip CR before scanning the (NN) trailers.
        let text = String::from_utf8_lossy(&output.stdout).replace('\r', "");
        let emitted = parse_check_numbers(&text);
        if !emitted.contains(&entry.check_code) {
            failures.push(format!(
                "CHECK {} ({}): real CLAN CHECK no longer emits it (got {:?}). CLAN drifted; \
                 re-ground the fixture/mapping.",
                entry.check_code, entry.fixture, emitted
            ));
        }
    }
    if !failures.is_empty() {
        return Err(TestError::Failure(format!(
            "{} fixture(s) drifted vs real CLAN CHECK:\n  {}",
            failures.len(),
            failures.join("\n  ")
        )));
    }
    Ok(())
}

/// Extract the trailing `(NN)` CHECK error numbers from CLAN CHECK output.
fn parse_check_numbers(text: &str) -> Vec<u16> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(open) = line.rfind('(')
            && let Some(close) = line[open..].find(')')
            && let Ok(n) = line[open + 1..open + close].parse::<u16>()
        {
            out.push(n);
        }
    }
    out
}
