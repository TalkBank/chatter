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
use crate::spec::error::{Demonstration, ErrorSpec};
use crate::spec::metadata::SpecErrorCode;
use crate::spec::metadata::Status;
use talkbank_spec_vocabulary::paths::RepoRelativePath;
use talkbank_spec_vocabulary::validation_manifest::{
    FixtureName, ValidationFixtureEntry, ValidationManifest,
};

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
        unreachable_specs_with_examples: unreachable_but_demonstrated(&specs, repo_root),
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

/// Specs whose `unreachable_from_chat` status their own examples disprove.
///
/// The converse of the escape hatch, so the status cannot become a way to opt a
/// perfectly reachable rule out of its fixture: if an example produces the
/// spec's OWN code, the rule is reachable and the status is wrong.
///
/// [`Demonstration::ByExample`], not "has any example". It read
/// `!matches!(.., NoExamples)` until 2026-09-08, which also caught
/// [`Demonstration::Absent`]: a spec whose examples every one assert OTHER
/// codes. That is the opposite finding. A `subsumed_by` example is the evidence
/// FOR unreachability, showing what CHAT actually produces where the rule would
/// have to fire, and it is the most useful thing such a spec can hold. The old
/// form forbade writing it down, so E314 and E512, both established unreachable
/// by reading the code and by probing seven inputs, could not carry the status
/// their own prose argued for.
///
/// What this does NOT do, stated because a first draft of this comment claimed
/// it did: [`ErrorSpec::demonstration`] reads the example's declared CLAIM, not
/// the parser. An example that in fact produces the spec's own code while
/// declaring `subsumed_by` is `Absent` here and passes. The check that catches
/// a claim the parser disproves is `status_falsified_by_the_snapshot` in
/// `scripts/lint/error_code_demonstration.py`, which compares every excused
/// status against the observation snapshot; that is the mechanism to point a
/// reader at, and this one only compares two declarations.
///
/// And an `unreachable_from_chat` status has a second effect worth knowing
/// here: `spec_runtime_tools::error_spec_validation` omits such a spec, so its
/// examples stop being run at all. A `subsumed_by` example offered as evidence
/// FOR unreachability is therefore no longer verified by the corpus runner
/// once the status it argues for is granted. The snapshot still records it.
///
/// A separate function from [`build`] so the decision is reachable from a test
/// without a whole repository behind it.
fn unreachable_but_demonstrated(
    specs: &[ErrorSpec],
    repo_root: &std::path::Path,
) -> Vec<RepoRelativePath> {
    specs
        .iter()
        .filter(|spec| {
            spec.status() == Status::UnreachableFromChat
                && matches!(spec.demonstration(), Demonstration::ByExample)
        })
        .map(|spec| RepoRelativePath::new(repo_root, &spec.source_path_display()))
        .collect()
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
                    rules: example.rules,
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

    /// SURVIVES: policy. Which examples contradict an `unreachable_from_chat`
    /// status is a judgement about what an example ASSERTS, not a fact any
    /// signature carries.
    ///
    /// A spec whose every example claims a DIFFERENT code is evidence FOR
    /// unreachability: it shows what CHAT actually produces where the rule
    /// would have to fire. The filter read "has any example" until 2026-09-08
    /// and so forbade writing that down, which kept E314 and E512 from
    /// carrying the status their own prose argued for.
    #[test]
    fn a_subsumed_by_example_does_not_contradict_an_unreachable_status() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_spec(
            dir.path(),
            "E999_subsumed.md",
            "+++\n\
             code = 'E999'\n\
             name = 'Subsumed'\n\n\
             [[example]]\n\
             level = 'utterance'\n\
             claim = { subsumed_by = 'E316' }\n\
             chat = \"@UTF8\\n@Begin\\none\\n@End\"\n\
             +++\n\n## Description\n\nDemo.\n",
        );
        let registry = crate::test_registry::declaring(&[("E999", Status::UnreachableFromChat)]);
        let specs = ErrorSpec::load_all(dir.path(), &registry).expect("load specs");
        assert!(
            unreachable_but_demonstrated(&specs, dir.path()).is_empty(),
            "a spec asserting only OTHER codes is evidence for unreachability, \
             not against it"
        );
    }

    /// The other direction, which is what stops the status becoming an opt-out.
    #[test]
    fn an_example_producing_the_spec_s_own_code_contradicts_an_unreachable_status() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_spec(
            dir.path(),
            "E999_reachable.md",
            "+++\n\
             code = 'E999'\n\
             name = 'Reachable'\n\n\
             [[example]]\n\
             level = 'utterance'\n\
             claim = 'violates'\n\
             chat = \"@UTF8\\n@Begin\\none\\n@End\"\n\
             +++\n\n## Description\n\nDemo.\n",
        );
        let registry = crate::test_registry::declaring(&[("E999", Status::UnreachableFromChat)]);
        let specs = ErrorSpec::load_all(dir.path(), &registry).expect("load specs");
        assert_eq!(
            unreachable_but_demonstrated(&specs, dir.path()).len(),
            1,
            "an example that produces the code proves the rule reachable"
        );
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
