//! One reader for `spec/errors/*.md`, shared by everything in this workspace
//! that needs to know what a spec says about itself.
//!
//! # Why this exists
//!
//! The `spec/errors` markdown format had four readers by 2026-08-11, three of
//! them in this workspace, each re-deriving the same two conventions: that a
//! spec is named `<CODE>_<slug>.md`, and that its state is declared on a
//! `**Status**:` line. Every copy also re-derived the reason the filename is
//! split on the FIRST underscore rather than matched with `starts_with`, which
//! is that `E21` would otherwise claim `E210_auto.md`.
//!
//! Duplication that costs something: on the day this was written, one reader
//! decided a spec's status with `content.contains("not_implemented")` over the
//! whole file, so a spec mentioning the token in PROSE would silently leave its
//! denominator. Two specs already do.
//!
//! # What is deliberately NOT shared
//!
//! [`SpecStatus`] is the vocabulary; what to DO about each state is the
//! caller's policy and stays with the caller. `SpecStatusGate` treats only
//! `NotImplemented` as unenforced; the re2c parity gate measures everything
//! except `NotImplemented`; `spec/runtime-tools` skips `Deprecated` and
//! `UnreachableFromChat` as well. Those are three different, defensible
//! judgements about the same fact, and collapsing them would be inventing a
//! policy none of them holds.
//!
//! # The fourth reader, and why it is still separate
//!
//! `spec/tools` and `spec/runtime-tools` live in a SEPARATE cargo workspace
//! that depends downward into `talkbank-model`. They cannot import this crate.
//! Sharing with them would mean putting a markdown reader into a published CHAT
//! model crate, which is the wrong home for it: `talkbank-model` describes CHAT
//! transcripts, not this project's spec-authoring format. The remaining
//! duplication across the workspace boundary is therefore deliberate, and is
//! recorded here rather than left for the next reader to rediscover.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use talkbank_model::ErrorCode;

/// What a spec declares about its own implementation state.
///
/// The closed set actually written in `spec/errors`, verified against all 239
/// files: 89 `implemented`, 42 `not_implemented`, 2 `deprecated`, 1
/// `unreachable_from_chat`, and 105 declaring nothing at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecStatus {
    /// Enforced by the validator.
    Implemented,
    /// Documented but not yet enforced.
    NotImplemented,
    /// Superseded; kept for the record.
    Deprecated,
    /// Describes a state no CHAT input can reach.
    UnreachableFromChat,
    /// No `**Status**:` line at all, which is the majority.
    Undeclared,
}

/// One spec file, read once.
pub struct SpecFile {
    /// The file's own name, e.g. `E375_replacement_needs_preceding_space.md`.
    pub filename: String,
    /// Absolute path, for a diagnostic that has to name the file on disk.
    pub path: PathBuf,
    /// The whole file, read once and shared by every caller.
    pub content: String,
}

/// The code a spec FILENAME names: `E375_replacement....md` -> `E375`.
///
/// `None` rather than a guess when the stem parses to no declared code, so a
/// caller reports it as unresolved instead of quietly dropping it. The split is
/// on the first underscore, never `starts_with`, or a hypothetical `E21` would
/// claim `E210_auto.md`. Two specs (`E707.md`, `E711.md`) carry no slug at all,
/// hence the whole-stem fallback.
///
/// A free function, not just a method, because one caller has a filename and no
/// file: it was briefly written as `SpecFile { filename, path: PathBuf::new(),
/// content: String::new() }.code()`, which fabricates two empty fields to reach
/// one accessor and is the shape this workspace keeps deleting.
#[must_use]
pub fn code_of(filename: &str) -> Option<ErrorCode> {
    let stem = filename.strip_suffix(".md")?;
    ErrorCode::parse_exact(stem.split_once('_').map_or(stem, |(code, _)| code))
}

/// The text after an `**Expected Error Codes**:` label, if this line is one.
///
/// Here for the same reason [`SpecFile::status`] is: the `spec/errors` markdown
/// conventions get re-derived by every new reader, and the optional `- ` list
/// marker plus the bold-label spelling are exactly the details that drift. This
/// field had two independent matchers, character-for-character identical, in
/// two crates of one workspace.
#[must_use]
pub fn expected_codes_declaration(line: &str) -> Option<&str> {
    line.trim_start()
        .trim_start_matches("- ")
        .strip_prefix("**Expected Error Codes**:")
        .map(str::trim)
}

impl SpecFile {
    /// The code this spec's filename names. See [`code_of`].
    #[must_use]
    pub fn code(&self) -> Option<ErrorCode> {
        code_of(&self.filename)
    }

    /// What the `**Status**:` line declares.
    ///
    /// Anchored at the start of a line after trimming an optional list marker,
    /// NOT a substring search of the whole file. `Err` for a status nobody has
    /// modelled, so a typo stops the caller rather than silently becoming one
    /// of the five real states.
    pub fn status(&self) -> Result<SpecStatus, String> {
        let Some(raw) = self.content.lines().find_map(|line| {
            line.trim_start()
                .trim_start_matches("- ")
                .strip_prefix("**Status**:")
                .map(str::trim)
        }) else {
            return Ok(SpecStatus::Undeclared);
        };
        match raw {
            "implemented" => Ok(SpecStatus::Implemented),
            "not_implemented" => Ok(SpecStatus::NotImplemented),
            "deprecated" => Ok(SpecStatus::Deprecated),
            "unreachable_from_chat" => Ok(SpecStatus::UnreachableFromChat),
            other => Err(format!(
                "{}: unrecognised status {other:?}; add it to SpecStatus",
                self.filename
            )),
        }
    }
}

/// Every `.md` under `dir`, read, in filename order so a run is reproducible.
///
/// `Err` on an empty directory: a caller comparing against nothing reports
/// clean, and a clean report from a broken read is indistinguishable from a
/// clean report from a healthy one.
pub fn load(dir: &Path) -> Result<Vec<SpecFile>, String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|err| format!("cannot read {}: {err}", dir.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "md"))
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut specs = Vec::new();
    for entry in entries {
        let path = entry.path();
        specs.push(SpecFile {
            filename: entry.file_name().to_string_lossy().into_owned(),
            content: std::fs::read_to_string(&path)
                .map_err(|err| format!("cannot read {}: {err}", path.display()))?,
            path,
        });
    }
    match specs.first() {
        None => Err(format!("no spec files under {}", dir.display())),
        Some(_) => Ok(specs),
    }
}

/// Every code that some spec filename names, for coverage questions.
pub fn specified_codes(specs: &[SpecFile]) -> BTreeSet<ErrorCode> {
    specs.iter().filter_map(SpecFile::code).collect()
}
