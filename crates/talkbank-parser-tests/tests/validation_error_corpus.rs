//! Validation error corpus, driven by the spec-generated manifest.
//!
//! `spec/errors/` is the single source of truth. The generator
//! (`gen_validation_corpus`) emits one `.cha` fixture per validation example plus
//! a `manifest.json` recording the codes each fixture must produce and the
//! source spec's implementation status. This runner reads that manifest, skips
//! any fixture whose status is not `implemented` (e.g. `not_implemented`,
//! `deprecated`), and asserts every other fixture produces all its expected
//! codes.
//!
//! ## Test strategy
//! 1. Parse the fixture (syntactically valid CHAT that should parse).
//! 2. Run `validate_with_alignment`.
//! 3. Assert every expected code appears among the parse + validation diagnostics.
//! 4. Hard coverage gate: fail if any implemented validation spec contributed no
//!    example (`manifest.implemented_specs_without_examples`), so a new
//!    implemented spec cannot ship without a test.
//!
//! ## Usage
//! ```bash
//! cargo nextest run -p talkbank-parser-tests -E 'test(validation_errors_detected)'
//! ```

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Implementation status carried in the manifest. The runner asserts only
/// `Implemented` fixtures and skips the rest (`not_implemented`, `deprecated`).
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum FixtureStatus {
    Implemented,
    NotImplemented,
    Deprecated,
}

/// An expected error/warning code from the manifest, compared against the
/// runtime diagnostic codes the parser/validator produces.
#[derive(Deserialize)]
#[serde(transparent)]
struct ExpectedCode(String);

impl ExpectedCode {
    fn as_str(&self) -> &str {
        &self.0
    }
}

/// One fixture's expectations, mirrored from the generator's
/// `ValidationFixtureEntry`.
#[derive(Deserialize)]
struct ManifestEntry {
    fixture: String,
    expected_codes: Vec<ExpectedCode>,
    status: FixtureStatus,
    source_spec: String,
}

/// The corpus manifest written by `gen_validation_corpus`.
#[derive(Deserialize)]
struct Manifest {
    fixtures: Vec<ManifestEntry>,
    /// Implemented validation specs that produced no example. Reported as a
    /// coverage warning (report-only gate).
    #[serde(default)]
    implemented_specs_without_examples: Vec<String>,
}

/// The validation corpus dir under this crate (where the generator writes).
fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/error_corpus/validation_errors")
}

/// Verify each implemented validation fixture produces all its expected codes.
#[test]
fn validation_errors_detected() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let dir = corpus_dir();

    let manifest_text = fs::read_to_string(dir.join("manifest.json")).map_err(|err| {
        TestError::Failure(format!(
            "Failed to read manifest.json in {} (regenerate with gen_validation_corpus): {err}",
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
        if entry.status != FixtureStatus::Implemented {
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
            let stem = fixture_path.file_stem().and_then(|s| s.to_str());
            chat_file.validate_with_alignment(&validation_errors, stem);
            codes.extend(
                validation_errors
                    .to_vec()
                    .iter()
                    .map(|e| e.code.to_string()),
            );
        }

        for expected in &entry.expected_codes {
            if codes.iter().any(|code| code == expected.as_str()) {
                println!(
                    "  ✓ {} → {} ({})",
                    expected.as_str(),
                    entry.fixture,
                    codes.join(", ")
                );
            } else {
                failures.push(format!(
                    "{} (expected {}, got {:?}) [{}]",
                    entry.fixture,
                    expected.as_str(),
                    codes,
                    entry.source_spec
                ));
                println!(
                    "  ✗ {} → {:?} (expected {}) [{}]",
                    entry.fixture,
                    codes,
                    expected.as_str(),
                    entry.source_spec
                );
            }
        }
    }

    println!("\nskipped (status not `implemented`): {skipped}");

    // Hard coverage gate: every implemented validation spec must contribute at
    // least one example, so a newly-implemented spec cannot silently ship without
    // a test. `gen_validation_corpus` records any offenders in the manifest; a
    // non-empty list fails the run alongside any fixture mismatches above.
    let coverage_gaps = &manifest.implemented_specs_without_examples;

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
            "{} implemented validation specs lack examples (add a triggering \
             example, or set Status: not_implemented with a reason):\n  {}",
            coverage_gaps.len(),
            coverage_gaps.join("\n  ")
        ));
    }
    if !sections.is_empty() {
        return Err(TestError::Failure(sections.join("\n")));
    }

    println!("\n✓ All implemented validation fixtures produced their expected codes");
    println!("✓ Every implemented validation spec contributes at least one example");
    Ok(())
}
