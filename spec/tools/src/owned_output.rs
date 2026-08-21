//! Preparing a generated-output directory, safely, for every generator.
//!
//! # Why this exists
//!
//! Generators clear stale output so that deleting a spec deletes its generated
//! artifacts. The hazard is that "clear stale output" and "delete files I did
//! not write" are the same operation unless something distinguishes them, and
//! generated files are usually indistinguishable by content from
//! hand-maintained ones sitting beside them.
//!
//! The tree-sitter corpus generator wiped the SHARED `grammar/test/corpus/`
//! root, where
//! `word_markers/marker_density.txt` (1,468 lines mined from wild corpus data)
//! lived alongside generated tests. It destroyed that file twice in three days;
//! both times the loss was caught only because a human read the diff.
//!
//! Before this module the four generators each had their own answer to that
//! problem, and no two agreed. The answer is now a single typed field:
//! [`crate::artifacts::Ownership`], which every artifact declares and the
//! writer honours, so "may I clear this directory" is asked once rather than
//! per generator.
//!
//! # The rule
//!
//! A generator OWNS its output directory outright and may clear it wholesale,
//! which is safe precisely because nothing else is in it. Ownership is claimed
//! by a marker file, and [`clear_owned`] REFUSES to clear a directory that does
//! not carry one. Pointing a generator at a shared or hand-maintained tree then
//! fails loudly instead of deleting someone's work.
//!
//! The marker states the rule in situ, so a reader who finds the directory does
//! not have to find this module.
//!
//! # The dual, and why it is the same module
//!
//! [`clear_owned`] protects a HUMAN's files from a generator that was pointed at
//! the wrong directory. [`WritableDir`] protects the same files from a generator
//! that was pointed at the RIGHT directory and should not have been writing at
//! all.
//!
//! `spec/errors/` and `spec/constructs/` are the source of truth for what CHAT
//! is. Three tools have written into them, deciding what to write by running the
//! parser and recording what it did, which makes the specification derived from
//! the implementation and every gate green by construction. That is the reverse
//! arrow the spec-system redesign exists to remove, and R5 removes it
//! mechanically rather than by asking people not to do it.
//!
//! Two markers, opposite meanings, one question each:
//!
//! | marker | meaning | who is protected |
//! |---|---|---|
//! | [`OWNERSHIP_MARKER`] | a generator owns this outright | the human, from a mis-aimed generator |
//! | [`HUMAN_AUTHORED_MARKER`] | a human owns this outright | the source of truth, from any generator |
//!
//! A directory carrying both would be claiming two owners; nothing creates that
//! state, and `WritableDir::claim` refuses it rather than picking one.

use std::path::Path;

use anyhow::{Context, Result, bail};

/// Marker naming a directory as some generator's exclusive territory.
pub const OWNERSHIP_MARKER: &str = ".generated-output-dir";

/// Contents of the marker file.
const MARKER_BODY: &str = "\
This directory is GENERATED and is deleted in full on every generator run.
Do not put anything here by hand: it will be silently lost.

Hand-maintained files belong in a sibling directory that no generator writes to.
";

/// Clear `dir` so a generator can rewrite it, refusing any directory it does
/// not own.
///
/// On success the directory exists, is empty apart from the ownership marker,
/// and is ready to be written into. A directory that does not yet exist is
/// created and claimed, which is what makes a first run work.
///
/// # Errors
///
/// Fails when `dir` exists without the [`OWNERSHIP_MARKER`], which means it is
/// shared or hand-maintained and clearing it would destroy work. The message
/// names the directory and explains how to reserve one.
pub fn clear_owned(dir: &Path) -> Result<()> {
    if dir.exists() {
        if !dir.join(OWNERSHIP_MARKER).exists() {
            bail!(
                "refusing to clear {}: no `{}` marker, so this directory may hold work \
                 that is not generated output.\n\
                 \n\
                 Point the generator at a directory reserved for its output, or, if this \
                 one really is exclusively generated, claim it by creating a `{}` file in it.",
                dir.display(),
                OWNERSHIP_MARKER,
                OWNERSHIP_MARKER
            );
        }
        std::fs::remove_dir_all(dir)
            .with_context(|| format!("clearing generated output directory {}", dir.display()))?;
    }
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating generated output directory {}", dir.display()))?;
    std::fs::write(dir.join(OWNERSHIP_MARKER), MARKER_BODY)
        .with_context(|| format!("writing the ownership marker in {}", dir.display()))?;
    Ok(())
}

/// Marker naming a directory as human-authored source that no generator writes.
pub const HUMAN_AUTHORED_MARKER: &str = ".human-authored";

/// Contents of the human-authored marker.
const HUMAN_AUTHORED_BODY: &str = "\
The files in this directory are the SOURCE OF TRUTH and are written by people.

No generator may write here. A tool that decides what a specification says by
running the implementation makes the specification derived from the
implementation, so every gate passes by construction and none of them can tell a
finished rule from a gap. Three tools did exactly that; the marker is what stops
the fourth.

A bootstrap tool that proposes specs writes to `spec/proposals/`, which a person
reads, completes and moves. It can never write the source of truth.
";

/// Permission to write into a directory, which a human-authored one never grants.
///
/// # Why a type and not a `refuse_if_human_authored(dir)` check
///
/// A check is a rule: a writer has to remember to call it, and the writers this
/// exists to stop were three tools that each called `fs::write` on a path
/// straight from `--spec-dir`. Nothing would have reminded a fourth.
///
/// Holding one of these IS the permission, and [`Self::write`] is how a file gets
/// written, so the natural way to write is the checked way. Rust cannot stop a
/// caller passing its own `PathBuf` to `fs::write`, but that is now visibly the
/// odd path rather than the only path.
#[derive(Debug)]
pub struct WritableDir {
    /// The directory, proved to carry no human-authored claim.
    dir: std::path::PathBuf,
}

impl WritableDir {
    /// Claim write access to `dir`, refusing one a human has claimed.
    ///
    /// # Errors
    ///
    /// Fails when `dir` carries the [`HUMAN_AUTHORED_MARKER`], naming the
    /// directory and saying where a proposal belongs instead. Also fails when a
    /// directory claims both owners, which no code creates and which would
    /// otherwise be resolved silently in favour of whichever check ran first.
    pub fn claim(dir: &Path) -> Result<Self> {
        let human = dir.join(HUMAN_AUTHORED_MARKER).exists();
        let generated = dir.join(OWNERSHIP_MARKER).exists();
        match (human, generated) {
            (true, true) => bail!(
                "{} carries both `{}` and `{}`, so it claims two owners. \
                 Remove whichever is wrong; refusing rather than choosing one.",
                dir.display(),
                HUMAN_AUTHORED_MARKER,
                OWNERSHIP_MARKER
            ),
            (true, false) => bail!(
                "refusing to write into {}: it carries `{}`, so its files are the \
                 source of truth and are written by people.\n\
                 \n\
                 A tool that decides what a spec says by running the implementation \
                 makes the spec derived from the implementation, and every gate then \
                 passes by construction. Write proposals to `spec/proposals/` for a \
                 person to complete and move.",
                dir.display(),
                HUMAN_AUTHORED_MARKER
            ),
            (false, _) => Ok(Self {
                dir: dir.to_path_buf(),
            }),
        }
    }

    /// Write one file into the claimed directory.
    ///
    /// # Errors
    ///
    /// Propagates the write failure, naming the path.
    pub fn write(&self, name: &Path, contents: &str) -> Result<()> {
        let path = self.dir.join(name);
        std::fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))
    }
}

/// Claim a human-authored directory, so the claim is committed rather than
/// assumed.
///
/// Used by the test that asserts the real spec directories carry it. Kept beside
/// the marker it writes so the body has one owner.
///
/// # Errors
///
/// Propagates the write failure.
pub fn mark_human_authored(dir: &Path) -> Result<()> {
    std::fs::write(dir.join(HUMAN_AUTHORED_MARKER), HUMAN_AUTHORED_BODY)
        .with_context(|| format!("writing the human-authored marker in {}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two directories that ARE the source of truth, relative to the repo.
    ///
    /// A committed marker can be deleted as easily as it was added, so the gate
    /// is this list, not the files. Adding a source-of-truth directory without
    /// adding it here is the drift this catches.
    const HUMAN_AUTHORED_DIRS: &[&str] = &["spec/errors", "spec/constructs"];

    /// The real spec directories carry the marker.
    ///
    /// A MEASUREMENT of the committed tree, not an invariant a type can hold:
    /// nothing in the type system can require a file to exist on disk.
    #[test]
    fn the_source_of_truth_directories_are_claimed() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for relative in HUMAN_AUTHORED_DIRS {
            let dir = repo.join(relative);
            assert!(
                dir.join(HUMAN_AUTHORED_MARKER).exists(),
                "{relative} must carry `{HUMAN_AUTHORED_MARKER}`: it is the source of \
                 truth, and the marker is what refuses a generator writing into it"
            );
            assert!(
                WritableDir::claim(&dir).is_err(),
                "{relative} must refuse a write claim"
            );
        }
    }

    #[test]
    fn refuses_a_human_authored_directory() -> Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("errors");
        std::fs::create_dir_all(&dir)?;
        mark_human_authored(&dir)?;

        let error = WritableDir::claim(&dir).expect_err("a human-authored dir must be refused");
        assert!(
            error.to_string().contains(HUMAN_AUTHORED_MARKER),
            "the error must name the marker so the reader can act on it"
        );
        Ok(())
    }

    #[test]
    fn allows_and_writes_into_an_unclaimed_directory() -> Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("proposals");
        std::fs::create_dir_all(&dir)?;

        let writable = WritableDir::claim(&dir)?;
        writable.write(Path::new("E999_proposed.md"), "# E999\n")?;
        assert!(dir.join("E999_proposed.md").exists());
        Ok(())
    }

    #[test]
    fn refuses_a_directory_claiming_two_owners() -> Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("confused");
        std::fs::create_dir_all(&dir)?;
        mark_human_authored(&dir)?;
        std::fs::write(dir.join(OWNERSHIP_MARKER), "")?;

        let error = WritableDir::claim(&dir).expect_err("two owners must be refused");
        assert!(error.to_string().contains("two owners"));
        Ok(())
    }

    #[test]
    fn claims_and_clears_a_new_directory() -> Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("generated");

        clear_owned(&dir)?;
        assert!(dir.join(OWNERSHIP_MARKER).exists());

        std::fs::write(dir.join("stale.txt"), "old")?;
        clear_owned(&dir)?;
        assert!(
            !dir.join("stale.txt").exists(),
            "stale output must be cleared"
        );
        assert!(
            dir.join(OWNERSHIP_MARKER).exists(),
            "the claim must survive"
        );
        Ok(())
    }

    #[test]
    fn refuses_a_directory_it_does_not_own() -> Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("shared");
        std::fs::create_dir_all(&dir)?;
        let precious = dir.join("marker_density.txt");
        std::fs::write(&precious, "1468 lines of mined corpus data")?;

        let error = clear_owned(&dir).expect_err("an unmarked directory must be refused");
        assert!(precious.exists(), "the hand-maintained file must survive");
        assert!(
            error.to_string().contains(OWNERSHIP_MARKER),
            "the error must name the marker so the reader can act on it"
        );
        Ok(())
    }
}
