//! Source-root admission for the spec tools, shared with the main workspace.

use std::path::{Path, PathBuf};
use talkbank_spec_vocabulary::registry::{CodeRegistry, RegistryLoadError};

pub use talkbank_repo::NotARepoRoot;

/// The Chatter checkout selected for a generator invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRoot(talkbank_repo::RepoRoot);

/// Context for a required checkout admission in a test or generator.
pub const NOT_A_CHECKOUT: &str = "run inside a chatter checkout or set CHATTER_REPO_ROOT";

impl RepoRoot {
    /// Read this admitted checkout's code registry.
    pub fn code_registry(&self) -> Result<CodeRegistry, RegistryLoadError> {
        CodeRegistry::load(self.as_path())
    }

    /// Admit an operator's exact root, or discover the runtime checkout.
    /// An explicit invalid root is rejected without falling back.
    pub fn resolve(given: Option<PathBuf>) -> Result<Self, NotARepoRoot> {
        match given {
            Some(path) => talkbank_repo::RepoRoot::at(path),
            None => talkbank_repo::RepoRoot::discover(),
        }
        .map(Self)
    }

    /// Borrow the admitted absolute root.
    pub fn as_path(&self) -> &Path {
        self.0.as_path()
    }

    /// Resolve a repository-relative artifact or source path.
    pub fn join(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.0.join(relative)
    }
}

impl AsRef<Path> for RepoRoot {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_a_directory_that_is_not_a_chatter_checkout() -> Result<(), std::io::Error> {
        let empty = tempfile::tempdir()?;
        match RepoRoot::resolve(Some(empty.path().to_path_buf())) {
            Err(NotARepoRoot::NotAChatterCheckout { missing, .. }) => {
                assert_eq!(missing, "spec/errors");
            }
            other => panic!("an empty directory must not resolve as a root: {other:?}"),
        }
        Ok(())
    }
}
