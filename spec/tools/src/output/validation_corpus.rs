//! Build the validation-error fixture corpus and its typed manifest from
//! `spec/errors/`.
//!
//! For EVERY spec this produces one `.cha` fixture per
//! EXAMPLE, plus `manifest.json` recording each fixture's spec code, its
//! CLAIM (`violates` / `legal` / `subsumed_by`, both halves enforced), its
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

use crate::artifacts::GeneratedFiles;
use crate::repo_paths::RepoRelativePath;
use crate::spec::error::{Demonstration, ErrorSpec};
use crate::spec::metadata::SpecErrorCode;
use crate::spec::metadata::Status;
use crate::spec::validation_manifest::{FixtureName, ValidationFixtureEntry, ValidationManifest};

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
    // EVERY spec, since R4. The `layer.is_validation()` filter that stood
    // here kept the runner partial: the runner has always collected BOTH
    // stages' codes against a real file, so it was already the total
    // instrument, and the authored field was the only thing routing 85
    // parse-stage examples away from it. Measured before the change: zero
    // implemented examples fail the runner's union check, and five examples'
    // codes are SPLIT across stages, which no per-stage harness can assert.
    let registry = talkbank_spec_vocabulary::registry::CodeRegistry::load(repo_root)?;
    let specs: Vec<ErrorSpec> =
        ErrorSpec::load_all(crate::artifacts::error_dir(repo_root), &registry)
            .map_err(|e| anyhow::anyhow!("Failed to load error corpus specs: {}", e))?;

    let planned = plan_fixtures(&specs, repo_root);

    let mut files = GeneratedFiles::new();
    for fixture in &planned {
        // `parse_markdown` strips the chat block's trailing newline, so it has
        // to be put back: the grammar REQUIRES a final newline, and without one
        // every fixture parsed with a `MISSING newline` recovery node. That was
        // invisible for as long as every fixture also emitted a real
        // diagnostic, which hid the spurious node behind a genuine one. The
        // first `claim = 'legal'` example written inline surfaced it
        // immediately, by failing `no_recovery_node_in_accepted_file`: a clean
        // file has nothing to hide the node behind.
        //
        // A fixture that needs recovery is not the input its spec claims to
        // describe, so this is not cosmetic; it is the difference between
        // testing the rule and testing the rule plus an unrelated defect.
        files.insert(
            fixture.entry.fixture.as_str().into(),
            format!("{}\n", fixture.input.trim_end_matches('\n')),
        );
    }

    let mut manifest = ValidationManifest {
        fixtures: planned.into_iter().map(|f| f.entry).collect(),
        // An implemented rule owes a fixture, and the obligation is the
        // CODE's: several spec files may claim one code, and a no-example
        // spec whose sibling demonstrates the code is documentation, not a
        // gap. `UnreachableFromChat` is the one state that cannot pay: no
        // CHAT input reaches the rule, so it owes a named out-of-corpus test
        // instead, and is excluded rather than being quietly absent.
        implemented_codes_without_examples: {
            /// What the specs claiming one code jointly establish about it.
            #[derive(Default)]
            struct CodeStanding {
                implemented: bool,
                has_example: bool,
            }
            let mut per_code: std::collections::BTreeMap<&SpecErrorCode, CodeStanding> =
                std::collections::BTreeMap::new();
            for spec in &specs {
                let standing = per_code.entry(&spec.error.code).or_default();
                standing.implemented |= spec.status() == Status::Implemented;
                standing.has_example |= !matches!(spec.demonstration(), Demonstration::NoExamples);
            }
            per_code
                .into_iter()
                .filter(|(_, standing)| standing.implemented && !standing.has_example)
                .map(|(code, _)| code.clone())
                .collect()
        },
        // The converse, so the new state cannot become a way to opt a
        // perfectly reachable rule out of its fixture: if an example exists,
        // the rule is reachable and the status is wrong.
        unreachable_specs_with_examples: specs
            .iter()
            .filter(|spec| {
                spec.status() == Status::UnreachableFromChat
                    && !matches!(spec.demonstration(), Demonstration::NoExamples)
            })
            .map(|spec| RepoRelativePath::new(repo_root, &spec.source_path_display()))
            .collect(),
    };
    manifest
        .fixtures
        .sort_by(|a, b| a.fixture.as_str().cmp(b.fixture.as_str()));
    manifest.implemented_codes_without_examples.sort();
    manifest.unreachable_specs_with_examples.sort();

    files.insert(
        "manifest.json".into(),
        serde_json::to_string_pretty(&manifest)? + "\n",
    );
    Ok(files)
}

/// Plan one fixture per example, named by the example's identity (see
/// [`fixture_name`]).
fn plan_fixtures(specs: &[ErrorSpec], repo_root: &std::path::Path) -> Vec<PlannedFixture> {
    let mut planned = Vec::new();
    for spec in specs {
        // Computed once per spec; every example of the spec shares them.
        let source_spec = RepoRelativePath::new(repo_root, &spec.source_path_display());
        let status = spec.status();
        for (index, example) in spec.error.examples.iter().enumerate() {
            planned.push(PlannedFixture {
                input: example.input.clone(),
                entry: ValidationFixtureEntry {
                    fixture: FixtureName::new(fixture_name(spec, index)),
                    code: spec.error.code.clone(),
                    claim: example.claim.clone(),
                    status,
                    source_spec: source_spec.clone(),
                },
            });
        }
    }
    planned
}

/// The fixture's name IS the example's identity; [`ExampleId`] owns the rule.
///
/// The history of why (the collision-counter scheme that renamed other specs'
/// fixtures when the corpus grew) is on `ExampleId` itself, where the next
/// generator will actually read it.
fn fixture_name(spec: &ErrorSpec, index: usize) -> String {
    talkbank_spec_vocabulary::observations::ExampleId::from_enumerate(spec.source_file(), index)
        .fixture_name()
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
    fn plans_one_fixture_per_example_with_its_own_codes() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A two-example spec whose examples declare different codes.
        write_spec(
            dir.path(),
            "E999_multi.md",
            "+++\n\
             code = 'E999'\n\
             name = 'Multi'\n\n\
             [[example]]\n\
             level = 'utterance'\n\
             claim = { subsumed_by = 'E316' }\n\
             chat = \"@UTF8\\n@Begin\\none\\n@End\"\n\n\
             [[example]]\n\
             level = 'utterance'\n\
             claim = { subsumed_by = 'E600' }\n\
             chat = \"@UTF8\\n@Begin\\ntwo\\n@End\"\n\
             +++\n\n## Description\n\nDemo.\n",
        );
        // A registry declaring the fixture's own code. The claim TARGETS
        // (E316, E600) are not resolved: a claim names codes, it does not
        // document them.
        let registry = crate::test_registry::declaring(&[("E999", Status::Implemented)]);
        let specs = ErrorSpec::load_all(dir.path(), &registry).expect("load specs");
        let planned = plan_fixtures(&specs, dir.path());

        assert_eq!(planned.len(), 2, "one fixture per example");
        use talkbank_spec_vocabulary::frontmatter::Claim;
        let targets: Vec<&str> = planned
            .iter()
            .flat_map(|f| match &f.entry.claim {
                Claim::SubsumedBy(t) => t.as_slice(),
                Claim::Violates | Claim::Legal => &[],
            })
            .map(|c| c.as_str())
            .collect();
        assert!(targets.contains(&"E316") && targets.contains(&"E600"));
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
        // A temp checkout is a checkout: it needs the registry, because a spec
        // cannot be loaded without resolving the code it names. Written at
        // `REGISTRY_PATH`, not at a hand-spelled copy of it.
        crate::test_registry::write_into(root.path(), &[("E999", Status::Implemented)]);
        write_spec(
            &dir,
            "E999_one.md",
            "+++\n\
             code = 'E999'\n\
             name = 'One'\n\n\
             [[example]]\n\
             level = 'utterance'\n\
             claim = 'violates'\n\
             chat = \"@UTF8\\n@Begin\\none\\n@End\"\n\
             +++\n\n## Description\n\nDemo.\n",
        );
        let files = build(root.path()).expect("build");
        assert!(files.contains_key(Path::new("manifest.json")));
        assert_eq!(files.len(), 2, "one fixture plus the manifest");
    }
}
