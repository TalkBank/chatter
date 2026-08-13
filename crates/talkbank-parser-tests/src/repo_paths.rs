//! Locating the workspace root, once, for every audit and gate in this crate.
//!
//! # Why this is a module and not another copy
//!
//! This workspace holds fifteen root-finders in three flavours that do not
//! agree: the `[workspace]` walk-up below, fixed `dir.pop()` counts, and
//! `parent().parent()` chains. The pop-count and parent-chain forms encode the
//! caller's own depth in the tree, so moving or renaming a directory breaks
//! them silently, and that has already happened once (recorded in
//! `talkbank-parser-re2c`'s fixture helper, which carries the other correct
//! copy and cannot be imported because it lives in a `tests/` module).
//!
//! A gate that cannot find the repository does not fail loudly; it usually
//! finds the WRONG directory, walks an empty tree, and reports clean. That is
//! the reason to have one spelling rather than a style preference.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Nearest ancestor directory holding a `[workspace]` Cargo.toml.
///
/// A walk-up, not a fixed number of `..` hops, so it is independent of where
/// the calling crate sits. `[workspace]` rather than `Cargo.lock`, because
/// orphan lockfiles do occur in scratch directories and would stop the walk
/// early.
///
/// Memoised: several gates call this and each call reads a file per level.
///
/// # Panics
///
/// On a tree with no `[workspace]` manifest above `CARGO_MANIFEST_DIR`. That
/// cannot happen in a checkout, and there is no sensible recovery: every
/// caller is a test or an audit whose entire input is the repository.
#[allow(clippy::panic)]
pub fn workspace_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        loop {
            if fs::read_to_string(dir.join("Cargo.toml"))
                .map(|text| text.contains("[workspace]"))
                .unwrap_or(false)
            {
                return dir;
            }
            if !dir.pop() {
                panic!(
                    "no [workspace] Cargo.toml above CARGO_MANIFEST_DIR; \
                     a gate that cannot locate the repository must not report clean"
                );
            }
        }
    })
}
