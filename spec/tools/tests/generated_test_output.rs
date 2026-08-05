//! What `gen_rust_tests` is allowed to leave in the output directory.
//!
//! This is the generator's real boundary: a directory of files that ANOTHER
//! crate compiles. Until 2026-08-04 the generator wrote each suite TWICE, once
//! standalone with a `use` preamble and once as bodies only, and the test tree
//! `include!`d only the bodies. The standalone pair was 175 KB of tracked
//! source holding 213 `#[test]` functions that nothing ever compiled, and every
//! count of "how many tests does chatter have" included them.
//!
//! So the contract these tests pin is deliberately cross-crate: the set of
//! files this generator WRITES must equal the set the consumer `include!`s.
//! Both sides are read from their real sources rather than restated here,
//! because a hardcoded list of the expected filenames would be a third
//! hand-maintained copy of exactly the thing that drifted in the first place.

use std::collections::BTreeSet;

use generators::output::rust_test::{self, GeneratedTestFile, RETIRED_OUTPUT_NAMES};
use generators::spec::{ConstructSpec, ErrorSpec};

/// The repository root, two levels up from this crate's manifest.
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The real specs, as the generator would load them.
fn load_real_specs() -> anyhow::Result<(Vec<ConstructSpec>, Vec<ErrorSpec>)> {
    let root = repo_root();
    let constructs = ConstructSpec::load_all(root.join("spec/constructs"))
        .map_err(|e| anyhow::anyhow!("load construct specs: {e}"))?;
    let errors = ErrorSpec::load_all(root.join("spec/errors"))
        .map_err(|e| anyhow::anyhow!("load error specs: {e}"))?;
    anyhow::ensure!(
        !constructs.is_empty() && !errors.is_empty(),
        "the specs must actually load, or these tests prove nothing"
    );
    Ok((constructs, errors))
}

/// Run the generator into a fresh directory and report what it left behind.
fn generate_into(directory: &std::path::Path) -> anyhow::Result<BTreeSet<String>> {
    let (constructs, errors) = load_real_specs()?;
    rust_test::write_generated_tests(
        directory,
        &constructs,
        &errors,
        "talkbank_parser_tests::test_error::TestError",
    )?;
    Ok(std::fs::read_dir(directory)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<BTreeSet<String>>>()?)
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

/// The generator writes exactly what the test tree compiles: no more, no less.
///
/// Surviving category: behaviour a signature cannot describe. No Rust type can
/// relate a generator in one workspace to an `include!` in another.
#[test]
fn every_written_file_is_one_the_test_tree_includes() -> anyhow::Result<()> {
    let output = tempfile::tempdir()?;
    let written = generate_into(output.path())?;
    assert_eq!(
        written,
        included_by_the_test_tree()?,
        "the generator's output set and generated_tests.rs's include! set have diverged"
    );
    Ok(())
}

/// The enum that owns the outputs agrees with what running the generator produces.
///
/// Surviving category: behaviour a signature cannot describe. `ALL` is a
/// declaration; that the writer honours it is a fact about the writer.
#[test]
fn the_owning_enum_lists_exactly_what_gets_written() -> anyhow::Result<()> {
    let output = tempfile::tempdir()?;
    let declared: BTreeSet<String> = GeneratedTestFile::ALL
        .iter()
        .map(|file| file.file_name().to_string())
        .collect();
    assert_eq!(generate_into(output.path())?, declared);
    Ok(())
}

/// Regenerating in a checkout that still holds retired outputs removes them,
/// without touching the other producer writing into the same directory.
///
/// Surviving category: behaviour a signature cannot describe. The retired names
/// have no renderer by design, so nothing in the write path implies they are
/// swept; only running the cleaner shows it.
#[test]
fn regenerating_sweeps_retired_files_and_spares_the_other_producer() -> anyhow::Result<()> {
    let output = tempfile::tempdir()?;
    for stale in RETIRED_OUTPUT_NAMES {
        std::fs::write(
            output.path().join(stale),
            "// left over from an old checkout\n",
        )?;
    }
    // `bootstrap_reference_corpus` writes into this same directory. Clearing it
    // wholesale deletes that file, which broke the build on 2026-07-29.
    let other_producer = "reference_corpus.rs";
    std::fs::write(output.path().join(other_producer), "// another producer\n")?;

    let written = generate_into(output.path())?;

    for stale in RETIRED_OUTPUT_NAMES {
        assert!(
            !written.contains(*stale),
            "retired file {stale} must be swept, got {written:?}"
        );
    }
    assert!(
        written.contains(other_producer),
        "another producer's output must survive, got {written:?}"
    );
    Ok(())
}
