//! Every committed artifact that is GENERATED from `spec/`, and the one
//! description that produces, locates and checks each one.
//!
//! # Why this exists
//!
//! Four separate statements used to answer the same question, and any two of
//! them could disagree:
//!
//! 1. what a generator binary writes, and WHERE (a `--output-dir` argument the
//!    caller supplied);
//! 2. what the book's Regenerating section tells a contributor to type;
//! 3. what a generated file's own header tells you to run, which said
//!    `make test-gen` for a repository that has no `Makefile`;
//! 4. what checks that the committed copy is current, which for four of the six
//!    artifacts was NOTHING. `spec/tools/tests/generated_test_output.rs` pins
//!    the SET of files a generator writes against the set the test tree
//!    `include!`s, and never compares their content to anything.
//!
//! An [`Artifact`] states all of it once. The destination is a constant here
//! rather than an argument, so a generator cannot be pointed at the wrong
//! directory; [`build`](Artifact::build) returns the files rather than writing
//! them, so the currency gate can answer a read-only question without writing
//! anything; and one binary (`spec_gen`) both writes and checks from this list.
//!
//! The first run of the check found the tree-sitter corpus already stale: it
//! still held `errors/e311.txt` where the specs now produce `e311_1.txt` and
//! `e311_2.txt`. That drift had been invisible since 2026-07-30, and reasoning
//! from commit dates had said it was fine, which is why a gate exists and an
//! inference does not.
//!
//! # Adding an artifact
//!
//! Add a row. That is the whole change: the writer, the checker, the `just`
//! recipes and the gate all iterate this slice. If a new artifact needs the
//! live parser or model, it belongs in `spec-runtime-tools`'s own registry
//! instead, for the reason that crate exists.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::output::{markdown, rust_test, tree_sitter, validation_corpus};
use crate::owned_output::clear_owned;
use crate::spec::by_code::SpecsByCode;
use crate::spec::{ConstructSpec, ErrorSpec};

/// The complete contents of one artifact: relative path to file body.
///
/// Ordered, because the map is compared, written and printed, and an unstable
/// order would make a diff unreadable and a failure message unreproducible.
pub type GeneratedFiles = BTreeMap<PathBuf, String>;

/// The fully-qualified `TestError` path the generated Rust tests refer to.
///
/// A constant rather than a CLI argument: a different value produces tests that
/// do not compile, so it is not a choice a caller should be offered.
const TEST_ERROR_PATH: &str = "talkbank_parser_tests::test_error::TestError";

/// How much of its destination directory an artifact owns.
///
/// The distinction is real and was learned the expensive way: claiming
/// `tests/integration/generated/` wholesale deletes `reference_corpus.rs`,
/// which a DIFFERENT producer writes, and that broke the build immediately when
/// it was tried on 2026-07-29.
#[derive(Debug, Clone, Copy)]
pub enum Ownership {
    /// The generator owns the whole directory and clears it wholesale before
    /// writing, which is how a file disappears when its spec is deleted.
    ///
    /// [`clear_owned`] refuses a directory that does not carry the
    /// `.generated-output-dir` marker, so this cannot be pointed at a
    /// hand-authored tree.
    WholeDirectory,
    /// The generator owns only the files it produces, plus `retired`, inside a
    /// directory shared with other producers.
    ///
    /// `retired` names files this generator no longer writes but must still
    /// sweep, so a checkout predating the change does not keep them forever.
    NamedFiles {
        /// Names to delete on write, though nothing produces them any more.
        retired: &'static [&'static str],
    },
}

impl Ownership {
    /// Committed files the specs do not produce, if this ownership kind is in a
    /// position to say.
    ///
    /// An exhaustive match rather than an `if`, so a THIRD ownership kind has
    /// to decide this at compile time instead of silently inheriting "compute
    /// nothing". It was a guard whose false branch was invisible: for a
    /// `NamedFiles` artifact, `Difference::Extra` simply could never be
    /// produced, and the only statement of that was a doc comment.
    fn extras(&self, root: &Path, expected: &GeneratedFiles) -> Result<Vec<Difference>> {
        match self {
            // The directory is this artifact's alone, so anything in it that
            // the specs no longer produce is a leftover.
            Self::WholeDirectory => Ok(committed_files(root)?
                .into_iter()
                .filter(|committed| !expected.contains_key(committed))
                .map(Difference::Extra)
                .collect()),
            // Another producer's output shares this directory, and this
            // artifact has no standing to call it extra. Explicitly nothing,
            // not accidentally nothing.
            Self::NamedFiles { .. } => Ok(Vec::new()),
        }
    }
}

/// One generated artifact: what it is, where it is committed, and how to build
/// it.
pub struct Artifact {
    /// Short name, used in progress output and failure messages.
    pub what: &'static str,
    /// Destination, relative to the repository root. Never a caller's choice.
    pub root: &'static str,
    /// How much of `root` this artifact owns.
    pub ownership: Ownership,
    /// Produce the complete contents, given the repository root. Writes nothing.
    pub build: fn(&Path) -> Result<GeneratedFiles>,
}

/// Every artifact generated from `spec/` that needs no parser or model crate.
///
/// Artifacts needing the live `ErrorCode` enum live in `spec-runtime-tools`,
/// which is the whole reason that crate is separate from this one.
pub static ARTIFACTS: &[Artifact] = &[
    Artifact {
        what: "tree-sitter corpus tests",
        root: "grammar/test/corpus/generated",
        ownership: Ownership::WholeDirectory,
        build: build_tree_sitter_corpus,
    },
    Artifact {
        what: "generated Rust test bodies",
        root: "crates/talkbank-parser-tests/tests/integration/generated",
        ownership: Ownership::NamedFiles {
            retired: rust_test::RETIRED_OUTPUT_NAMES,
        },
        build: build_rust_tests,
    },
    Artifact {
        what: "published error documentation",
        root: "docs/errors",
        ownership: Ownership::WholeDirectory,
        build: build_error_docs,
    },
    Artifact {
        what: "validation fixture corpus + manifest",
        root: "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
        ownership: Ownership::WholeDirectory,
        build: build_validation_corpus,
    },
];

/// Look up an artifact by its committed root.
///
/// The root string is an artifact's identity: it is what the registry declares,
/// what the writer writes to, and what a failure message names. `None` means
/// the caller named a root no artifact claims, which is a programming error at
/// the call site rather than a runtime condition.
pub fn artifact_for_root(root: &str) -> Option<&'static Artifact> {
    ARTIFACTS.iter().find(|artifact| artifact.root == root)
}

/// `spec/constructs`, under `repo_root`.
fn construct_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("spec/constructs")
}

/// `spec/errors`, under `repo_root`.
///
/// `pub(crate)` so the sibling builders reach for it instead of re-typing the
/// literal, which is how two spellings of one path start drifting.
pub(crate) fn error_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("spec/errors")
}

/// The published per-code documentation and its index, from the error specs.
///
/// # Why this is in the registry at all
///
/// It was TRACKED, GENERATED and UNGATED, which is the one combination that
/// silently rots: nothing regenerated it and nothing compared it. When it was
/// added on 2026-08-15 the committed copy was already stale, and not
/// cosmetically. `docs/errors/E311.md` told readers "Planned; documented but
/// not yet enforced by the validator" for a check that is Active, and omitted
/// an example the spec had gained. A published document telling users a check
/// does not fire, when it does, is worse than no document.
fn build_error_docs(repo_root: &Path) -> Result<GeneratedFiles> {
    let specs = ErrorSpec::load_all(error_dir(repo_root))
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {e}"))?;

    // ONE grouping, shared by the pages and the index, and it DISCARDS NOTHING.
    //
    // The page loop used to get last-wins for free from `GeneratedFiles::insert`
    // keyed on `{code}.md`, and the index re-stated the same rule in its own
    // `BTreeMap`, with a doc comment as the only thing holding the two
    // together. Both rules threw specs away, because a map keyed on the code
    // has nowhere to put the second spec that claims it. See
    // `crate::spec::by_code` for why several specs under one code is a
    // legitimate state rather than a conflict to adjudicate.
    let by_code = SpecsByCode::group(specs);

    let mut files = GeneratedFiles::new();
    files.insert("index.md".into(), markdown::generate_error_index(&by_code));
    for (code, specs) in by_code.codes() {
        files.insert(
            format!("{code}.md").into(),
            markdown::generate_error_page(code, specs),
        );
    }
    Ok(files)
}

/// Tree-sitter corpus tests, from construct specs plus parser-layer error specs.
fn build_tree_sitter_corpus(repo_root: &Path) -> Result<GeneratedFiles> {
    let specs = ConstructSpec::load_all(construct_dir(repo_root))
        .map_err(|e| anyhow::anyhow!("Failed to load construct specs: {e}"))?;
    let error_specs = ErrorSpec::load_all(error_dir(repo_root))
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {e}"))?;

    // Membership is decided per EXAMPLE from the observation snapshot, an
    // artifact this generator READS (the registry regenerates it first; see
    // `spec_runtime_tools::artifacts::all`). The file is data, so this crate's
    // no-live-parser boundary holds, the same way the main workspace consumes
    // manifest.json. An absent snapshot is refused: generating an empty error
    // corpus from a missing input would read as "no parser-stage examples".
    let snapshot_path = repo_root
        .join("spec/observations")
        .join(talkbank_spec_vocabulary::observations::SNAPSHOT_FILE);
    let snapshot_text = std::fs::read_to_string(&snapshot_path).map_err(|err| {
        anyhow::anyhow!(
            "cannot read the observation snapshot at {}: {err}. It is an input \
             to corpus membership; `just spec-gen` generates it first",
            snapshot_path.display()
        )
    })?;
    let snapshot: talkbank_spec_vocabulary::observations::ObservationSnapshot =
        serde_json::from_str(&snapshot_text)
            .map_err(|err| anyhow::anyhow!("parsing {}: {err}", snapshot_path.display()))?;
    let all_errors: Vec<&ErrorSpec> = error_specs.iter().collect();

    let templates = repo_root.join("spec/tools/templates");
    let mut files = GeneratedFiles::new();
    for (name, content) in
        tree_sitter::generate_corpus_files_with_templates(&specs, Some(&templates))
            .map_err(|e| anyhow::anyhow!("{e}"))?
    {
        files.insert(name.into(), content);
    }
    for (name, content) in tree_sitter::generate_error_corpus_files(&all_errors, &snapshot)
        .map_err(|e| anyhow::anyhow!("{e}"))?
    {
        files.insert(name.into(), content);
    }
    Ok(files)
}

/// The two `_body.rs` files the parser test tree `include!`s.
fn build_rust_tests(repo_root: &Path) -> Result<GeneratedFiles> {
    let construct_specs = ConstructSpec::load_all(construct_dir(repo_root))
        .map_err(|e| anyhow::anyhow!("Failed to load construct specs: {e}"))?;

    let mut files = GeneratedFiles::new();
    for file in rust_test::GeneratedTestFile::ALL {
        files.insert(
            file.file_name().into(),
            file.render(&construct_specs, TEST_ERROR_PATH),
        );
    }
    Ok(files)
}

/// The validation fixtures and their manifest.
fn build_validation_corpus(repo_root: &Path) -> Result<GeneratedFiles> {
    validation_corpus::build(repo_root)
}

/// How a committed copy differs from what the specs say it should be.
///
/// A typed difference rather than a formatted string, so a caller can count
/// them, group them, or decide that one kind matters more than another. The
/// gate's message is built from these; it does not receive them pre-rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Difference {
    /// The specs produce this file and the committed tree does not have it.
    Missing(PathBuf),
    /// The committed tree has this file and the specs do not produce it.
    Extra(PathBuf),
    /// Both have it, with different bytes.
    Differs(PathBuf),
}

impl Difference {
    /// The file this difference is about.
    ///
    /// Sorting by this groups a file's problems together. The first version
    /// sorted by the rendered `Display` string, which allocated two `String`s
    /// per comparison and ordered by the fixed prefix, so every `extra:` came
    /// before every `missing:` and a reader looking for one file had to scan.
    fn path(&self) -> &Path {
        match self {
            Self::Missing(path) | Self::Extra(path) | Self::Differs(path) => path,
        }
    }
}

impl std::fmt::Display for Difference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(path) => write!(f, "missing:  {}", path.display()),
            Self::Extra(path) => write!(f, "extra:    {}", path.display()),
            Self::Differs(path) => write!(f, "stale:    {}", path.display()),
        }
    }
}

impl Artifact {
    /// The committed location, under `repo_root`.
    pub fn path(&self, repo_root: &Path) -> PathBuf {
        repo_root.join(self.root)
    }

    /// Compare the committed copy against what the specs produce.
    ///
    /// Writes nothing. An empty result means the committed copy is current.
    ///
    /// Whether a committed file the specs do NOT produce counts as a problem
    /// is [`Ownership`]'s decision, not this method's; see
    /// [`Ownership::extras`].
    pub fn check(&self, repo_root: &Path) -> Result<Vec<Difference>> {
        let expected = (self.build)(repo_root)
            .with_context(|| format!("building {} for comparison", self.what))?;
        let root = self.path(repo_root);

        let mut differences = Vec::new();
        for (relative, want) in &expected {
            let full = root.join(relative);
            match std::fs::read_to_string(&full) {
                Ok(got) if &got == want => {}
                Ok(_) => differences.push(Difference::Differs(relative.clone())),
                Err(_) => differences.push(Difference::Missing(relative.clone())),
            }
        }

        differences.extend(self.ownership.extras(&root, &expected)?);
        differences.sort_by(|a, b| a.path().cmp(b.path()));
        Ok(differences)
    }

    /// Rebuild the committed copy from the specs, and report what was written.
    pub fn write(&self, repo_root: &Path) -> Result<usize> {
        let files = (self.build)(repo_root).with_context(|| format!("building {}", self.what))?;
        let root = self.path(repo_root);

        match self.ownership {
            // Clearing wholesale is how a file disappears when its spec is
            // deleted. `clear_owned` refuses a directory without the marker.
            Ownership::WholeDirectory => clear_owned(&root)?,
            Ownership::NamedFiles { retired } => {
                // No `create_dir_all` here: removal does not need the directory
                // to exist, and the write loop below creates every parent.
                for name in files
                    .keys()
                    .filter_map(|p| p.to_str())
                    .chain(retired.iter().copied())
                {
                    let path = root.join(name);
                    if path.exists() {
                        std::fs::remove_file(&path)?;
                    }
                }
            }
        }

        for (relative, content) in &files {
            let full = root.join(relative);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&full, content)
                .with_context(|| format!("writing {}", full.display()))?;
        }
        Ok(files.len())
    }
}

/// Every committed file under `root`, relative to it, excluding the ownership
/// marker (which is infrastructure, not content).
fn committed_files(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.with_context(|| format!("walking {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() == crate::owned_output::OWNERSHIP_MARKER {
            continue;
        }
        // Propagated, never defaulted. `walkdir` only yields paths under
        // `root`, so this cannot fail today; the point is what happens if it
        // ever does. Falling back to the absolute path would push it into a
        // list whose every other member is RELATIVE, and that list is compared
        // against the specs' output by key, so the file would be reported as
        // `Extra` and DELETED on the next write. A wrong answer that
        // type-checks is exactly the shape this registry exists to remove.
        let relative = entry.path().strip_prefix(root).with_context(|| {
            format!(
                "{} is not under {}, which walkdir should make impossible",
                entry.path().display(),
                root.display()
            )
        })?;
        out.push(relative.to_path_buf());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two artifacts writing the same directory would each report the other's
    /// files as `Extra`, and each would delete the other's on write.
    #[test]
    fn no_two_artifacts_claim_the_same_root() {
        let mut seen = std::collections::BTreeSet::new();
        for artifact in ARTIFACTS {
            assert!(
                seen.insert(artifact.root),
                "two artifacts claim {}",
                artifact.root
            );
        }
    }

    /// A `WholeDirectory` artifact deletes everything in its root, so pointing
    /// one at a directory with another producer in it destroys that producer's
    /// output. `tests/integration/generated/` is exactly that case and is
    /// deliberately `NamedFiles`.
    ///
    /// SURVIVES a type: the hazard is a relationship between a constant in this
    /// file and what some other program writes, which no signature here can see.
    #[test]
    fn the_shared_generated_directory_is_not_claimed_wholesale() {
        let shared = ARTIFACTS
            .iter()
            .find(|a| a.root.ends_with("tests/integration/generated"))
            .expect("the Rust test bodies artifact");
        assert!(
            matches!(shared.ownership, Ownership::NamedFiles { .. }),
            "bootstrap_reference_corpus also writes into {}; claiming it \
             wholesale deleted reference_corpus.rs and broke the build on \
             2026-07-29",
            shared.root
        );
    }
}
