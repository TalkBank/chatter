//! Build the validation-error fixture corpus and its typed manifest from
//! `spec/errors/`.
//!
//! For every `Layer: validation` spec this produces one `.cha` fixture per
//! EXAMPLE, plus `manifest.json` recording the codes that fixture must produce
//! (the example's own `Expected Error Codes`, not the spec title), its
//! implementation status, and its source spec. The data-driven runner
//! (`validation_error_corpus.rs`) consumes the manifest.
//!
//! # Why this is a library module rather than a binary's `main`
//!
//! It lived in `gen_validation_corpus`'s `main`, which meant the only way to
//! ask "what SHOULD this corpus contain" was to write it somewhere and look.
//! A currency gate cannot be built on that: it would have to generate into a
//! temporary directory and compare trees, so the check would need write access
//! to answer a read-only question. Returning the files instead lets
//! [`crate::artifacts`] both write them and compare them, from one description.

use std::collections::HashSet;

use crate::artifacts::GeneratedFiles;
use crate::repo_paths::RepoRelativePath;
use crate::spec::error_corpus::{ErrorCorpusExample, ErrorCorpusSpec};
use crate::spec::metadata::Status;
use crate::spec::validation_manifest::{FixtureName, ValidationFixtureEntry, ValidationManifest};

/// Fallback fixture-name prefix for an example with no codes (should not occur:
/// `parse_markdown` fills every example with at least the title code).
const UNKNOWN_CODE: &str = "UNKNOWN";

/// One fixture to write: the CHAT input plus the manifest entry (which carries
/// the unique filename and what the runner must assert). Produced from one spec
/// example.
struct PlannedFixture {
    input: String,
    entry: ValidationFixtureEntry,
}

/// Build every file of the validation corpus, including `manifest.json`.
///
/// Reads `repo_root/spec/errors`. Takes the repository root rather than the
/// spec directory, because every recorded `source_spec` must be relative to
/// that root and nothing else can compute it.
pub fn build(repo_root: &std::path::Path) -> anyhow::Result<GeneratedFiles> {
    let validation_specs: Vec<ErrorCorpusSpec> =
        ErrorCorpusSpec::load_all(crate::artifacts::error_dir(repo_root))
            .map_err(|e| anyhow::anyhow!("Failed to load error corpus specs: {}", e))?
            .into_iter()
            .filter(|spec| spec.metadata.layer.is_validation())
            .collect();

    let planned = plan_fixtures(&validation_specs, repo_root);

    let mut files = GeneratedFiles::new();
    for fixture in &planned {
        // The input was already stripped of its trailing newline when the chat
        // block was captured in parse_markdown, so store it verbatim.
        files.insert(fixture.entry.fixture.as_str().into(), fixture.input.clone());
    }

    let mut manifest = ValidationManifest {
        fixtures: planned.into_iter().map(|f| f.entry).collect(),
        // An implemented rule owes a fixture. `UnreachableFromChat` is the
        // one state that cannot pay: no CHAT input reaches the rule, so it
        // owes a named out-of-corpus test instead, and is excluded here rather
        // than being quietly absent from the loader as before.
        implemented_specs_without_examples: validation_specs
            .iter()
            .filter(|spec| spec.metadata.status == Status::Implemented && spec.examples.is_empty())
            .map(|spec| RepoRelativePath::new(repo_root, &spec.source_path_display()))
            .collect(),
        // The converse, so the new state cannot become a way to opt a
        // perfectly reachable rule out of its fixture: if an example exists,
        // the rule is reachable and the status is wrong.
        unreachable_specs_with_examples: validation_specs
            .iter()
            .filter(|spec| {
                spec.metadata.status == Status::UnreachableFromChat && !spec.examples.is_empty()
            })
            .map(|spec| RepoRelativePath::new(repo_root, &spec.source_path_display()))
            .collect(),
    };
    manifest
        .fixtures
        .sort_by(|a, b| a.fixture.as_str().cmp(b.fixture.as_str()));
    manifest.implemented_specs_without_examples.sort();
    manifest.unreachable_specs_with_examples.sort();

    files.insert(
        "manifest.json".into(),
        serde_json::to_string_pretty(&manifest)? + "\n",
    );
    Ok(files)
}

/// Plan one fixture per example across all validation specs, assigning each a
/// filename unique within the corpus dir (multi-example specs would otherwise
/// collide on the shared spec title; the per-example code usually disambiguates,
/// and a numeric suffix covers the rest).
fn plan_fixtures(specs: &[ErrorCorpusSpec], repo_root: &std::path::Path) -> Vec<PlannedFixture> {
    let mut used: HashSet<String> = HashSet::new();
    let mut planned = Vec::new();
    for spec in specs {
        // Computed once per spec; every example of the spec shares them.
        let source_spec = RepoRelativePath::new(repo_root, &spec.source_path_display());
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
    use std::fs;
    use std::path::Path;

    use super::*;

    fn write_spec(dir: &Path, name: &str, body: &str) {
        use std::io::Write;
        let mut file = fs::File::create(dir.join(name)).expect("create spec file");
        file.write_all(body.as_bytes()).expect("write spec body");
    }

    #[test]
    fn sanitize_collapses_runs_of_separators() {
        assert_eq!(
            sanitize_filename("Illegal 'xx' marker"),
            "Illegal_xx_marker"
        );
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
             - **Category**: demo\n- **Level**: utterance\n- **Layer**: validation\n\
             - **Status**: implemented\n\n\
             ## Example 1\n\n**Expected Error Codes**: E316\n\n```chat\n@UTF8\n@Begin\none\n@End\n```\n\n\
             ## Example 2\n\n**Expected Error Codes**: E600\n\n```chat\n@UTF8\n@Begin\ntwo\n@End\n```\n",
        );
        let specs = ErrorCorpusSpec::load_all(dir.path()).expect("load specs");
        let planned = plan_fixtures(&specs, dir.path());

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
        assert!(
            planned
                .iter()
                .all(|f| f.entry.status == Status::Implemented)
        );
        assert!(
            planned[0]
                .entry
                .source_spec
                .as_str()
                .ends_with("E999_multi.md")
        );
    }

    /// The manifest is one of the built files, not a side effect of writing.
    ///
    /// SURVIVES a type: this is a wire format. Nothing in the signature says
    /// the map contains `manifest.json`, and the runner reads it by that name.
    #[test]
    fn the_manifest_is_one_of_the_built_files() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("spec/errors");
        fs::create_dir_all(&dir).expect("spec/errors");
        write_spec(
            &dir,
            "E999_one.md",
            "# E999: One\n\n## Description\n\nDemo.\n\n## Metadata\n\n\
             - **Category**: demo\n- **Level**: utterance\n- **Layer**: validation\n\
             - **Status**: implemented\n\n\
             ## Example 1\n\n**Expected Error Codes**: E999\n\n```chat\n@UTF8\n@Begin\none\n@End\n```\n",
        );
        let files = build(root.path()).expect("build");
        assert!(files.contains_key(Path::new("manifest.json")));
        assert_eq!(files.len(), 2, "one fixture plus the manifest");
    }
}
