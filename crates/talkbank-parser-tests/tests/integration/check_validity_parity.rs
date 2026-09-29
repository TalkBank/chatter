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
//! This suite checks Chatter against adjudicated expectations, not CHECK as an
//! oracle. The authoritative policy is in the manifest and generated book. The
//! parity audit (`bin/audit_check_parity.rs`) only *maps* CHECK numbers to
//! chatter codes by name; it never runs the two validators on the same input.
//! Here each CLAN CHECK number is grounded against a real `.cha` fixture and the
//! two validators' behaviour is compared.
//!
//! Both sides evolve independently (CLAN CHECK is maintained upstream, we
//! maintain chatter), so two tests guard the ledger in `check_parity/manifest.json`:
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
use std::process::{Command, Output};
use talkbank_model::model::TranscriptName;

use serde::Deserialize;
use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_model::Severity;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;
use talkbank_spec_vocabulary::check_assessment::{
    NoObligationReason, ParityEntry, ParityManifest, ParityStatus,
};

/// `tests/check_parity/` under this crate.
fn parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/check_parity")
}

/// `clan-check-reference/` under this crate: the generated view of `check.cpp`.
fn clan_reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("clan-check-reference")
}

/// One CHECK code as the generator sees it in `check.cpp`.
#[derive(Deserialize)]
struct ReferenceCode {
    code: u16,
    /// Live `check_err(N, ...)` sites: call sites inside comments do NOT count.
    n_call_sites: usize,
}

#[derive(Deserialize)]
struct ClanReference {
    codes: Vec<ReferenceCode>,
}

fn load_clan_reference() -> Result<ClanReference, TestError> {
    let path = clan_reference_dir().join("check-error-codes.json");
    let text = fs::read_to_string(&path)
        .map_err(|e| TestError::Failure(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| TestError::Failure(format!("parse {}: {e}", path.display())))
}

/// Whether an entry claims `check.cpp` still emits its code.
///
/// Derived from the status by [`ParityStatus::expected_sites`], so the gate is
/// one comparison rather than a branch per case. Each case's remediation advice
/// survives as data beside the expectation; it was the four copies of the
/// control flow, not the four distinct hints, that were the duplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SiteExpectation {
    /// The reference must show at least one live `check_err` call site.
    Live,
    /// The reference must show none.
    None,
}

impl SiteExpectation {
    /// The claim being tested, phrased for the failure message.
    fn claim(self) -> &'static str {
        match self {
            Self::Live => "this entry asserts CLAN emits the code",
            Self::None => "this entry asserts `check.cpp` no longer emits the code",
        }
    }
}

trait ExpectedSites {
    fn expected_sites(&self) -> (SiteExpectation, &'static str);
}

impl ExpectedSites for ParityStatus {
    /// What this entry implies about the generated reference, and what to do
    /// when the reference disagrees.
    fn expected_sites(&self) -> (SiteExpectation, &'static str) {
        match self {
            Self::Parity | Self::Gap | Self::Divergence => (
                SiteExpectation::Live,
                "Either CLAN retired it (move the entry to `no_obligation` with a reason), \
                 or it is reached through a wrapper the generator does not yet follow \
                 (teach `find_code_carrying_aliases`, do not add an exception here).",
            ),
            Self::NoObligation {
                no_obligation_reason,
            } => match no_obligation_reason {
                // Claims about the SOURCE, which the generated reference can
                // confirm because the generator excludes exactly these two
                // kinds of not-compiled text: comments, and the preprocessor
                // regions a unix build drops.
                NoObligationReason::CommentedOut
                | NoObligationReason::NoEmissionPath
                | NoObligationReason::GuiOnly => (
                    SiteExpectation::None,
                    "Either the reference is stale (regenerate it from the current CLAN \
                     sources with scripts/extract_check_codes.py) or the reason is wrong.",
                ),
                // A claim about RUNTIME reachability: the call site is real and
                // compiled, so the reference counts it; only running CLAN shows
                // it never fires.
                NoObligationReason::UnreachableInFileMode => (
                    SiteExpectation::Live,
                    "This reason claims the call site EXISTS but never fires, so the \
                     reference should still count it. If the site is gone, the reason \
                     should be `commented_out` or `no_emission_path`.",
                ),
            },
        }
    }
}

/// The manifest's claims about CLAN must agree with the generated view of
/// `check.cpp`. Runs in CI; needs no CLAN binary, only the checked-in reference.
///
/// This is the gate that was missing. Two independent artifacts describe the
/// same fact (which codes CLAN can still emit): this manifest, hand-maintained
/// as codes are adjudicated, and `clan-check-reference/check-error-codes.json`,
/// generated from `check.cpp`. Nothing compared them, so they drifted, and the
/// drift was invisible in both directions:
///
/// - eleven entries said "call site commented out" while the reference still
///   reported live call sites, because the generator stripped `//` comments but
///   not `/* ... */` blocks;
/// - CHECK 76 was retired upstream on 2026-08-07 inside such a block, and the
///   manifest went on claiming `parity` with a code no build can emit.
///
/// Note what this does NOT assert: that chatter is right, or that CLAN is
/// right. Only that our two records of CLAN's behaviour say the same thing.
/// Whether CLAN's behaviour is CORRECT is a separate question, answered by
/// adjudication, and whether CLAN still behaves as recorded is answered by
/// `clan_check_grounding` against the real binary.
#[test]
fn manifest_agrees_with_clan_reference() -> Result<(), TestError> {
    let manifest = load_manifest()?;
    let reference = load_clan_reference()?;

    let live_sites: std::collections::HashMap<u16, usize> = reference
        .codes
        .iter()
        .map(|c| (c.code, c.n_call_sites))
        .collect();

    let mut failures = Vec::new();

    for entry in &manifest.entries {
        let code = entry.check_code;
        // A code the reference does not mention at all is one `check.cpp` never
        // names; treat it as zero live sites rather than skipping, so a typo in
        // a manifest code number surfaces here instead of passing vacuously.
        let sites = live_sites.get(&code).copied().unwrap_or(0);
        let (expected, hint) = entry.status.expected_sites();

        let disagrees = match expected {
            SiteExpectation::Live => sites == 0,
            SiteExpectation::None => sites != 0,
        };
        if disagrees {
            failures.push(format!(
                "CHECK {code}: {} but the generated reference reports {sites} live \
                 call site(s). {hint}",
                expected.claim()
            ));
        }
    }

    if !failures.is_empty() {
        return Err(TestError::Failure(format!(
            "{} manifest entr(ies) disagree with the generated CLAN reference:\n  {}",
            failures.len(),
            failures.join("\n  ")
        )));
    }
    Ok(())
}

fn load_manifest() -> Result<ParityManifest, TestError> {
    let path = parity_dir().join("manifest.json");
    let text = fs::read_to_string(&path)
        .map_err(|e| TestError::Failure(format!("read {}: {e}", path.display())))?;
    let manifest: ParityManifest = serde_json::from_str(&text)
        .map_err(|e| TestError::Failure(format!("parse {}: {e}", path.display())))?;
    manifest
        .assess()
        .map_err(|e| TestError::Failure(e.to_owned()))?;
    Ok(manifest)
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
        chat_file
            .validate_with_alignment(&validation_errors, TranscriptName::for_path(path.as_ref()));
        codes.extend(error_severity_codes(&validation_errors));
    }
    Ok(codes)
}

/// Push a failure for every code in `expected_chatter_codes` that chatter did
/// not emit. Shared by the `Parity` arm and the chatter-stricter `Divergence`
/// shape, which assert the same thing.
fn report_missing_expected_codes(
    entry: &ParityEntry,
    fixture: &str,
    codes: &[String],
    failures: &mut Vec<String>,
) {
    for expected in &entry.expected_chatter_codes {
        if !codes.contains(expected) {
            failures.push(format!(
                "CHECK {} ({}): expected chatter {} but got {:?} [{}]",
                entry.check_code, fixture, expected, codes, entry.note
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
    fixture: &str,
    codes: &[String],
    failures: &mut Vec<String>,
    marked: &str,
    hint: &str,
) {
    if !codes.is_empty() {
        failures.push(format!(
            "CHECK {} ({}): {} but chatter now emits {:?} -- {} [{}]",
            entry.check_code, fixture, marked, codes, hint, entry.note
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
        let Some(fixture) = &entry.fixture else {
            // Only `no_obligation` may go fixture-less (nothing groundable
            // exists); any other status without a fixture is an authoring
            // error, fail closed rather than silently skipping.
            if !matches!(entry.status, ParityStatus::NoObligation { .. }) {
                failures.push(format!(
                    "CHECK {}: entry has no fixture but status {:?}; only \
                     `no_obligation` entries may omit the fixture [{}]",
                    entry.check_code, entry.status, entry.note
                ));
            }
            continue;
        };
        let codes = chatter_codes(&parser, fixture)?;
        match entry.status {
            ParityStatus::Parity => {
                report_missing_expected_codes(entry, fixture, &codes, &mut failures);
            }
            ParityStatus::Divergence => {
                // Two shapes; see `ParityStatus::Divergence`. Empty expected codes
                // means chatter intentionally accepts (must validate clean);
                // non-empty means it rejects via those codes (like parity).
                if entry.expected_chatter_codes.is_empty() {
                    report_unexpected_codes(
                        entry,
                        fixture,
                        &codes,
                        &mut failures,
                        "marked intentional `divergence` (chatter accepts)",
                        "reassess the divergence, not a gap",
                    );
                } else {
                    report_missing_expected_codes(entry, fixture, &codes, &mut failures);
                }
            }
            // chatter does not yet catch this CLAN rule, so the fixture must
            // validate clean; emitting a code here fails the test and prompts a
            // flip of the manifest entry to `parity`.
            ParityStatus::Gap => report_unexpected_codes(
                entry,
                fixture,
                &codes,
                &mut failures,
                "marked `gap`",
                "close the gap and flip the manifest entry to `parity`",
            ),
            // CLAN cannot emit the code, but the fixture still pins chatter's
            // recorded behaviour on the construct: same two shapes as
            // `Divergence` (codes present, or clean when the list is empty).
            ParityStatus::NoObligation { .. } => {
                if entry.expected_chatter_codes.is_empty() {
                    report_unexpected_codes(
                        entry,
                        fixture,
                        &codes,
                        &mut failures,
                        "marked `no_obligation` with chatter accepting",
                        "re-adjudicate the entry if chatter now rejects",
                    );
                } else {
                    report_missing_expected_codes(entry, fixture, &codes, &mut failures);
                }
            }
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
/// file-mode pty wrapper (`clan-run.sh`). Explicitly selecting this ignored test
/// requires that configuration; an unexecuted audit cannot pass. Run on every
/// new CLAN bundle to catch CLAN-side drift.
#[test]
#[ignore = "requires real CLAN CHECK via CHATTER_CLAN_RUN (file-mode pty wrapper)"]
fn clan_check_grounding() -> Result<(), TestError> {
    let wrapper = std::env::var_os("CHATTER_CLAN_RUN").ok_or_else(|| {
        TestError::Failure("CHATTER_CLAN_RUN must name the file-mode CLAN wrapper when explicitly running clan_check_grounding".to_owned())
    })?;
    let manifest = load_manifest()?;
    let mut failures = Vec::new();
    for entry in &manifest.entries {
        let Some(fixture_name) = &entry.fixture else {
            // Fixture-less entries carry no CLAN-side obligation; the CI gate
            // already fails closed if the status is not `no_obligation`.
            continue;
        };
        let fixture = parity_dir().join("fixtures").join(fixture_name);
        let output = Command::new("bash")
            .arg(&wrapper)
            .arg("check")
            .args(&entry.clan_flags)
            .arg(&fixture)
            .output()
            .map_err(|e| TestError::Failure(format!("run CLAN wrapper: {e}")))?;
        // Both positive and negative obligations require an observed CHECK
        // run. A wrapper refusal must not satisfy a no-obligation tripwire.
        let observation = CheckObservation::try_from(output).map_err(|error| {
            TestError::Failure(format!(
                "CHECK {} ({fixture_name}): {error}",
                entry.check_code
            ))
        })?;
        let text = &observation.text;
        let emitted = &observation.emitted;
        if matches!(entry.status, ParityStatus::NoObligation { .. }) {
            // Tripwire, inverted assertion: unix CLAN must still NOT emit the
            // code on this construct. If a future CLAN bundle revives the rule
            // (uncomments the call site, compiles the GUI-only region into the
            // unix build, changes the preempting control flow), this fires and
            // the entry must be re-adjudicated as parity/gap/divergence.
            if emitted.contains(&entry.check_code) {
                failures.push(format!(
                    "CHECK {} ({}): marked `no_obligation` but real CLAN CHECK now \
                     EMITS it (got {:?}). CLAN revived the rule; re-adjudicate the \
                     entry. [{}]",
                    entry.check_code, fixture_name, emitted, entry.note
                ));
            }
            continue;
        }
        let grounded = if entry.banner_only {
            // Sound only when CHECK is restricted to exactly this code;
            // otherwise any unrelated error would ground it vacuously.
            let exclusive_flag = format!("+e{}", entry.check_code);
            if entry.clan_flags != [exclusive_flag] {
                failures.push(format!(
                    "CHECK {} ({}): banner_only requires clan_flags == [\"+e{}\"], got {:?}",
                    entry.check_code, fixture_name, entry.check_code, entry.clan_flags
                ));
                continue;
            }
            text.contains("THERE WERE SOME ERROR(S) FOUND")
        } else if entry.no_numeric_suffix {
            text.contains(entry.check_message.trim())
        } else {
            emitted.contains(&entry.check_code)
        };
        if !grounded {
            failures.push(format!(
                "CHECK {} ({}): real CLAN CHECK no longer emits it (got {:?}). CLAN drifted; \
                 re-ground the fixture/mapping.",
                entry.check_code, fixture_name, emitted
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

/// Admitted output of a successful wrapper invocation that actually ran CHECK.
///
/// This is an observation, not proof of a complete second pass: CHECK may
/// stop after reporting a first-pass error. Numeric diagnostics still witness
/// that route. Empty output, a wrapper refusal and a bare startup banner do not.
struct CheckObservation {
    text: String,
    emitted: Vec<u16>,
}

impl TryFrom<Output> for CheckObservation {
    type Error = TestError;

    fn try_from(output: Output) -> Result<Self, Self::Error> {
        if !output.status.success() {
            return Err(TestError::Failure(format!(
                "CLAN wrapper failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim(),
            )));
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|error| TestError::Failure(format!("CHECK output is not UTF-8: {error}")))?
            .replace('\r', "");
        let started = text.lines().any(|line| {
            line.starts_with("check (") && line.ends_with(") is conducting analyses on:")
        });
        let emitted = parse_check_numbers(&text);
        let reported = !emitted.is_empty()
            || text.contains("ALL FILES CHECKED OUT OK!")
            || text.contains("THERE WERE SOME ERROR(S) FOUND")
            || text.contains("No errors other than excluded by ");
        if !started || !reported {
            return Err(TestError::Failure(
                "wrapper output does not witness a CHECK result".to_owned(),
            ));
        }
        Ok(Self { text, emitted })
    }
}

/// External-output admission is a wire boundary, not a CHAT model constructor.
#[test]
fn check_observation_requires_a_result_not_just_successful_transport() {
    let admit = |stdout: &[u8]| {
        CheckObservation::try_from(Output {
            status: Default::default(),
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
        })
    };
    let banner = "check (21-Sep-2026) is conducting analyses on:\r\n";
    for missing in [
        "",
        banner,
        "ALL FILES CHECKED OUT OK!",
        "wrapper: REFUSED (12)",
    ] {
        assert!(admit(missing.as_bytes()).is_err(), "accepted {missing:?}");
    }
    assert!(admit(&[0xff]).is_err());
    for result in [
        "ALL FILES CHECKED OUT OK!",
        "THERE WERE SOME ERROR(S) FOUND.",
        "No errors other than excluded by +e option were found.",
        "Missing required header.(77)",
    ] {
        let observation = admit(format!("{banner}{result}").as_bytes()).unwrap();
        assert!(!observation.text.contains('\r'));
        assert_eq!(observation.emitted, parse_check_numbers(result));
    }
}

#[cfg(unix)]
#[test]
fn check_observation_rejects_wrapper_failure_even_with_plausible_stdout() {
    use std::os::unix::process::ExitStatusExt;

    assert!(
        CheckObservation::try_from(Output {
            status: std::process::ExitStatus::from_raw(12 << 8),
            stdout: b"check (21-Sep-2026) is conducting analyses on:\nALL FILES CHECKED OUT OK!"
                .to_vec(),
            stderr: b"wrapper refused a stale executable".to_vec(),
        })
        .is_err()
    );
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
