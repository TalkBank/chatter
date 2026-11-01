// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Utilities for loading test fixtures.

// Each test binary that says `mod fixture_utils;` compiles this module
// independently and warns on any helper it doesn't use, same convention
// as `tests/cli_common/mod.rs` elsewhere in the workspace.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// File transport variants; authored bytes remain a separate observation.
#[derive(Clone, Copy, Debug)]
pub enum LineEnding {
    Authored,
    CrLf,
    Cr,
}

impl LineEnding {
    pub const ALL: [Self; 3] = [Self::Authored, Self::CrLf, Self::Cr];

    pub fn apply(self, text: &str) -> std::borrow::Cow<'_, str> {
        let newline = match self {
            Self::Authored => return text.into(),
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        };
        text.replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\n', newline)
            .into()
    }
}

#[test]
fn transport_variants_preserve_content_and_empty_lines() {
    let text = "é\r\n\r\n二\rx\ny";
    assert_eq!(LineEnding::Authored.apply(text), text);
    assert_eq!(LineEnding::CrLf.apply(text), "é\r\n\r\n二\r\nx\r\ny");
    assert_eq!(LineEnding::Cr.apply(text), "é\r\r二\rx\ry");
    for ending in LineEnding::ALL {
        assert_eq!(ending.apply(""), "");
    }
    assert_eq!(LineEnding::CrLf.apply("x\n"), "x\r\n");
    assert_eq!(LineEnding::Cr.apply("x\n"), "x\r");
}

/// Admitted runtime checkout, shared with the other test and audit crates.
pub fn workspace_root() -> &'static Path {
    talkbank_repo::root()
        .expect("runtime chatter checkout")
        .as_path()
}

/// Meta-repository root (the parent of [`workspace_root`]), where the
/// out-of-workspace `data/` corpora live.
///
/// Used by the two corpus-divergence tests (`quick_divergence_check.rs`,
/// `lexer_tests.rs`) that read CHAT files from `data/aphasia-data/`,
/// `data/biling-data/`, etc., a `data/` tree expected one level above
/// this repository's root.
pub fn meta_repo_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        workspace_root()
            .parent()
            .unwrap_or_else(|| {
                panic!(
                    "meta_repo_root(): workspace_root {} has no parent",
                    workspace_root().display()
                )
            })
            .to_path_buf()
    })
}

/// A checked-in fixture with at least one entry. Only the loader constructs it;
/// callers cannot turn missing or empty input into a successful zero-iteration test.
pub struct FixtureEntries {
    first: String,
    rest: Vec<String>,
}

type FixtureIter<'a> = std::iter::Chain<std::iter::Once<&'a String>, std::slice::Iter<'a, String>>;

#[derive(Debug, thiserror::Error)]
enum FixtureLoadError {
    #[error("could not read fixture {path}: {source}", path = path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("fixture {path} contains no entries", path = .0.display())]
    Empty(PathBuf),
}

impl FixtureEntries {
    fn read(path: &Path) -> Result<Self, FixtureLoadError> {
        let content = std::fs::read_to_string(path).map_err(|source| FixtureLoadError::Read {
            path: path.to_owned(),
            source,
        })?;
        let mut entries = Vec::new();
        let mut current = String::new();
        for line in content.lines() {
            if line.starts_with('#') {
                continue;
            }
            if line.is_empty() {
                if !current.is_empty() {
                    entries.push(std::mem::take(&mut current));
                }
            } else {
                if !current.is_empty() {
                    current.push('\n');
                }
                current.push_str(line);
            }
        }
        if !current.is_empty() {
            entries.push(current);
        }
        let mut entries = entries.into_iter();
        let first = entries
            .next()
            .ok_or_else(|| FixtureLoadError::Empty(path.to_owned()))?;
        Ok(Self {
            first,
            rest: entries.collect(),
        })
    }

    pub fn entry_count(&self) -> usize {
        1 + self.rest.len()
    }

    pub fn iter(&self) -> FixtureIter<'_> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }
}

impl<'a> IntoIterator for &'a FixtureEntries {
    type Item = &'a String;
    type IntoIter = FixtureIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Load one checked-in fixture or fail the requesting test.
/// This belongs to the integration target, never the production library API.
pub fn load_fixture(name: &str) -> FixtureEntries {
    let path = talkbank_repo::root()
        .expect("runtime chatter checkout")
        .join("crates/talkbank-parser-re2c/tests/fixtures")
        .join(format!("{name}.txt"));
    FixtureEntries::read(&path).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn empty_fixture_files_refuse_admission() {
    let root = talkbank_repo::root()
        .expect("runtime chatter checkout")
        .join("crates/talkbank-parser-re2c/tests/fixtures");
    for name in ["header_recording_quality", "tier_flo", "tier_xcod"] {
        let path = root.join(format!("{name}.txt"));
        assert!(
            matches!(FixtureEntries::read(&path), Err(FixtureLoadError::Empty(ref rejected)) if rejected == &path)
        );
    }
}

#[test]
fn missing_fixture_retains_its_io_failure() {
    let path = talkbank_repo::root()
        .expect("runtime chatter checkout")
        .join("crates/talkbank-parser-re2c/tests/fixtures/__absent_fixture__.txt");
    assert!(
        matches!(FixtureEntries::read(&path), Err(FixtureLoadError::Read { path: rejected, source }) if rejected == path && source.kind() == std::io::ErrorKind::NotFound)
    );
}

/// Load fixtures and verify all lex cleanly (zero error tokens).
/// Returns the entries for further testing.
pub fn load_and_verify_lex(name: &str) -> FixtureEntries {
    let entries = load_fixture(name);
    let mut errors = 0;
    for entry in &entries {
        let input = if entry.ends_with('\n') {
            entry.clone()
        } else {
            format!("{entry}\n")
        };
        let result = talkbank_parser_re2c::lex(&input);
        if !result.is_clean() {
            errors += 1;
            if errors <= 3 {
                let snippet = entry.chars().take(60).collect::<String>();
                eprintln!("  LEX ERROR in {name}: {}", snippet.escape_debug());
                eprint!("{}", result.error_report());
            }
        }
    }
    assert_eq!(
        errors,
        0,
        "{name}: {errors}/{} entries had lex errors",
        entries.entry_count()
    );
    eprintln!("  {name}: {} entries, all lex clean", entries.entry_count());
    entries
}
