//! Where the repository root is, answered once for the whole `spec/` workspace.
//!
//! # Why this module exists
//!
//! Six independent implementations, in three different techniques, all
//! computing the same thing from `CARGO_MANIFEST_DIR`:
//! `.ancestors().nth(2)`, `.join("..").join("..")`, and
//! `.parent().and_then(Path::parent)`. One of them already carried a doc
//! comment saying it was public precisely so a second would not be written;
//! two more were written anyway, on 2026-08-15, by someone who had read it.
//! A sentence asking the next person to think is not a mechanism.
//!
//! `env!("CARGO_MANIFEST_DIR")` must expand in the crate that writes it, so
//! what it names is always THIS crate. That is not a limitation here, because
//! both spec member crates sit at `<root>/spec/<crate>`: expanding it once, in
//! this module, resolves the same root every caller in the workspace wants.
//! [`RepoRoot::resolve`] does exactly that, which is why callers in the sibling
//! crate can use it and none of them needs an `env!` of its own.
//!
//! The depth is written down once, in [`RepoRoot::DEPTH`], and what happens
//! when the walk runs out of ancestors is written down once, in
//! [`RepoRoot::from_manifest_dir`].

use std::path::{Path, PathBuf};

/// The chatter repository root.
///
/// A newtype rather than a `PathBuf`, because "the repository root", "a spec
/// directory" and "an output directory" are three different kinds of path that
/// a `&Path` parameter cannot tell apart. Passing the wrong one is not
/// hypothetical here: giving an absolute spec directory where a repo root was
/// meant put this machine's home directory into a committed manifest on
/// 2026-08-15.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRoot(PathBuf);

/// What a gate or test should say when the root cannot be resolved.
///
/// A `const` because four call sites had written the same sentence, three of
/// them with a five-line doc comment explaining the `expect` as well. The
/// message is a fact about the workspace layout, so it has one owner like every
/// other such fact in this module.
pub const NOT_A_CHECKOUT: &str = "this crate must sit under a chatter checkout";

/// Why a directory is not the chatter repository root.
///
/// A typed error rather than a panic, and the difference is not stylistic: the
/// panic was FORCED. Two `Default` impls resolved the root, `Default::default()`
/// has no error channel, and so the only way to report a bad root was to abort
/// the process. Deleting those two impls is what made this type possible, which
/// is the usual shape: an infallible trait high in a call chain turns every
/// failure below it into a panic.
#[derive(Debug, thiserror::Error)]
pub enum NotARepoRoot {
    /// The manifest directory is too shallow to sit at `<root>/spec/<crate>`.
    #[error(
        "{manifest_dir} has fewer than {depth} ancestors, so it is not a crate \
         under spec/ and the repository root cannot be resolved"
    )]
    TooShallow {
        /// The `CARGO_MANIFEST_DIR` that was resolved from.
        manifest_dir: String,
        /// How many levels up the root was expected to be.
        depth: usize,
    },
    /// The candidate exists but is not a chatter checkout.
    #[error("{root} is not the chatter repository root: it has no {missing}")]
    NotAChatterCheckout {
        /// The directory that was checked.
        root: PathBuf,
        /// The first required entry that was absent.
        missing: &'static str,
    },
}

/// Something that must exist for a directory to BE the chatter root.
///
/// A list rather than a chain of `assert!`s so the error can name which entry
/// was missing, which is the whole diagnostic value: "no spec/errors" tells you
/// the crate moved, "no crates" tells you the path points at something else
/// entirely.
struct RootMarker {
    /// Path relative to the candidate root.
    relative: &'static str,
    /// What kind of entry it has to be.
    kind: MarkerKind,
}

/// Whether a marker is a directory or a file.
///
/// An enum rather than a `bool` field named `is_dir`, so the check below reads
/// as a match on what the marker IS rather than on a flag a caller has to
/// remember the polarity of.
enum MarkerKind {
    Directory,
    File,
}

/// What every chatter checkout has and nothing else does.
///
/// Checked in order, and the first absent one names the failure. `Cargo.toml`
/// alone would accept any Rust project; `spec/errors` and `crates` are what make
/// it this one. The assertion these encode is the one the deleted
/// `resolves_to_a_directory_holding_the_spec_tree` test used to make once, moved
/// into a constructor that makes it on every call.
const ROOT_MARKERS: &[RootMarker] = &[
    RootMarker {
        relative: "spec/errors",
        kind: MarkerKind::Directory,
    },
    RootMarker {
        relative: "crates",
        kind: MarkerKind::Directory,
    },
    RootMarker {
        relative: "Cargo.toml",
        kind: MarkerKind::File,
    },
];

impl RepoRoot {
    /// The number of levels from a member crate's manifest directory to the
    /// repository root: `<root>/spec/<crate>`.
    const DEPTH: usize = 2;

    /// Resolve the root from a member crate's `CARGO_MANIFEST_DIR`.
    ///
    /// PRIVATE, and that is the point. Its only caller is [`Self::resolve`], ten
    /// lines below. It was briefly `pub`, with a doc telling callers to invoke it
    /// as `from_manifest_dir(env!("CARGO_MANIFEST_DIR"))` -- which is exactly
    /// what the module doc now tells them NOT to do, because `resolve(None)`
    /// answers the same question from one place. A proof type is only as strong
    /// as its weakest constructor, and leaving this public kept the un-consolidated
    /// path open in the change that consolidated it.
    fn from_manifest_dir(manifest_dir: &str) -> Result<Self, NotARepoRoot> {
        let candidate = Path::new(manifest_dir)
            .ancestors()
            .nth(Self::DEPTH)
            .ok_or_else(|| NotARepoRoot::TooShallow {
                manifest_dir: manifest_dir.to_owned(),
                depth: Self::DEPTH,
            })?;
        Self::verified(candidate.to_path_buf())
    }

    /// The root a run should operate on: the one the operator named, or else
    /// the checkout this binary was built from.
    ///
    /// A named transition rather than an `unwrap_or_else` in a `main`. The
    /// value stays a [`RepoRoot`] the whole way, where the caller used to unwrap
    /// it to a bare `PathBuf` in order to make the two branches the same type.
    /// That downgrade threw away the newtype this module exists to provide,
    /// three lines after resolving it.
    ///
    /// `resolve(None)` is also the plain "this checkout" answer, and is what
    /// every caller that has no flag to offer should use.
    ///
    /// The operator's path is verified exactly as the derived one is. It is the
    /// likelier of the two to be wrong, being typed by hand.
    pub fn resolve(given: Option<PathBuf>) -> Result<Self, NotARepoRoot> {
        match given {
            Some(path) => Self::verified(path),
            None => Self::from_manifest_dir(env!("CARGO_MANIFEST_DIR")),
        }
    }

    /// The ONLY constructor, so holding a `RepoRoot` proves the directory
    /// really is one.
    ///
    /// Private, deliberately. A public `RepoRoot(path)` would let any caller
    /// assert the invariant from raw parts, which makes the type a label rather
    /// than a proof.
    fn verified(candidate: PathBuf) -> Result<Self, NotARepoRoot> {
        for marker in ROOT_MARKERS {
            let full = candidate.join(marker.relative);
            let present = match marker.kind {
                MarkerKind::Directory => full.is_dir(),
                MarkerKind::File => full.is_file(),
            };
            if !present {
                return Err(NotARepoRoot::NotAChatterCheckout {
                    root: candidate,
                    missing: marker.relative,
                });
            }
        }
        Ok(Self(candidate))
    }

    /// Borrow the root as a path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// A path under the root.
    #[must_use]
    pub fn join(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.0.join(relative)
    }
}

impl AsRef<Path> for RepoRoot {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

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

    /// A directory that is not a chatter checkout is REFUSED, naming what it
    /// lacks.
    ///
    /// SURVIVES a type: it reaches the filesystem, which no type of ours does,
    /// and it is the proof that the constructor's check actually fires. Its
    /// predecessor asserted the POSITIVE case, that the resolved root contains
    /// `spec/errors`; that assertion is now the constructor's own precondition,
    /// so every call in the workspace makes it and the test was deleted.
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
