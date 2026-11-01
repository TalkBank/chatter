//! Runtime source roots for tests and checkout-based development tools.
//!
//! No build-machine path is compiled into this crate. Cargo runs tests in their
//! package directory; other runners may start anywhere beneath the checkout.
//! A caller outside the checkout selects it explicitly with `CHATTER_REPO_ROOT`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A checkout whose identifying directories and manifest have been admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRoot(PathBuf);

/// Failure to admit a runtime checkout. Explicit selections never fall back.
#[derive(Debug, thiserror::Error)]
pub enum NotARepoRoot {
    /// The operating system could not supply the current directory.
    #[error("cannot read the current directory: {0}")]
    CurrentDirectory(#[source] std::io::Error),
    /// The selected path could not be resolved.
    #[error("cannot resolve checkout path {}: {source}", path.display())]
    Resolve {
        /// Selected path.
        path: PathBuf,
        /// Filesystem failure.
        source: std::io::Error,
    },
    /// A required entry is absent or has the wrong kind.
    #[error("{} is not the chatter repository root: it has no {missing}", root.display())]
    NotAChatterCheckout {
        /// Rejected root.
        root: PathBuf,
        /// First missing identifying entry.
        missing: &'static str,
    },
    /// No ancestor of the runtime directory is a checkout.
    #[error("no chatter checkout above {}; run inside the checkout or set CHATTER_REPO_ROOT", start.display())]
    NotFound {
        /// Runtime search origin.
        start: PathBuf,
    },
}

impl RepoRoot {
    /// Resolve an explicit selection, or discover from the current directory.
    /// `CHATTER_REPO_ROOT` selects an exact root, never an ancestor search.
    pub fn discover() -> Result<Self, NotARepoRoot> {
        match std::env::var_os("CHATTER_REPO_ROOT") {
            Some(path) => Self::at(PathBuf::from(path)),
            None => {
                Self::find_from(std::env::current_dir().map_err(NotARepoRoot::CurrentDirectory)?)
            }
        }
    }

    /// Admit an exact root. Canonicalization makes later joins independent of
    /// changes to the working directory and resolves platform path aliases.
    pub fn at(path: impl AsRef<Path>) -> Result<Self, NotARepoRoot> {
        let path = path.as_ref();
        let root = path
            .canonicalize()
            .map_err(|source| NotARepoRoot::Resolve {
                path: path.to_owned(),
                source,
            })?;
        Self::verified(root)
    }

    /// Locate a checkout above a runtime directory. A nested independent Cargo
    /// workspace or orphan lockfile cannot be mistaken for the Chatter root.
    pub fn find_from(start: impl AsRef<Path>) -> Result<Self, NotARepoRoot> {
        let start = start.as_ref();
        let start = start
            .canonicalize()
            .map_err(|source| NotARepoRoot::Resolve {
                path: start.to_owned(),
                source,
            })?;
        for candidate in start.ancestors() {
            if let Ok(root) = Self::verified(candidate.to_owned()) {
                return Ok(root);
            }
        }
        Err(NotARepoRoot::NotFound { start })
    }

    fn verified(root: PathBuf) -> Result<Self, NotARepoRoot> {
        for (relative, kind) in [
            ("spec/errors", EntryKind::Directory),
            ("crates", EntryKind::Directory),
            ("Cargo.toml", EntryKind::File),
        ] {
            let path = root.join(relative);
            let present = match kind {
                EntryKind::Directory => path.is_dir(),
                EntryKind::File => path.is_file(),
            };
            if !present {
                return Err(NotARepoRoot::NotAChatterCheckout {
                    root,
                    missing: relative,
                });
            }
        }
        Ok(Self(root))
    }

    /// Borrow the admitted absolute root.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Resolve a repository-relative source or fixture location.
    pub fn join(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.0.join(relative)
    }
}

enum EntryKind {
    Directory,
    File,
}

impl AsRef<Path> for RepoRoot {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

/// Admit the process's runtime checkout once, retaining either proof or error.
/// Tests can fail at this boundary; tools may propagate the typed error.
pub fn root() -> Result<&'static RepoRoot, &'static NotARepoRoot> {
    static ROOT: OnceLock<Result<RepoRoot, NotARepoRoot>> = OnceLock::new();
    ROOT.get_or_init(RepoRoot::discover).as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkout() -> std::io::Result<tempfile::TempDir> {
        let directory = tempfile::tempdir()?;
        for marker in ["spec/errors", "crates", "spec/tools/src"] {
            std::fs::create_dir_all(directory.path().join(marker))?;
        }
        std::fs::write(directory.path().join("Cargo.toml"), "[workspace]\n")?;
        // This nested workspace must not stop root discovery.
        std::fs::write(directory.path().join("spec/Cargo.toml"), "[workspace]\n")?;
        Ok(directory)
    }

    #[test]
    fn runtime_discovery_crosses_nested_workspaces() -> Result<(), Box<dyn std::error::Error>> {
        let directory = checkout()?;
        let root = RepoRoot::find_from(directory.path().join("spec/tools/src"))?;
        assert_eq!(root.as_path(), directory.path().canonicalize()?);
        Ok(())
    }

    #[test]
    fn exact_selection_rejects_a_nested_or_incomplete_root()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = checkout()?;
        assert!(matches!(
            RepoRoot::at(directory.path().join("spec")),
            Err(NotARepoRoot::NotAChatterCheckout {
                missing: "spec/errors",
                ..
            })
        ));
        std::fs::remove_file(directory.path().join("Cargo.toml"))?;
        assert!(matches!(
            RepoRoot::at(directory.path()),
            Err(NotARepoRoot::NotAChatterCheckout {
                missing: "Cargo.toml",
                ..
            })
        ));
        assert!(matches!(
            RepoRoot::find_from(directory.path()),
            Err(NotARepoRoot::NotFound { .. })
        ));
        Ok(())
    }

    #[test]
    fn missing_selection_retains_filesystem_error() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        assert!(
            matches!(RepoRoot::at(directory.path().join("absent")), Err(NotARepoRoot::Resolve { source, .. }) if source.kind() == std::io::ErrorKind::NotFound)
        );
        Ok(())
    }
}
