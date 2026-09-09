//! Error corpus generation infrastructure.
//!
//! Programmatically generates test files for all error codes to ensure 100% coverage.
//! Uses [`ChatFileBuilder`](crate::ChatFileBuilder) to create valid CHAT files with
//! specific errors for validation testing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::{Path, PathBuf};

// Validation-error generators (E2xx word, E4xx/E6xx tier, E5xx header, E7xx
// alignment) were retired once `spec/errors/` became the single source of truth
// for validation tests (see `tests/error_corpus/validation_errors/README.md`).
// Only the parse-error and warning generators remain, feeding
// `tests/error_corpus/parse_errors/` and `tests/error_corpus/warnings/`.
pub mod internal_errors;
pub mod parser_errors;
pub mod warnings;

use std::fs;

pub use internal_errors::generate_e0_e1xx_internal_errors;
pub use parser_errors::generate_e3xx_parser_errors;
pub use warnings::generate_wxxx_warnings;

/// Convenience type alias used by all generator functions.
pub type GenResult = Result<usize, Box<dyn std::error::Error>>;

/// Updates file.
pub fn write_file(path: &Path, content: String) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, content)?;
    let fallback = path.to_string_lossy();
    let name = match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => fallback.as_ref(),
    };
    println!("  Generated: {}", name);
    Ok(())
}

/// Where the generated corpus belongs, with a check that it is INSIDE the
/// repository.
///
/// # The bug this function exists to make unrepeatable
///
/// This walked THREE parents from `crates/talkbank-parser-tests`, which is one
/// too many: two reach the repository root and the third leaves it. Every run
/// therefore wrote a full corpus into a `tests/error_corpus` directory beside
/// the repository and touched nothing tracked, so regenerating appeared to
/// succeed and changed nothing. It had been doing that long enough to leave 66
/// files there.
///
/// Counting parents is the kind of arithmetic that is wrong silently, so the
/// count is no longer the thing trusted: the result is checked against the
/// manifest directory it was derived from, and a root that does not contain
/// that directory is refused rather than written to.
pub fn error_corpus_root(manifest_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("manifest dir has no workspace root above it")?;

    if !manifest_dir.starts_with(workspace_root) {
        return Err(format!(
            "refusing to generate outside the repository: {} does not contain {}",
            workspace_root.display(),
            manifest_dir.display()
        )
        .into());
    }

    Ok(workspace_root.join("tests/error_corpus"))
}

#[cfg(test)]
mod corpus_root_tests {
    use super::error_corpus_root;
    use std::path::{Path, PathBuf};

    /// SURVIVES: arithmetic over a compile-time constant that no signature can
    /// describe, and it was wrong in production long enough to leave 66 files
    /// beside the repository.
    #[test]
    fn the_corpus_root_is_inside_the_repository() {
        let manifest = PathBuf::from("/repo/crates/talkbank-parser-tests");
        let root = error_corpus_root(&manifest).expect("a root two parents up");
        assert_eq!(root, Path::new("/repo/tests/error_corpus"));
        assert!(root.starts_with("/repo"));
    }

    /// The exact off-by-one that shipped: one parent further leaves the repo.
    #[test]
    fn one_parent_too_many_would_leave_the_repository() {
        let manifest = Path::new("/repo/crates/talkbank-parser-tests");
        let three_up = manifest
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("three parents exist")
            .join("tests/error_corpus");
        assert!(!three_up.starts_with("/repo"));
        assert_ne!(
            three_up,
            error_corpus_root(manifest).expect("a root two parents up")
        );
    }
}
