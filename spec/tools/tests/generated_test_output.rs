//! What the Rust-test artifact is allowed to leave in its output directory.
//!
//! This is the generator's real boundary: a directory of files that ANOTHER
//! crate compiles. Until 2026-08-04 the generator wrote each suite TWICE, once
//! standalone with a `use` preamble and once as bodies only, and the test tree
//! `include!`d only the bodies. The standalone pair was 175 KB of tracked
//! source holding 213 `#[test]` functions that nothing ever compiled, and every
//! count of "how many tests does chatter have" included them.
//!
//! So the contract these tests pin is deliberately cross-crate: the set of
//! files this artifact PRODUCES must equal the set the consumer `include!`s.
//! Both sides are read from their real sources rather than restated here,
//! because a hardcoded list of the expected filenames would be a third
//! hand-maintained copy of exactly the thing that drifted in the first place.
//!
//! Rewritten 2026-08-15 to drive [`generators::artifacts`] rather than a
//! writer function that only these tests still called. Testing a path
//! production does not take is how a function stays alive after its callers
//! are gone.

use std::collections::BTreeSet;

use generators::artifacts::{Artifact, artifact_for_root};
use generators::output::rust_test::{GeneratedTestFile, RETIRED_OUTPUT_NAMES};
use generators::repo_paths::{self, RepoRoot};

/// The committed root of the artifact under test.
const RUST_TESTS_ROOT: &str = "crates/talkbank-parser-tests/tests/integration/generated";

/// The repository root, resolved by the workspace's one resolver.
fn repo_root() -> RepoRoot {
    RepoRoot::resolve(None).expect(repo_paths::NOT_A_CHECKOUT)
}

/// The artifact this file is about.
fn rust_tests() -> &'static Artifact {
    artifact_for_root(RUST_TESTS_ROOT).expect("the Rust test bodies artifact is in the registry")
}

/// The names the artifact produces, built from the REAL specs.
fn produced_names() -> anyhow::Result<BTreeSet<String>> {
    let files = (rust_tests().build)(repo_root().as_path())?;
    anyhow::ensure!(
        !files.is_empty(),
        "the specs must actually load, or these tests prove nothing"
    );
    Ok(files
        .keys()
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}

/// The file names the consuming crate actually `include!`s, read from its source.
///
/// Parsed rather than restated: this is the OTHER half of the contract, and
/// copying it here would recreate the drift these tests exist to catch.
fn included_by_the_test_tree() -> anyhow::Result<BTreeSet<String>> {
    let consumer =
        repo_root().join("crates/talkbank-parser-tests/tests/integration/generated_tests.rs");
    let source = std::fs::read_to_string(&consumer)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", consumer.display()))?;
    let names: BTreeSet<String> = source
        .split("include!(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .filter_map(|path| std::path::Path::new(path).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    anyhow::ensure!(
        !names.is_empty(),
        "found no include! in {}, so this test would pass vacuously",
        consumer.display()
    );
    Ok(names)
}

/// The artifact produces exactly what the test tree compiles: no more, no less.
///
/// SURVIVES a type: behaviour a signature cannot describe. No Rust type can
/// relate a generator in one workspace to an `include!` in another.
#[test]
fn every_produced_file_is_one_the_test_tree_includes() -> anyhow::Result<()> {
    assert_eq!(
        produced_names()?,
        included_by_the_test_tree()?,
        "the artifact's output set and generated_tests.rs's include! set have diverged"
    );
    Ok(())
}

/// The enum that owns the outputs agrees with what the artifact produces.
///
/// SURVIVES a type: `ALL` is a declaration; that the builder honours it is a
/// fact about the builder.
#[test]
fn the_owning_enum_lists_exactly_what_gets_produced() -> anyhow::Result<()> {
    let declared: BTreeSet<String> = GeneratedTestFile::ALL
        .iter()
        .map(|file| file.file_name().to_string())
        .collect();
    assert_eq!(produced_names()?, declared);
    Ok(())
}

/// Writing into a checkout that still holds retired outputs removes them,
/// without touching the other producer writing into the same directory.
///
/// SURVIVES a type: the retired names have no renderer by design, so nothing in
/// the write path implies they are swept; only running the writer shows it. It
/// also reaches the filesystem, which no type of ours does.
///
/// The temp directory is laid out as a repository root, with `spec` symlinked
/// to the real one, so this drives the SAME `Artifact::write` production takes
/// rather than a test-only entry point.
#[test]
fn writing_sweeps_retired_files_and_spares_the_other_producer() -> anyhow::Result<()> {
    let fake_root = tempfile::tempdir()?;
    std::os::unix::fs::symlink(repo_root().join("spec"), fake_root.path().join("spec"))?;

    let artifact = rust_tests();
    let out = fake_root.path().join(artifact.root);
    std::fs::create_dir_all(&out)?;
    for stale in RETIRED_OUTPUT_NAMES {
        std::fs::write(out.join(stale), "// stale output from an older checkout")?;
    }
    // The other producer's file, which this artifact must not touch.
    let foreign = out.join("reference_corpus.rs");
    std::fs::write(&foreign, "// written by bootstrap_reference_corpus")?;

    artifact.write(fake_root.path())?;

    for stale in RETIRED_OUTPUT_NAMES {
        assert!(
            !out.join(stale).exists(),
            "{stale} survived a regeneration; a checkout predating the change \
             would keep it forever"
        );
    }
    assert_eq!(
        std::fs::read_to_string(&foreign)?,
        "// written by bootstrap_reference_corpus",
        "claiming this directory wholesale deletes the other producer's output, \
         which broke the build on 2026-07-29"
    );
    for name in produced_names()? {
        assert!(out.join(&name).exists(), "{name} was not written");
    }
    Ok(())
}
