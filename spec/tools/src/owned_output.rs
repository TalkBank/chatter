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

#[cfg(test)]
mod tests {
    use super::*;

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
