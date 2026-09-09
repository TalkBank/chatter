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

//! Data-driven runner for the generated validation corpus.
//!
//! `just spec-gen` emits one `.cha` fixture per error-spec EXAMPLE (every
//! spec, both pipeline stages, since R4 made the corpus total) plus a
//! `manifest.json` recording each fixture's spec code and its CLAIM. This
//! test:
//!
//! 1. Reads the manifest (fails if missing: regenerate with `just spec-gen`).
//! 2. Parses each implemented fixture with streaming diagnostics, then runs
//!    `validate_with_alignment`, collecting BOTH stages' codes.
//! 3. Judges the fixture's claim via the shared `Claim::satisfied_by`,
//!    negative halves included (`legal`, and `subsumed_by`'s own-code-absent
//!    part).
//! 4. Enforces the manifest's per-code coverage gate
//!    (`implemented_codes_without_examples`), so a newly-implemented rule
//!    cannot silently ship with no triggering example anywhere.
use std::fs;
use std::path::PathBuf;
use talkbank_model::model::TranscriptName;

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

// The manifest types are the GENERATOR'S OWN, read from the crate both cargo
// workspaces share. A hand-written mirror of `ValidationFixtureEntry` and
// `ValidationManifest` stood here until 2026-09-08, matched to the generator
// by field name and by nothing else; its own doc recorded that a previous
// local status enum had already drifted, and adding `rules` to the format
// would have meant adding it here too. Moving the wire types into
// `talkbank-spec-vocabulary` deleted both structs and the drift with them.
use talkbank_spec_vocabulary::SpecErrorCode;
use talkbank_spec_vocabulary::paths::RepoRelativePath;
use talkbank_spec_vocabulary::validation_manifest::ValidationManifest as Manifest;

/// The validation corpus dir under this crate (where the generator writes).
fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/error_corpus/validation_errors")
}

/// Verify each implemented fixture SATISFIES ITS CLAIM, absences included.
#[test]
fn validation_errors_detected() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let dir = corpus_dir();

    let manifest_text = fs::read_to_string(dir.join("manifest.json")).map_err(|err| {
        TestError::Failure(format!(
            "Failed to read manifest.json in {} (regenerate with `just spec-gen`): {err}",
            dir.display()
        ))
    })?;
    let manifest: Manifest = serde_json::from_str(&manifest_text)
        .map_err(|err| TestError::Failure(format!("Failed to parse manifest.json: {err}")))?;

    if manifest.fixtures.is_empty() {
        return Err(TestError::Failure(
            "Validation manifest has no fixtures!".to_string(),
        ));
    }

    println!("Testing {} manifest fixtures...\n", manifest.fixtures.len());

    let mut failures = Vec::new();
    let mut skipped = 0usize;

    for entry in &manifest.fixtures {
        if entry.status != talkbank_spec_vocabulary::Status::Implemented {
            skipped += 1;
            println!(
                "  ⊘ {} → skipped (status: {:?}, {})",
                entry.fixture, entry.status, entry.source_spec
            );
            continue;
        }

        let content = fs::read_to_string(dir.join(&entry.fixture)).map_err(|err| {
            TestError::Failure(format!("Failed to read fixture {}: {err}", entry.fixture))
        })?;

        // Parse with streaming diagnostics so recovered parser errors are visible,
        // then validate. Collect both parse- and validation-level codes.
        let parse_errors = ErrorCollector::new();
        let parse_result = parser.parse_chat_file_fragment(&content, 0, &parse_errors);
        let mut codes: Vec<String> = parse_errors
            .to_vec()
            .iter()
            .map(|e| e.code.to_string())
            .collect();
        if let ParseOutcome::Parsed(mut chat_file) = parse_result {
            let validation_errors = ErrorCollector::new();
            let fixture_path = dir.join(&entry.fixture);
            // The fixture runs under the rules its own code declares. Before
            // the manifest carried them, every fixture ran under the default
            // rule set, so a fixture for an opt-in rule could only ever report
            // nothing; eight such codes were marked `not_implemented` to keep
            // this runner quiet about them.
            chat_file.validate_with_alignment_and_rules(
                entry.rules.selection(),
                &validation_errors,
                TranscriptName::for_path(&fixture_path),
            );
            codes.extend(
                validation_errors
                    .to_vec()
                    .iter()
                    .map(|e| e.code.to_string()),
            );
        }

        // The claim's MEANING lives on the claim (`Claim::satisfied_by`, the
        // one owner in the vocabulary crate); only the rendering of what was
        // wanted is local to this report.
        use talkbank_spec_vocabulary::frontmatter::Claim;
        let satisfied = entry.claim.satisfied_by(&entry.code, |code| {
            codes.iter().any(|got| got == code.as_str())
        });
        let wants = match &entry.claim {
            Claim::Violates => entry.code.as_str().to_owned(),
            Claim::Legal => format!("absence of {}", entry.code.as_str()),
            Claim::SubsumedBy(targets) => format!(
                "{} and absence of {}",
                targets
                    .as_slice()
                    .iter()
                    .map(|target| target.as_str())
                    .collect::<Vec<_>>()
                    .join("+"),
                entry.code.as_str()
            ),
        };
        // THE OTHER HALF OF AN OPT-IN CLAIM, and without it the declaration
        // is decoration. A fixture whose code declares an option is run a
        // SECOND time with no options at all, and the code must be absent.
        //
        // Why it has to exist: everything above runs the fixture only with the
        // option ON, so deleting the four `enable_quotation_validation` guards
        // in `validation/cross_utterance/mod.rs` (or flipping the default in
        // `validation/context.rs`) leaves every fixture emitting its code,
        // every claim satisfied, the observation snapshot byte-identical and
        // the gate green, while eight published pages go on telling readers a
        // default run is silent. This is the assertion those pages rest on.
        //
        // Only `violates` is checked: `legal` and `subsumed_by` already assert
        // the code is absent, so asserting it again under weaker rules says
        // nothing new.
        if matches!(entry.claim, Claim::Violates) && !entry.rules.is_default() {
            let default_errors = ErrorCollector::new();
            let mut default_codes: Vec<String> = Vec::new();
            let parse_errors = ErrorCollector::new();
            if let ParseOutcome::Parsed(mut chat_file) =
                parser.parse_chat_file_fragment(&content, 0, &parse_errors)
            {
                chat_file.validate_with_alignment(
                    &default_errors,
                    TranscriptName::for_path(&dir.join(&entry.fixture)),
                );
                default_codes.extend(
                    parse_errors
                        .to_vec()
                        .iter()
                        .chain(default_errors.to_vec().iter())
                        .map(|e| e.code.to_string()),
                );
            }
            if default_codes.iter().any(|got| got == entry.code.as_str()) {
                failures.push(format!(
                    "{}: {} is declared to need {} but a DEFAULT run reports it \
                     anyway (got {:?}). Either the rule is no longer opt-in, in \
                     which case drop `rules` from its registry entry, or a guard \
                     was lost. Every generated page for this code currently \
                     tells readers a default run does not report it. [{}]",
                    entry.fixture,
                    entry.code.as_str(),
                    entry
                        .rules
                        .cli_flag()
                        .unwrap_or("an option this build does not name"),
                    default_codes,
                    entry.source_spec
                ));
            }
        }

        if satisfied {
            println!("  ✓ {} → {} ({})", wants, entry.fixture, codes.join(", "));
        } else {
            failures.push(format!(
                "{} (claim wants {}, got {:?}) [{}]",
                entry.fixture, wants, codes, entry.source_spec
            ));
            println!(
                "  ✗ {} → {:?} (claim wants {}) [{}]",
                entry.fixture, codes, wants, entry.source_spec
            );
        }
    }

    println!("\nskipped (status not `implemented`): {skipped}");

    // Hard coverage gate: every implemented validation spec must contribute at
    // least one example, so a newly-implemented spec cannot silently ship without
    // a test. The generator records any offenders in the manifest; a
    // non-empty list fails the run alongside any fixture mismatches above.
    let coverage_gaps = &manifest.implemented_codes_without_examples;

    // Collect each non-empty failure category as its own section, then join.
    let mut sections = Vec::new();
    if !failures.is_empty() {
        sections.push(format!(
            "{} validation fixtures did not produce their expected codes:\n  {}",
            failures.len(),
            failures.join("\n  ")
        ));
    }
    if !coverage_gaps.is_empty() {
        sections.push(format!(
            "{} implemented codes have no triggering example in any spec (add a triggering \
             example; or Status: not_implemented with a reason; or, only when \
             no CHAT input can reach the rule at all, Status: \
             unreachable_from_chat naming its out-of-corpus test):\n  {}",
            coverage_gaps.len(),
            // Typed now that the manifest is the generator's own struct rather
            // than a `Vec<String>` mirror: rendered here, never re-parsed.
            coverage_gaps
                .iter()
                .map(SpecErrorCode::as_str)
                .collect::<Vec<_>>()
                .join("\n  ")
        ));
    }
    // The converse of the escape hatch: an `unreachable_from_chat` spec that
    // has an example is reachable, so the status is wrong. Without this the
    // new state would be a way to opt any rule out of its fixture.
    let mislabelled = &manifest.unreachable_specs_with_examples;
    if !mislabelled.is_empty() {
        sections.push(format!(
            "{} specs marked unreachable_from_chat carry an example, so CHAT \
             input does reach them and the status is wrong:\n  {}",
            mislabelled.len(),
            mislabelled
                .iter()
                .map(RepoRelativePath::as_str)
                .collect::<Vec<_>>()
                .join("\n  ")
        ));
    }
    if !sections.is_empty() {
        return Err(TestError::Failure(sections.join("\n")));
    }

    println!("\n✓ All implemented validation fixtures produced their expected codes");
    println!("✓ Every implemented validation spec contributes at least one example");
    Ok(())
}
