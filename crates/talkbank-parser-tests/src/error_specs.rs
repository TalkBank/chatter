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
//! [`Status`] is the vocabulary; what to DO about each state is the
//! caller's policy and stays with the caller. `SpecStatusGate` treats only
//! `NotImplemented` as unenforced; the re2c parity gate measures everything
//! except `NotImplemented`; `spec/runtime-tools` skips `Deprecated` and
//! `UnreachableFromChat` as well. Those are three different, defensible
//! judgements about the same fact, and collapsing them would be inventing a
//! policy none of them holds.
//!
//! # What is shared, and what is not
//!
//! The VALUE types are shared: `talkbank-spec-vocabulary` is a dependency-light
//! crate (serde and thiserror, nothing else) that both cargo workspaces depend
//! on by path, and it owns `Status`, the error-code newtype and the
//! code-shaped filename rule. This paragraph said sharing was impossible until
//! 2026-08-19, on the grounds that it would mean putting a markdown reader into
//! a published CHAT model crate; that was an argument against sharing the
//! READER, and it was read as an argument against sharing anything.
//!
//! The markdown READERS are still separate, and that part still holds: a
//! markdown parser does not belong in a published CHAT model crate, and this
//! one answers a different question anyway (it joins spec filenames against the
//! live `ErrorCode` enum, which the spec workspace cannot see).

use std::collections::BTreeSet;
use std::path::Path;

use talkbank_model::ErrorCode;

use serde::de::IgnoredAny;
use talkbank_spec_vocabulary::SpecErrorCode;
/// What a spec declares about its own implementation state.
///
/// The status vocabulary, owned by [`talkbank_spec_vocabulary`].
///
/// This was a local `SpecStatus` enum, and it is the reason that crate exists.
/// It carried a FIFTH variant, `Undeclared`, whose doc read: "verified against
/// all 239 files: 89 `implemented`, 42 `not_implemented`, 2 `deprecated`, 1
/// `unreachable_from_chat`, and 105 declaring nothing at all."
///
/// Measured 2026-08-19: 236 files, 192 implemented, 41 not_implemented, 2
/// deprecated, 1 unreachable_from_chat, and ZERO declaring nothing. Every
/// number in that doc had drifted, and the last one mattered: the spec-side
/// loader REFUSES a spec with no `**Status**:` bullet, so one reader treated
/// the case as the normal majority while the other treated it as fatal. The
/// variant was unreachable and the counts were three months stale.
pub use talkbank_spec_vocabulary::Status;
use talkbank_spec_vocabulary::frontmatter::{ExampleFrontmatter, SpecFrontmatter};

/// One spec file, read and PARSED once.
///
/// # What Phase 1b deleted here
///
/// This used to carry the whole file as text, and each caller scanned those
/// lines for the field it wanted: one for `**Status**:`, two more for
/// `**Expected Error Codes**:`, and one of those three additionally
/// re-implemented a markdown scanner to find ```` ```chat ```` fences and pair
/// each with the declarations above it. That scanner was the FIFTH reader of
/// this format, and its pairing rule (`declarations between the previous fence
/// and this one, last wins`) was a fourth answer to the question the loader's
/// `raw_after_fence_declares_codes` guard existed to police.
///
/// The frontmatter schema answers all of it by deserializing, so the text is
/// no longer a field: what a caller wants is a value, and there is one way to
/// get it.
pub struct SpecFile {
    /// The file's own name, e.g. `E375_replacement_needs_preceding_space.md`.
    pub filename: String,
    /// What the file declares, parsed at load.
    ///
    /// `IgnoredAny` for the kind: this workspace has no use for it, and the
    /// type parameter is what lets the schema live in a crate that cannot name
    /// `DiagnosticKind`.
    front: SpecFrontmatter<IgnoredAny>,
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
    ErrorCode::parse_exact(stem_code_of(filename)?)
}

/// The code TEXT a spec filename names, before any join to the live enum.
///
/// Split out so `load` can compare a filename against a declared code without
/// dragging in the separate question of whether the live `ErrorCode` enum has
/// a matching variant. `code_of`'s doc justified being a free function
/// "because one caller has a filename and no file"; that caller was the
/// markdown scanner Phase 1b deleted, so the justification outlived its
/// subject by one commit.
#[must_use]
fn stem_code_of(filename: &str) -> Option<&str> {
    let stem = filename.strip_suffix(".md")?;
    Some(stem.split_once('_').map_or(stem, |(code, _)| code))
}

impl SpecFile {
    /// The code this spec's filename names. See [`code_of`].
    #[must_use]
    pub fn code(&self) -> Option<ErrorCode> {
        code_of(&self.filename)
    }

    /// What this spec declares about its own implementation state.
    ///
    /// Infallible, where it used to return a `Result`. A missing or misspelled
    /// status is refused when the file is PARSED, so by the time a `SpecFile`
    /// exists the question has an answer. That is the shape this repository
    /// keeps looking for: not a better error message, but a state that cannot
    /// be reached.
    #[must_use]
    pub fn status(&self) -> Status {
        self.front.status
    }

    /// The code this spec DECLARES, as a spec-format code.
    ///
    /// Distinct from [`Self::code`], which reads the FILENAME and joins it to
    /// the live `ErrorCode` enum. The two agree across all 236 specs today,
    /// and collapsing them is R1's job (`ErrorCode` generated from the specs),
    /// not this phase's: one is a fact about the file's name and the other is
    /// a fact about its content, and Phase 1b is the change that makes the
    /// second one available at all.
    #[must_use]
    pub fn declared_code(&self) -> &SpecErrorCode {
        &self.front.code
    }

    /// This spec's examples, in file order.
    #[must_use]
    pub fn examples(&self) -> &[ExampleFrontmatter] {
        &self.front.examples
    }

    /// Every code this spec's examples POSITIVELY assert, over every example.
    ///
    /// Claim-derived since R2: `violates` contributes the spec's own code,
    /// `subsumed_by` its targets, `legal` nothing (its content is an absence).
    #[must_use]
    pub fn declared_codes(&self) -> BTreeSet<SpecErrorCode> {
        self.front
            .examples
            .iter()
            .flat_map(|example| example.effective_codes(&self.front.code))
            .collect()
    }
}

/// Every error SPEC under `dir`, read, in filename order so a run is
/// reproducible.
///
/// # Specs, not every `.md`
///
/// This took every markdown file, so `README.md` and `SPEC_ENHANCEMENT_GUIDE.md`
/// arrived as specs and every caller had to recognise and skip them. That is
/// what the deleted `Status::Undeclared` variant was really for: prose has no
/// `**Status**:` bullet, so "declares nothing" doubled as "is not a spec", and
/// a genuine spec that forgot the bullet was indistinguishable from a README.
///
/// The rule is now [`talkbank_spec_vocabulary::looks_like_a_code`] on the stem, shared with the
/// spec-side loader, which is the same question asked once.
///
/// `Err` on an empty directory: a caller comparing against nothing reports
/// clean, and a clean report from a broken read is indistinguishable from a
/// clean report from a healthy one.
pub fn load(dir: &Path) -> Result<Vec<SpecFile>, String> {
    // ONE enumeration, shared with the spec-side loader. This had its own
    // `read_dir` plus `sort_by_key(file_name)`, which agreed with
    // `spec_file_paths`'s full-path sort only because `spec/errors` is flat.
    // Bare `?`: the shared walker's error already names the directory, so
    // wrapping it printed the path and a verb twice.
    let paths = talkbank_spec_vocabulary::spec_file_paths(dir)?;

    let mut specs = Vec::new();
    for path in paths {
        // No fallible arm: `spec_file_paths` only yields a path whose STEM is
        // valid UTF-8 and code-shaped, so a file name exists and is UTF-8 by
        // construction. An error branch here would be unreachable, and an
        // unreachable branch needs a fabricated message to fill it.
        let filename = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name.to_owned(),
            None => continue,
        };
        let content = std::fs::read_to_string(&path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        // ONE call, because the schema crate owns the verb now. This was a
        // `split` then a `toml::from_str` with its own error prefix, which is
        // the same two steps the spec-side loader was writing separately.
        let (front, _body): (SpecFrontmatter<IgnoredAny>, &str) =
            talkbank_spec_vocabulary::frontmatter::read(&content)
                .map_err(|why| format!("{filename}: {why}"))?;

        // The FILENAME and the DECLARED code are two statements of one fact.
        // Phase 1b made the second one available and left them uncompared,
        // which is a drift nothing would have reported: rename a spec file and
        // the parity suite asserts one code while three coverage gates count
        // another, silently and forever. Merging them is R1's job (`ErrorCode`
        // generated from the specs); refusing a disagreement is not, and costs
        // four lines.
        if let Some(stem_code) = stem_code_of(&filename)
            && stem_code != front.code.as_str()
        {
            return Err(format!(
                "{filename}: the filename names {stem_code} and the frontmatter \
                 declares {}. One spec, one code.",
                front.code
            ));
        }
        specs.push(SpecFile { filename, front });
    }
    if specs.is_empty() {
        return Err(format!("no spec files under {}", dir.display()));
    }
    Ok(specs)
}

/// Every code that some spec filename names, for coverage questions.
pub fn specified_codes(specs: &[SpecFile]) -> BTreeSet<ErrorCode> {
    specs.iter().filter_map(SpecFile::code).collect()
}
