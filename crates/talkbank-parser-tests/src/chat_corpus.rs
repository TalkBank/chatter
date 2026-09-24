//! Admit a nonempty, readable fixture corpus before measuring its behavior.
//!
//! Discovery uses the shared checked walk. Consumers cannot silently skip a
//! missing directory, a failed walk, or an unreadable source and report success
//! over only the remaining files. Admission says nothing about CHAT validity;
//! that is the parser or validator's job.

use std::path::{Path, PathBuf};

use crate::construct_coverage::cha_files_under;
use crate::test_error::TestError;

/// A fixture path bound to the exact source read from it.
pub struct ChatFixture {
    path: PathBuf,
    source: String,
}

impl ChatFixture {
    /// Path used for failure attribution.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// UTF-8 source; not a claim that the fixture is valid CHAT.
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// A complete, nonempty discovery result whose files were all readable.
pub struct ChatCorpus {
    fixtures: Vec<ChatFixture>,
}

impl ChatCorpus {
    /// Read all CHAT fixtures under `root`, or fail without admitting a subset.
    pub fn read(root: &Path) -> Result<Self, TestError> {
        let fixtures = cha_files_under(root)?
            .into_iter()
            .map(|path| {
                let source = std::fs::read_to_string(&path).map_err(|error| {
                    TestError::Failure(format!("cannot read {}: {error}", path.display()))
                })?;
                Ok(ChatFixture { path, source })
            })
            .collect::<Result<Vec<_>, TestError>>()?;
        Ok(Self { fixtures })
    }

    /// Admit the committed reference corpus using the shared workspace locator.
    pub fn reference() -> Result<Self, TestError> {
        Self::read(&crate::repo_paths::workspace_root().join("corpus/reference"))
    }

    /// Every admitted fixture, in deterministic path order.
    pub fn fixtures(&self) -> &[ChatFixture] {
        &self.fixtures
    }
}
