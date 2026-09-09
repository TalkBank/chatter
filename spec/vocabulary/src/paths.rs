//! Paths as they are RECORDED in generated artifacts.
//!
//! Here rather than in the generator because the validation manifest names
//! one of these, and that manifest is read by BOTH cargo workspaces. A
//! type a wire format names must be nameable by every reader of that wire;
//! when it is not, the reader that cannot name it mirrors the whole struct
//! by hand, which is how the runner's copy came to be missing a status
//! variant the generator had.

use std::path::Path;

/// A path recorded INSIDE a committed artifact: relative to the repository
/// root, with forward slashes.
///
/// # Why this is a type and not a `String`
///
/// A recorded provenance path that depends on where the generator was RUN is
/// not provenance; it is a fact about somebody's laptop, and it lands in a file
/// under version control. That is not hypothetical: on 2026-08-15 the registry
/// started passing absolute spec directories, which is the correct thing to
/// pass, and every `source_spec` in the committed manifest became
/// `/Users/.../chatter/spec/errors/E243_auto.md`.
///
/// The first fix was a free function every caller had to remember to call, with
/// the rule written in its doc comment. Four call sites called it correctly and
/// nothing stopped a fifth from assigning a raw path to a `String` field. The
/// only constructor now takes the root, so the stripping cannot be skipped:
/// possession of one of these IS the proof that it is relative.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct RepoRelativePath(String);

impl RepoRelativePath {
    /// Record `path` relative to `repo_root`.
    ///
    /// A path already outside the root is kept as it is: this is a recording
    /// step, not a validator, and inventing a relative form for something that
    /// genuinely lives elsewhere would be worse than saying where it is.
    #[must_use]
    pub fn new(repo_root: &Path, path: &str) -> Self {
        match Path::new(path).strip_prefix(repo_root) {
            // Inside the tree: join the components, which normalises the
            // separator to `/` so the recorded text is the same on every
            // platform.
            Ok(relative) => Self(
                relative
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
            ),
            // Outside the tree: keep the path verbatim. Running it through the
            // component join would render a leading `RootDir` as `/` and then
            // add a separator after it, so `/elsewhere/x.md` came back as
            // `//elsewhere/x.md`. The free function this replaced had the same
            // defect and no test that could see it.
            Err(_) => Self(path.to_owned()),
        }
    }

    /// The recorded text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RepoRelativePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path under the root records relative; one outside records as it is.
    ///
    /// SURVIVES a type: this is the recording POLICY for the out-of-tree case,
    /// which is a choice with a real alternative (refuse it), not an invariant.
    #[test]
    fn records_relative_to_the_root_and_leaves_outsiders_alone() {
        let root = Path::new("/tmp/checkout");
        assert_eq!(
            RepoRelativePath::new(root, "/tmp/checkout/spec/errors/E202.md").as_str(),
            "spec/errors/E202.md"
        );
        assert_eq!(
            RepoRelativePath::new(root, "/elsewhere/E202.md").as_str(),
            "/elsewhere/E202.md"
        );
    }
}
