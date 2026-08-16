//! Does a spec's own example demonstrate the code the spec is for?
//!
//! # The defect this exists to stop growing
//!
//! 152 of the 238 files under `spec/errors/` were produced by
//! `spec/tools/src/bin/corpus_to_specs.rs`, which read a fixture `.cha` from
//! the legacy error corpus, ran it, and wrote down what came out. It named each
//! file after the code the FIXTURE was filed under, and recorded as the
//! expectation whatever chatter actually emitted. Where those two differed,
//! nothing noticed.
//!
//! So `E330_auto.md` is named for E330 and its only example expects E316.
//! `E243_auto.md` expects E202. `E511_auto.md` expects E522 and E523. Forty
//! files are in that state.
//!
//! That is not a cosmetic mismatch. These files are the SPECIFICATION: the
//! generated error tests come from them, and the two-backend parity gate grades
//! both parsers against "what the spec expects". A file whose example never
//! produces its own code documents nothing about that code, while still
//! carrying authority over it. On 2026-08-15 a parity failure reported
//! "spec expects [E522, E523]" for a case named E511 and cost real time before
//! the provenance explained it.
//!
//! # Why a baseline rather than a fix
//!
//! Writing the missing forty is per-rule adjudication: deciding what each rule
//! rejects and why, which is spec work and belongs with the maintainer, not
//! with a mechanical pass. What a gate CAN do is stop the count growing, and
//! make each entry a deliberate act with a name.
//!
//! The list may only shrink. An entry whose spec starts demonstrating its own
//! code FAILS until it is deleted, so a fix cannot land silently, and a new
//! spec that does not demonstrate its code cannot land at all.

use std::collections::BTreeSet;

use crate::error_specs;
use crate::gate::{Gate, GateOutcome, listing, report};

/// Specs whose examples do not demonstrate the code the file is named for.
///
/// Every entry is a spec that documents nothing about its own code. Delete an
/// entry in the commit that gives its spec a real example; the gate fails on a
/// stale entry, so the list cannot drift out of date in the quiet direction.
///
/// Measured 2026-08-15 over 238 spec files: 38 of them. The population is
/// almost entirely `_auto`, which is the generator's fingerprint rather than a
/// coincidence.
///
/// The first cut of this list said 40, from a shell loop that read only the
/// FIRST `**Expected Error Codes**` line per file. Two specs carry several
/// examples and demonstrate their code in a later one, and the gate refused
/// them the moment it ran, which is the whole argument for the check living in
/// code that reads every example rather than in a one-off measurement.
const SPECS_NOT_DEMONSTRATING_THEIR_CODE: &[&str] = &[
    "E003_auto.md",
    "E101_auto.md",
    "E203_auto.md",
    "E208_auto.md",
    "E210_auto.md",
    "E213_auto.md",
    "E231_auto.md",
    "E232_auto.md",
    "E243_auto.md",
    "E253_auto.md",
    "E302_auto.md",
    "E304_auto.md",
    "E307_auto.md",
    "E309_auto.md",
    "E312_auto.md",
    "E313_auto.md",
    "E314_auto.md",
    "E315_auto.md",
    "E323_auto.md",
    "E324_auto.md",
    "E330_auto.md",
    "E344_auto.md",
    "E346_auto.md",
    "E360_auto.md",
    "E360_deprecated_skip_bullet.md",
    "E361_auto.md",
    "E363_auto.md",
    "E364_auto.md",
    "E382_auto.md",
    "E506_auto.md",
    "E508_auto.md",
    "E510_auto.md",
    "E511_auto.md",
    "E512_auto.md",
    "E710_auto.md",
    "E999_auto.md",
];

/// The codes one spec file's examples say they expect.
///
/// Reads every `**Expected Error Codes**:` line, because a spec may carry
/// several examples and only one of them need demonstrate the file's own code.
fn expected_codes(content: &str) -> BTreeSet<String> {
    content
        .lines()
        .filter_map(error_specs::expected_codes_declaration)
        .flat_map(|list| {
            list.split(',')
                .map(|code| code.trim().to_string())
                .filter(|code| !code.is_empty())
        })
        .collect()
}

/// Reference-corpus gate: a spec's example demonstrates its own code.
pub struct SpecSelfDemonstrationGate;

impl Gate for SpecSelfDemonstrationGate {
    fn name(&self) -> &'static str {
        "spec examples demonstrate their own code"
    }

    fn check(&self) -> GateOutcome {
        let dir = crate::repo_paths::workspace_root().join("spec/errors");
        let specs = error_specs::load(&dir).map_err(|e| format!("cannot load specs: {e}"))?;

        let recorded: BTreeSet<&str> = SPECS_NOT_DEMONSTRATING_THEIR_CODE.iter().copied().collect();
        let mut newly_silent = Vec::new();
        let mut now_demonstrating = Vec::new();
        let mut checked = 0usize;

        for spec in &specs {
            // A spec with no example says nothing about expectations either
            // way; that is the `_auto` stub problem, and a different gate's
            // business. This one is only about a spec that HAS an example.
            let expected = expected_codes(&spec.content);
            if expected.is_empty() {
                continue;
            }
            let Some(code) = spec.code() else {
                continue;
            };
            checked += 1;

            let demonstrates = expected.contains(code.as_str());
            let is_recorded = recorded.contains(spec.filename.as_str());

            match (demonstrates, is_recorded) {
                (false, false) => newly_silent.push(format!(
                    "{} expects [{}] and never {}",
                    spec.filename,
                    expected.iter().cloned().collect::<Vec<_>>().join(", "),
                    code.as_str()
                )),
                (true, true) => now_demonstrating.push(spec.filename.clone()),
                _ => {}
            }
        }

        // A recorded entry naming a spec that no longer exists is coverage
        // claimed over nothing, the same rot the parity baseline guards against.
        let filenames: BTreeSet<&str> = specs.iter().map(|s| s.filename.as_str()).collect();
        let stale: Vec<String> = SPECS_NOT_DEMONSTRATING_THEIR_CODE
            .iter()
            .filter(|name| !filenames.contains(*name))
            .map(|name| (*name).to_string())
            .collect();

        // One report, not three early returns. With sequential returns an
        // operator fixes the first failure, re-runs, and only then learns about
        // the second; `report` is the crate's idiom precisely so every section
        // arrives in one run.
        let failures = report([
            listing(
                "FAIL: spec(s) whose example never produces the code the file is named for.\n\
                 A spec that does not demonstrate its own code documents nothing about it,\n\
                 while the generated tests and the backend-parity gate still grade against it.\n\
                 Give the example a case that produces the code, or record it deliberately:",
                &newly_silent,
            ),
            listing(
                "FAIL: recorded as not demonstrating their code, but they now DO.\n\
                 Delete them from SPECS_NOT_DEMONSTRATING_THEIR_CODE in the commit that fixed them:",
                &now_demonstrating,
            ),
            listing("FAIL: recorded spec(s) that no longer exist:", &stale),
        ]);
        if !failures.is_empty() {
            return Err(failures);
        }

        Ok(format!(
            "{checked} spec(s) with examples checked; {} still do not demonstrate their own code",
            SPECS_NOT_DEMONSTRATING_THEIR_CODE.len(),
        ))
    }
}
