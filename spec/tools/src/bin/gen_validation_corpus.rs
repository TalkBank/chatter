//! Generate the validation-error fixture corpus + a typed expectations manifest
//! from `spec/errors/` (the single source of truth for validation tests).
//!
//! For every `Layer: validation` spec this writes one `.cha` fixture per EXAMPLE
//! into the corpus dir and records, in `manifest.json`, the codes that fixture
//! must produce (the example's own `Expected Error Codes`, not the spec title),
//! its implementation status, and its source spec. The data-driven runner
//! (`validation_error_corpus.rs`) consumes the manifest; no `.rs` test files are
//! generated here anymore.

use anyhow::Result;
use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use generators::spec::error_corpus::{ErrorCorpusExample, ErrorCorpusSpec, Status};
use generators::spec::validation_manifest::{
    FixtureName, ValidationFixtureEntry, ValidationManifest,
};

/// Fallback fixture-name prefix for an example with no codes (should not occur:
/// `parse_markdown` fills every example with at least the title code).
const UNKNOWN_CODE: &str = "UNKNOWN";

/// CLI arguments: the spec directory to read and the corpus directory to write
/// fixtures + `manifest.json` into.
#[derive(Parser)]
#[command(name = "gen_validation_corpus")]
#[command(about = "Generate the validation fixture corpus + manifest from spec/errors")]
struct Args {
    /// Root directory containing error specs.
    #[arg(short, long, default_value = "spec/errors")]
    spec_dir: PathBuf,

    /// Corpus directory for `.cha` fixtures + `manifest.json`.
    #[arg(
        short,
        long,
        default_value = "crates/talkbank-parser-tests/tests/error_corpus/validation_errors"
    )]
    corpus_dir: PathBuf,
}

/// One fixture to write: the CHAT input plus the manifest entry (which carries
/// the unique filename and what the runner must assert). Produced from one spec
/// example.
struct PlannedFixture {
    input: String,
    entry: ValidationFixtureEntry,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let validation_specs: Vec<ErrorCorpusSpec> = ErrorCorpusSpec::load_all(&args.spec_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load error corpus specs: {}", e))?
        .into_iter()
        .filter(|spec| spec.metadata.layer.is_validation())
        .collect();

    let planned = plan_fixtures(&validation_specs);

    // Wipe stale top-level fixtures + manifest, preserving any subdir (e.g.
    // `not_implemented/`), then write the spec-derived fixtures.
    prepare_corpus_dir(&args.corpus_dir)?;
    for fixture in &planned {
        // The input was already stripped of its trailing newline when the chat
        // block was captured in parse_markdown, so write it verbatim.
        fs::write(
            args.corpus_dir.join(fixture.entry.fixture.as_str()),
            &fixture.input,
        )?;
    }

    let mut manifest = ValidationManifest {
        fixtures: planned.into_iter().map(|f| f.entry).collect(),
        implemented_specs_without_examples: validation_specs
            .iter()
            .filter(|spec| spec.metadata.status == Status::Implemented && spec.examples.is_empty())
            .map(ErrorCorpusSpec::source_path_display)
            .collect(),
    };
    manifest
        .fixtures
        .sort_by(|a, b| a.fixture.as_str().cmp(b.fixture.as_str()));
    manifest.implemented_specs_without_examples.sort();

    let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";
    fs::write(args.corpus_dir.join("manifest.json"), &manifest_json)?;

    println!(
        "Wrote {} fixtures + manifest.json to {}",
        manifest.fixtures.len(),
        args.corpus_dir.display()
    );
    if !manifest.implemented_specs_without_examples.is_empty() {
        println!(
            "coverage: {} implemented specs lack examples",
            manifest.implemented_specs_without_examples.len()
        );
    }
    Ok(())
}

/// Plan one fixture per example across all validation specs, assigning each a
/// filename unique within the corpus dir (multi-example specs would otherwise
/// collide on the shared spec title; the per-example code usually disambiguates,
/// and a numeric suffix covers the rest).
fn plan_fixtures(specs: &[ErrorCorpusSpec]) -> Vec<PlannedFixture> {
    let mut used: HashSet<String> = HashSet::new();
    let mut planned = Vec::new();
    for spec in specs {
        // Computed once per spec; every example of the spec shares them.
        let source_spec = spec.source_path_display();
        let status = spec.metadata.status;
        for example in &spec.examples {
            let name = unique_fixture_name(&mut used, &fixture_base(example));
            planned.push(PlannedFixture {
                input: example.input.clone(),
                entry: ValidationFixtureEntry {
                    fixture: FixtureName::new(name),
                    expected_codes: example.expected_codes.clone(),
                    status,
                    source_spec: source_spec.clone(),
                },
            });
        }
    }
    planned
}

/// The `<code>_<sanitized name>` stem for one example (no extension).
fn fixture_base(example: &ErrorCorpusExample) -> String {
    let code = example
        .expected_codes
        .first()
        .map(|c| c.as_str().to_string())
        .unwrap_or_else(|| UNKNOWN_CODE.to_string());
    format!("{}_{}", code, sanitize_filename(&example.name))
}

/// Append `.cha`, disambiguating with a numeric suffix on collision so no
/// fixture silently overwrites another.
fn unique_fixture_name(used: &mut HashSet<String>, base: &str) -> String {
    let mut candidate = format!("{base}.cha");
    let mut n = 2;
    while !used.insert(candidate.clone()) {
        candidate = format!("{base}_{n}.cha");
        n += 1;
    }
    candidate
}

/// Remove every top-level file in the corpus dir (stale fixtures + manifest),
/// leaving subdirectories untouched. Creates the dir if missing.
fn prepare_corpus_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file() {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// Sanitize an example name for use in a fixture filename: non-alphanumerics
/// become underscores, with consecutive underscores collapsed.
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_spec(dir: &Path, name: &str, body: &str) {
        use std::io::Write;
        let mut file = fs::File::create(dir.join(name)).expect("create spec file");
        file.write_all(body.as_bytes()).expect("write spec body");
    }

    #[test]
    fn sanitize_collapses_runs_of_separators() {
        assert_eq!(sanitize_filename("Illegal 'xx' marker"), "Illegal_xx_marker");
        assert_eq!(sanitize_filename("a -- b"), "a_b");
    }

    #[test]
    fn plans_one_fixture_per_example_with_its_own_codes() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A two-example spec whose examples declare different codes.
        write_spec(
            dir.path(),
            "E999_multi.md",
            "# E999: Multi\n\n## Description\n\nDemo.\n\n## Metadata\n\n\
             - **Category**: demo\n- **Level**: utterance\n- **Layer**: validation\n\n\
             ## Example 1\n\n**Expected Error Codes**: E316\n\n```chat\n@UTF8\n@Begin\none\n@End\n```\n\n\
             ## Example 2\n\n**Expected Error Codes**: E600\n\n```chat\n@UTF8\n@Begin\ntwo\n@End\n```\n",
        );
        let specs = ErrorCorpusSpec::load_all(dir.path()).expect("load specs");
        let planned = plan_fixtures(&specs);

        assert_eq!(planned.len(), 2, "one fixture per example");
        let codes: Vec<&str> = planned
            .iter()
            .flat_map(|f| f.entry.expected_codes.iter())
            .map(|c| c.as_str())
            .collect();
        assert!(codes.contains(&"E316") && codes.contains(&"E600"));
        // Distinct codes give distinct filenames.
        assert_ne!(
            planned[0].entry.fixture.as_str(),
            planned[1].entry.fixture.as_str()
        );
        assert!(planned.iter().all(|f| f.entry.status == Status::Implemented));
        assert!(planned[0].entry.source_spec.ends_with("E999_multi.md"));
    }
}
