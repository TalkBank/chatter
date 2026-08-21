//! The vocabulary of the CHAT error-spec format, shared by BOTH cargo
//! workspaces.
//!
//! # Why this is a crate and not a module
//!
//! `spec/errors/*.md` is read from two cargo workspaces. The spec workspace
//! has the loader; the main workspace's test crates need the same closed sets
//! to check the specs against the live `ErrorCode` enum, and cannot depend on
//! the loader, which pulls comrak, tera, clap and tree-sitter.
//!
//! So each side had its own copy of the vocabulary, and they diverged in the
//! way copies always do: `talkbank-parser-tests` carried a `SpecStatus` with a
//! FIFTH variant, `Undeclared`, documented as "the majority" on the strength of
//! a count of 239 files with 105 declaring nothing. Measured 2026-08-18: 236
//! files, ZERO declaring nothing, and the spec-side loader refuses a spec that
//! declares none. One reader treated the case as normal, the other as fatal,
//! and the variant was unreachable.
//!
//! This crate is the one owner. It is deliberately dependency-light, because
//! the main workspace compiles it as part of its test graph.

// A dangling rustdoc link is a doc naming an API that does not exist, which is
// the cheapest possible form of the rot this crate exists to prevent. Phase 1b
// shipped two of them within one commit: a link to a module the same commit
// deleted, and one to a method that never existed. Denied rather than warned,
// so deleting a symbol breaks the build instead of rotting a reference.
#![deny(rustdoc::broken_intra_doc_links)]

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub mod frontmatter;
pub mod observations;

/// Raised when a metadata value fails the rule its type states.
#[derive(Debug, thiserror::Error)]
#[error("unknown {field} value {value:?}")]
pub struct UnknownMetadataValue {
    field: &'static str,
    value: String,
}

/// Implementation status of an error spec: whether the validator actually
/// checks its rule. The runner asserts `Implemented` examples fire and skips
/// the rest.
/// # There is deliberately no `Default`
///
/// There was one, `#[default] Implemented`, and removing it is the point of
/// this note. `error.rs` had already made the `**Status**` bullet REQUIRED,
/// naming the file that lacks it, on the reasoning that 104 of 238 specs once
/// declared nothing and had an answer invented for them. When the vocabulary
/// moved here, that `Default` came with it and `error_corpus.rs` kept reaching
/// for it -- so the invented answer had been promoted from one parser's habit
/// into a property of the FORMAT, which is where it would have looked
/// authoritative.
///
/// A `Default` whose wrong value is invisible is banned by name in this
/// project. Both parsers now require the bullet, and they agree by type rather
/// than by two docstrings that said different things.
/// # Deserialized THROUGH [`FromStr`], not by a `rename_all` derive
///
/// A `rename_all = "snake_case"` derive spells the four names a second time,
/// in serde's generated code, where nothing holds them to [`Self::as_str`] or
/// to `from_str`. Phase 1b left this on the derive while giving `SpecLevel`
/// and `ErrorKind` the `try_from` route for exactly that reason, which also
/// orphaned this `FromStr` impl: it had no callers at all between the deletion
/// of the bullet field types and this note. One table, one route, and the four
/// vocabulary types now read the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Status {
    /// Rule implemented; each example must produce its declared codes.
    Implemented,
    /// Rule not implemented yet; examples are skipped by the runner.
    NotImplemented,
    /// Code deprecated/replaced by another; examples are skipped.
    Deprecated,
    /// Rule IS implemented, but no CHAT input can trigger it, so it cannot
    /// have a fixture and the corpus gate must not demand one.
    ///
    /// This exists because the honest alternatives were both wrong. Leaving
    /// such a spec as `Implemented` with no example made it vanish from the
    /// loader entirely (see `load_all`), taking it out of reach of the very
    /// gate meant to catch an untested rule; marking it `NotImplemented`
    /// would state something false about a rule that fires. A spec in this
    /// state owes a NAMED out-of-corpus regression test in its status note,
    /// since the corpus cannot carry one.
    ///
    /// First user: E768, whose value cannot appear in a `.cha` file because
    /// both parsers end an `@Media` filename at the comma.
    UnreachableFromChat,
}

impl Status {
    /// The spelling a spec file uses, which is also what `Display` renders.
    ///
    /// ONE table. `FromStr` reads it and `Display` writes it, so the parser and
    /// the reports cannot disagree about what a status is called, and a renamed
    /// variant changes both at once.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Implemented => "implemented",
            Self::NotImplemented => "not_implemented",
            Self::Deprecated => "deprecated",
            Self::UnreachableFromChat => "unreachable_from_chat",
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = UnknownMetadataValue;

    /// A plain match. (Its former sibling `SpecLayer` left with R4:
    /// layer is observed in the snapshot now, never authored.)
    ///
    /// This was briefly written as a `find` over a `Status::ALL` array, with a
    /// doc claiming it was "derived from `as_str` so the two cannot drift". That
    /// was false: `ALL` is a second hand-maintained list, of variants instead of
    /// strings, and nothing checks it for completeness. A fifth variant would
    /// break `as_str`'s match at compile time -- which is the guard that matters
    /// -- while `ALL` stayed `[Self; 4]`, so `from_str` would silently reject the
    /// new spelling and every spec declaring it would fail to load. The array
    /// reintroduced one level up exactly the drift it was said to prevent, and
    /// it made the two enums in this file parse two different ways for no
    /// reason a reader could find.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "implemented" => Ok(Self::Implemented),
            "not_implemented" => Ok(Self::NotImplemented),
            "deprecated" => Ok(Self::Deprecated),
            "unreachable_from_chat" => Ok(Self::UnreachableFromChat),
            other => Err(UnknownMetadataValue {
                field: "Status",
                value: other.to_owned(),
            }),
        }
    }
}

// `into = "String"` as well as `try_from`, so WRITING goes through `as_str`
// too. `rename_all = "snake_case"` governed both directions, and replacing it
// with only the read half silently changed what `manifest.json` serializes
// from `implemented` to `Implemented`. The manifest round-trip test caught it,
// which is what a wire-format test is for.
impl From<Status> for String {
    fn from(status: Status) -> Self {
        status.as_str().to_owned()
    }
}

impl TryFrom<String> for Status {
    type Error = UnknownMetadataValue;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

/// A CHAT error/warning code as written in a spec: `E` or `W` followed by three
/// digits.
///
/// # Every route in, enumerated
///
/// [`FromStr`] is the only one that can produce a value. [`Self::parse`] is a
/// yes/no wrapper over it, and `Deserialize` is routed through `TryFrom<String>`
/// so a code read back from `manifest.json` is held to the same rule as one read
/// from markdown. Without that last line the derive would have built one from
/// any JSON string, which is a forgeable proof rather than a proof, and the
/// manifest is exactly the round trip where it would have mattered.
///
/// # Why this lives beside the closed vocabularies
///
/// It is not a closed set, so it is a validated newtype rather than an enum.
/// It is here for the same reason [`Status`] is: it is a
/// value type belonging to the spec FORMAT, and it was previously owned by
/// the deleted `error_corpus` while `error.rs`, the OTHER parser of the same
/// files, held the same field as a bare `String` with no validation at all.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SpecErrorCode(String);

/// Is this token the exact `E###` / `W###` shape?
///
/// The single owner of the rule. [`SpecErrorCode::parse`] and [`FromStr`] both
/// ask it, so the yes/no path allocates nothing. That mattered when codes were
/// tokenized out of an `**Expected Error Codes**` LINE, where asking `FromStr`
/// and dropping its reason built an owned error for every prose word; codes
/// now arrive as a typed TOML array, so the saving is smaller and the split is
/// kept for the yes/no callers rather than for that.
fn is_exact_code(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() == 4 && matches!(bytes[0], b'E' | b'W') && bytes[1..].iter().all(u8::is_ascii_digit)
}

/// Was this token plainly MEANT to be an error code? `E` or `W` then a digit.
///
/// Deliberately looser than [`is_exact_code`]: its job is to separate a typo
/// from an ordinary word, so it must accept exactly the things that are wrong.
/// Also the filename test for "is this file an error spec", which is the same
/// question asked of a stem rather than a token.
pub fn looks_like_a_code(token: &str) -> bool {
    let mut chars = token.chars();
    matches!(chars.next(), Some('E' | 'W')) && chars.next().is_some_and(|c| c.is_ascii_digit())
}

impl SpecErrorCode {
    /// Parse an `E###`/`W###` token; `None` if it is not well formed.
    ///
    /// A yes/no verdict on an arbitrary token, where [`FromStr`]'s error would
    /// be built and discarded. Its one caller today is a test fixture in the
    /// spec tools; a caller reading a spec FILE wants [`FromStr`], whose error
    /// says what was wrong and can be reported against the file.
    pub fn parse(token: &str) -> Option<Self> {
        let token = token.trim();
        is_exact_code(token).then(|| Self(token.to_owned()))
    }

    /// The underlying `E###`/`W###` text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for SpecErrorCode {
    type Err = UnknownMetadataValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or_else(|| UnknownMetadataValue {
            field: "error code",
            value: value.trim().to_owned(),
        })
    }
}

impl TryFrom<String> for SpecErrorCode {
    type Error = UnknownMetadataValue;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<SpecErrorCode> for String {
    fn from(code: SpecErrorCode) -> Self {
        code.0
    }
}

impl fmt::Display for SpecErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The structural level an example's fault occurs at.
///
/// A CLOSED set: the five variants are the whole vocabulary, matching what
/// the corpus attests (every `level` in `spec/errors/` is one of these five)
/// and what every authored doc publishes. This was a validated open-string
/// newtype until 2026-08-21, on the theory that levels were "an open set
/// across corpora"; they never were, and the open type let a typo'd level
/// (`'wrod'`) parse, split a duplicate group in `by_code`, and publish itself
/// on the error page with nothing but a reader to notice. The enum makes the
/// wrong value unrepresentable and gives ordering away for free (`Ord` is
/// declaration order: containment from smallest fault site to whole file).
///
/// # Every route in, enumerated
///
/// [`FromStr`], and `Deserialize` routed THROUGH it via [`TryFrom<String>`],
/// so a level read out of frontmatter is held to the same rule as one parsed
/// from text. There is no third route: a bare derive would have accepted the
/// serde-renamed variants only, but the trim in `from_str` and the shared
/// error type are worth the one indirection. Nothing serializes a
/// `SpecLevel`, so `Serialize` and `Into<String>` stay deleted rather than
/// returning as a matched pair nobody asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "String")]
pub enum SpecLevel {
    /// A fault inside a single word.
    Word,
    /// A fault in an utterance's structure.
    Utterance,
    /// A fault on a dependent tier.
    Tier,
    /// A fault in an `@`-header.
    Header,
    /// A fault about the file as a whole.
    File,
}

impl SpecLevel {
    /// The written form, as authored in frontmatter and published on pages.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Word => "word",
            Self::Utterance => "utterance",
            Self::Tier => "tier",
            Self::Header => "header",
            Self::File => "file",
        }
    }
}

impl TryFrom<String> for SpecLevel {
    type Error = UnknownMetadataValue;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl FromStr for SpecLevel {
    type Err = UnknownMetadataValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "word" => Ok(Self::Word),
            "utterance" => Ok(Self::Utterance),
            "tier" => Ok(Self::Tier),
            "header" => Ok(Self::Header),
            "file" => Ok(Self::File),
            _ => Err(UnknownMetadataValue {
                field: "Level",
                value: value.to_owned(),
            }),
        }
    }
}

impl fmt::Display for SpecLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Every error-spec file under `root`, sorted, and NOTHING else in the
/// directory.
///
/// # One owner, because two rules that agree today are still two rules
///
/// The two parsers of these files each had their own answer to "is this file a
/// spec". The spec loader used a DENYLIST (not `_`-prefixed, not `README.md`,
/// not `SPEC_ENHANCEMENT_GUIDE.md`); the deleted `error_corpus` asked whether the
/// stem [`looks_like_a_code`]. Measured 2026-08-18, they select the identical
/// 236 files, which is exactly what makes the duplication easy to keep: the
/// divergence is latent, not visible. Add `notes.md` and the denylist parses
/// prose as a spec while the other skips it; add `E999_draft.md` and both take
/// it; add `_E999.md` and neither does.
///
/// The code-shaped rule is the one kept, because it says what a spec IS rather
/// than listing the things that have turned up next to one, so prose added to
/// this directory tomorrow needs no edit here.
///
/// It lives in this crate, rather than beside the loader, because BOTH cargo
/// workspaces enumerate this directory. The predicate was shared on 2026-08-19
/// and the ENUMERATION was not, so `talkbank-parser-tests` kept its own
/// `read_dir` with its own sort key. They agreed only because `spec/errors` is
/// flat: on a nested tree, per-directory traversal order and full-path
/// lexicographic order are different answers.
///
/// Sorted, because generated artifacts are compared byte-for-byte and
/// `WalkDir` yields in filesystem order. The denylist parser sorted; the other
/// did not.
///
/// Sorted ONCE, on the full paths. A `WalkDir::sort_by_file_name()` here as
/// well was dead work that also hid which ordering is meant: the two agree
/// only because `spec/errors` is flat, and on a nested tree per-directory
/// traversal order and full-path lexicographic order are different answers.
pub fn spec_file_paths(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        // A directory we cannot read is reported, never skipped: a spec that
        // silently leaves the set takes every gate that would have judged it.
        let entry = entry.map_err(|err| format!("could not walk {}: {err}", root.display()))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "md")
            && path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(looks_like_a_code)
        {
            paths.push(path.to_path_buf());
        }
    }
    paths.sort();
    Ok(paths)
}

/// A spec's `## Description` section: the whole thing, plus the one-paragraph
/// summary the index renders.
///
/// # Why the truncation is a TYPE and not a loader rule
///
/// The two parsers of these files each resolved "the description" differently
/// and silently: `error.rs` kept the first paragraph and dropped the rest,
/// `error_corpus.rs` joined every paragraph with a space. Neither said so, and
/// the first is why 51 of the 236 published pages ended mid-thought.
///
/// Both were really answering a PRESENTATION question in the loader. An index
/// wants one paragraph and a code's own page wants the section; that is a
/// choice each renderer should state at its call site, where a reader can see
/// it, rather than a fact the parser decides once for everybody. So the loader
/// keeps everything and the renderers pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecDescription(String);

impl SpecDescription {
    /// The section as authored, markdown and paragraph breaks intact.
    pub fn full(&self) -> &str {
        &self.0
    }

    /// The first paragraph, for a listing that shows many codes at once.
    ///
    /// Rendering `full()` in the index took it from 1,894 lines to 2,800, which
    /// is what an index is supposed to save you from reading.
    pub fn summary(&self) -> &str {
        match self.0.split_once("\n\n") {
            Some((first, _)) => first.trim_end(),
            None => self.0.trim_end(),
        }
    }
}

impl FromStr for SpecDescription {
    type Err = UnknownMetadataValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let text = value.trim();
        if text.is_empty() {
            return Err(UnknownMetadataValue {
                field: "description",
                value: value.to_owned(),
            });
        }
        Ok(Self(text.to_owned()))
    }
}

impl fmt::Display for SpecDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
