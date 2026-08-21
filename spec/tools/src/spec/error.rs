//! # Error Specification Types
//!
//! Structured representation of the error spec files in `spec/errors/`.
//!
//! Each Markdown file defines one error code with its metadata (kind,
//! category, layer), a human-readable description, and one or more bad-input
//! examples that should trigger the error. Generators consume these types to
//! emit Rust validation tests and error documentation pages.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

use super::comrak_text::section_source;
use super::metadata::{SpecDescription, SpecErrorCode, SpecLevel, Status};

/// Root structure for an error specification file.
///
/// Loaded from a single `spec/errors/E###_*.md` Markdown file: the metadata its
/// `## Metadata` block declares, and the one error it defines.
///
/// # There is exactly one way to build one, and it is [`Self::load_all`]
///
/// This derived `Deserialize` until 2026-08-18, along with `ErrorMetadata`,
/// `ErrorDefinition` and `ErrorExample`, and nothing in the workspace ever
/// deserialized any of them: there is no JSON or TOML route into a spec, only
/// the markdown loader below. What the derive actually did was make the
/// `#[serde(default)]` attributes read as a documented schema, so five fields
/// no spec file can declare looked like an optional part of the format rather
/// than what they were, which is dead weight the loader filled with empty
/// values. Removing it is what let those fields be deleted.
#[derive(Debug)]
pub struct ErrorSpec {
    /// What the frontmatter declares about the code itself (description,
    /// status, kind). `level` is a fact about each EXAMPLE, not the code.
    pub metadata: ErrorMetadata,
    /// The error this spec defines.
    ///
    /// ONE, not a `Vec`. It was `errors: Vec<ErrorDefinition>` and the loader
    /// has only ever built it as `vec![one]`, so every consumer looped over a
    /// collection that could not have a second element, and several carried a
    /// branch for the empty case that could not happen either: a
    /// `NoErrorDefinitions` failure variant, a `spec.errors.len() > 1` label
    /// arm, an `unwrap_or("<unknown>")` on an index lookup, and TEN loops
    /// across nine files in the two crates. `generate_error_index` named this
    /// cure in a comment before it was applied, estimating "roughly eight";
    /// the commit that applied it repeated the estimate as though it were the
    /// measurement, which is the same defect one level up, so the number here
    /// is `git show 0843effc | grep -c '^-.*in &spec\.errors'`.
    pub error: ErrorDefinition,
    /// The file this spec was loaded from. Not present in the Markdown itself;
    /// set by the loader.
    ///
    /// The full PATH, where this was a bare basename. Both were wanted: the
    /// basename names the spec in reports, and the path is what
    /// `RepoRelativePath` needs to record provenance in `manifest.json`. The
    /// sibling parser carried the path for exactly that reason, so the two
    /// parsers of one file disagreed about what "where did this come from"
    /// means. One field, and the basename is derived.
    pub source_path: std::path::PathBuf,
}

impl ErrorSpec {
    /// The spec file's basename, for naming it in a report.
    // Clippy suggests `.unwrap_or_default()`, which this workspace bans:
    // the arm below is a REASONED empty, not a default, and the reasoning
    // lives on the arm where a reader will meet it.
    #[allow(clippy::manual_unwrap_or_default)]
    pub fn source_file(&self) -> &str {
        match self.source_path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name,
            // `load_all` only ever yields paths that ended in `.md`, so this
            // is unreachable; it returns the whole path rather than inventing
            // a placeholder name for a file that does have one.
            None => "",
        }
    }

    /// The spec file's path as text, for provenance in generated manifests.
    pub fn source_path_display(&self) -> String {
        self.source_path.display().to_string()
    }

    /// The DISTINCT levels this spec's examples cover.
    ///
    /// Usually one; E519 legitimately covers more than one across its specs
    /// (header sites and an utterance site), which is why `level` is
    /// per-example at all. Returns the one type that can hold the answer;
    /// rendering and comparison belong to [`LevelSet`], not to callers.
    #[must_use]
    pub fn levels(&self) -> LevelSet {
        LevelSet::from_examples(self.error.examples.iter().map(|e| e.level))
    }

    /// Does any example here demonstrate the rule this spec is FOR?
    ///
    /// The gate that used to enforce this (`SpecSelfDemonstrationGate`, a
    /// 36-entry shrink-only baseline in the other workspace) was DELETED by
    /// R2: an example must claim something, so "demonstrates nothing" stopped
    /// being writable and the baseline had nothing left to hold. What remains
    /// here is the one name in `spec/tools` for a classification three call
    /// sites had each spelled out by hand, now a report rather than a gate.
    ///
    /// Derived from the CLAIMS since R2: a spec demonstrates its rule iff
    /// some example claims `violates`. A spec whose examples are all
    /// `subsumed_by` is the parser-specificity worklist, and
    /// [`Demonstration::Absent`] carries the targets so the report names what
    /// fires instead.
    #[must_use]
    pub fn demonstration(&self) -> Demonstration {
        use talkbank_spec_vocabulary::frontmatter::Claim;
        if self.error.examples.is_empty() {
            return Demonstration::NoExamples;
        }
        let mut declared: Vec<SpecErrorCode> = Vec::new();
        for example in &self.error.examples {
            match &example.claim {
                Claim::Violates => return Demonstration::ByExample,
                Claim::SubsumedBy(targets) => {
                    for code in targets.as_slice() {
                        if !declared.contains(code) {
                            declared.push(code.clone());
                        }
                    }
                }
                Claim::Legal => {}
            }
        }
        declared.sort();
        Demonstration::Absent { declared }
    }
}

/// The distinct, ordered set of levels a spec's examples cover.
///
/// # A proof type, and its one constructor
///
/// Sorted (by [`SpecLevel`]'s containment order) and deduplicated BY
/// CONSTRUCTION; the only route in is [`ErrorSpec::levels`], so an equality
/// between two `LevelSet`s is meaningful without either side remembering to
/// normalize, which is what `by_code`'s duplicate grouping relies on. When
/// this was a bare `Vec<&SpecLevel>`, the sorted-distinct invariant lived in
/// one method body and the rendering was copy-pasted at both publish sites,
/// one of which forgot the empty case.
#[derive(Debug, PartialEq, Eq)]
pub struct LevelSet(Vec<SpecLevel>);

impl LevelSet {
    /// Build from a spec's examples. Private on purpose: see the type doc.
    fn from_examples(levels: impl Iterator<Item = SpecLevel>) -> Self {
        let mut levels: Vec<SpecLevel> = levels.collect();
        levels.sort_unstable();
        levels.dedup();
        Self(levels)
    }

    /// The published form: levels joined with `, `, or `None` when the spec
    /// has no examples. `None` forces each publish site to say what an
    /// absent level means there (the page omits its Level line; the index
    /// leaves the cell blank), instead of one site handling it and the other
    /// printing an accidental empty string.
    #[must_use]
    pub fn rendered(&self) -> Option<String> {
        if self.0.is_empty() {
            return None;
        }
        Some(
            self.0
                .iter()
                .map(|level| level.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}

/// Whether a spec's examples demonstrate the rule the spec is named for.
///
/// THREE states, because they have three different owners and collapsing any
/// two is how each of them stayed invisible.
/// [`NoExamples`](Self::NoExamples) is what the validation manifest's
/// per-code accounting reports and what `SpecCoverage` calls a stub;
/// [`Absent`](Self::Absent) is the subsumption worklist `coverage --errors`
/// prints (the gate that used to baseline it fell to R2's required claim).
#[derive(Debug)]
pub enum Demonstration {
    /// Some example produces the spec's own code.
    ByExample,
    /// The spec has examples, and every one asserts only OTHER codes.
    ///
    /// Carries what they DO assert, because that is the finding and the gate
    /// in the other workspace reports filenames only. Most of these declare
    /// E316, "unparsable content", meaning the mined input does not parse and
    /// the specific rule is never reached: such a spec documents a GAP rather
    /// than a RULE. No count here; `coverage --errors` prints the live list.
    Absent {
        /// The codes the examples assert instead, sorted and deduplicated.
        declared: Vec<SpecErrorCode>,
    },
    /// The spec carries no example at all.
    ///
    /// Gated by the manifest's `implemented_codes_without_examples`, which is
    /// per-CODE and total across the corpus since R4, so there is no
    /// parser-layer blind spot any more (E001, E002 and E340 sat in one until
    /// then; all three are `unreachable_from_chat` now).
    NoExamples,
}

/// Metadata about the error, as the spec's frontmatter declares it.
#[derive(Debug)]
pub struct ErrorMetadata {
    /// The spec's `## Description` section.
    pub description: SpecDescription,
    /// Implementation status, parsed at load into the closed set.
    ///
    /// Was a `String` on this struct until 2026-08-15, while the sibling parser
    /// of the same files had it typed. Why that mattered, and what it cost, is
    /// recorded once on [`Status`].
    ///
    /// [`Status`] has no `Default`, deliberately, so a spec that declares
    /// nothing is refused rather than given an invented answer.
    pub status: Status,
    /// What this diagnostic intrinsically IS, per the code's own spec.
    ///
    /// Deliberately REQUIRED, not `Option` with a default: a spec file
    /// declaring no `kind` fails to load (see
    /// [`ErrorSpec::from_frontmatter`]) rather than silently falling back to a
    /// guess.
    /// The talkbank-model `DiagnosticKind` registry
    /// (`crates/talkbank-model/src/errors/generated_diagnostic_kind.rs`) is
    /// generated from this field across every spec file, so an unclassified
    /// code is a build-time failure here, not a silent gap in that registry.
    pub kind: ErrorKind,
}

/// The four `DiagnosticKind` axis values a spec file's `## Metadata` block
/// can declare via its `- **Kind**:` bullet.
///
/// Mirrors `talkbank_model::errors::DiagnosticKind` structurally by name.
/// This crate cannot depend on `talkbank-model` (that would be circular:
/// `talkbank-model`'s own diagnostic-kind registry is generated FROM this
/// crate's spec loader, by a binary in the sibling `spec/runtime-tools`
/// crate, which is the one place both directions of the dependency meet).
/// The generator maps each variant here to the identically-named
/// `DiagnosticKind` variant by name; a variant added to one and not the
/// other is caught at the generator's match, not silently ignored.
///
/// # Deserialized THROUGH [`Self::parse`], not by the derive
///
/// A plain `Deserialize` derive would spell the four variant names a second
/// time, in serde's generated code, where nothing holds them to
/// [`Self::as_str`]. That is the same duplication this type's own doc warns
/// about one paragraph down, so the read route goes through the table rather
/// than beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub enum ErrorKind {
    /// Violates the spec, or the construct does not make sense.
    Invalidity,
    /// Preserved but not interpreted: a chatter coverage gap, never a fault
    /// in the file itself.
    Unmodeled,
    /// Valid now, discouraged, on a sunset path toward `Invalidity`.
    Deprecation,
    /// Valid, purely stylistic.
    Style,
}

impl TryFrom<String> for ErrorKind {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl ErrorKind {
    /// ONE table, the way [`Status`] already has one.
    ///
    /// This name is FOUR things at once: what a spec's `kind` frontmatter
    /// value must say,
    /// what the generated `DiagnosticKind` registry emits as source text, what
    /// `docs/errors/*.md` publishes, and what the index table shows. Three of
    /// those were separate matches until 2026-08-15, and the published pair
    /// were `{:?}` on the derived `Debug`, so renaming a variant would have
    /// silently changed user-facing documentation while
    /// `diagnostic_kind_variant` kept emitting the old literal and only
    /// `parse` failed loudly.
    fn as_str(self) -> &'static str {
        match self {
            Self::Invalidity => "Invalidity",
            Self::Unmodeled => "Unmodeled",
            Self::Deprecation => "Deprecation",
            Self::Style => "Style",
        }
    }

    /// Parse a `kind` frontmatter value. Case-sensitive and exact: the
    /// four spelled-out variant names, nothing else (no abbreviations, no
    /// synonyms), so a typo in a spec file fails loudly at load time
    /// instead of silently defaulting.
    fn parse(value: &str) -> Result<Self, String> {
        // A plain match, the shape `Status::from_str` uses. This was a `find`
        // over a four-element array of variants, which
        // that impl's own doc records as tried and REMOVED: the array is a
        // second hand-maintained list that nothing checks for completeness, so
        // a fifth variant breaks `as_str`'s match at compile time (the guard
        // that matters) while the array stays at four and every spec declaring
        // the new value fails to load. This was the last enum in the format
        // still written that way.
        match value.trim() {
            "Invalidity" => Ok(Self::Invalidity),
            "Unmodeled" => Ok(Self::Unmodeled),
            "Deprecation" => Ok(Self::Deprecation),
            "Style" => Ok(Self::Style),
            other => Err(format!(
                "unrecognized Kind value {other:?}: expected one of \
                 Invalidity, Unmodeled, Deprecation, Style"
            )),
        }
    }

    /// The identically-named `talkbank_model::errors::DiagnosticKind`
    /// variant this value maps to, as source text for code generation.
    ///
    /// Identical to [`Self::as_str`] by construction rather than by
    /// coincidence, and named separately because the CALLER's intent differs:
    /// this one is Rust source text and must not drift if the published
    /// spelling ever gains a space.
    pub fn diagnostic_kind_variant(self) -> &'static str {
        self.as_str()
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single error definition
#[derive(Debug)]
pub struct ErrorDefinition {
    /// Error code: "E241", "E520", etc.
    pub code: SpecErrorCode,
    /// Short name: "IllegalUntranscribed", "SpeakerNotInParticipants", etc.
    pub name: String,
    /// The `## CHAT Rule` section: what CHAT requires here, and therefore what
    /// a maintainer must write instead. `None` when the spec declares no such
    /// section (18 of 236 as of 2026-08-18).
    ///
    /// # The one owner of why the published pages had empty sections
    ///
    /// This is the field the published `## How to Fix` section always wanted.
    /// It had been a `suggestion: String` the loader set to `String::new()`
    /// behind a `// TODO`, so all 224 published pages printed an empty
    /// `## How to Fix`, and a sibling `expected_message` did the same for a
    /// `**Error**:` line on 219 of them. Neither field has any source: the
    /// format has no expected-message and no suggestion, and never has.
    ///
    /// Meanwhile the text was in the same files. The since-deleted second
    /// parser of `spec/errors/*.md` read `## CHAT Rule` and threw it away into
    /// a `let _chat_rule` binding. Present, parsed, discarded.
    ///
    /// What hid it: the two tests over the renderer built `ErrorDefinition` by
    /// hand and filled in a `suggestion` production never produced, so they
    /// could only ever see a page that does not exist. A model is only as
    /// strong as its weakest constructor.
    ///
    /// `markdown.rs` points here rather than restating this; it was told three
    /// times with the numbers drifting between the copies, which is the same
    /// defect one layer up.
    pub chat_rule: Option<String>,
    /// Bad examples that trigger this error
    pub examples: Vec<ErrorExample>,
}

/// A bad example that triggers an error
#[derive(Debug)]
pub struct ErrorExample {
    /// The input that triggers the error
    pub input: String,
    /// Where in a transcript this example's fault occurs.
    pub level: SpecLevel,
    /// What this example asserts: `violates`, `legal`, or `subsumed_by`.
    ///
    /// REQUIRED by the schema, so an example that claims nothing is
    /// unwritable; the history of the predecessor field (an optional
    /// free-text code list whose absence meant inheritance) is on
    /// [`Claim`](talkbank_spec_vocabulary::frontmatter::Claim).
    pub claim: talkbank_spec_vocabulary::frontmatter::Claim,
    /// The fixture path the example was taken from, as its `**Source**` line
    /// gives it, when it has one.
    ///
    /// Its STEM is the transcript's name, which decides whether rules about
    /// the file's own name run (E531). An example with no `**Source**` is
    /// genuinely anonymous.
    pub source: Option<String>,
}

impl ErrorSpec {
    /// Load all error specifications from a directory
    pub fn load_all(root: impl AsRef<Path>) -> Result<Vec<Self>, String> {
        let root = root.as_ref();
        let mut specs = Vec::new();
        let mut issues = Vec::new();

        // ONE owner for "which files are specs", shared with the sibling parser
        // of these same files; see `metadata::spec_file_paths` for what the two
        // private rules were and why this one survived.
        let paths = super::metadata::spec_file_paths(root)?;

        for path in &paths {
            let loaded = std::fs::read_to_string(path)
                .map_err(|err| format!("failed to read: {err}"))
                .and_then(|content| Self::from_frontmatter(path, &content));
            match loaded {
                Ok(spec) => specs.push(spec),
                Err(err) => issues.push(format!("Failed to load {}: {}", path.display(), err)),
            }
        }

        // A load failure (missing/invalid Kind, malformed metadata, a WalkDir
        // error) must actually fail the whole load: this used to be collected
        // into `issues` and then silently discarded (the `println!` below was
        // commented out), which meant a spec file that failed to parse was
        // just dropped from the result with NO signal to the caller. Every
        // caller of `load_all` already propagates a `Result` with `?`, so
        // surfacing failures here costs nothing and closes that hole. This is
        // also what makes `ErrorMetadata::kind` genuinely REQUIRED rather
        // than "required unless the loader swallows the error."
        if issues.is_empty() {
            Ok(specs)
        } else {
            Err(issues.join("\n"))
        }
    }

    /// Load a spec from its `+++` TOML frontmatter and its prose body.
    ///
    /// # What this does NOT do, and why that is the whole point
    ///
    /// It does not decide anything. The bullet loader had to: which of three
    /// routes names the code, which of three separators splits the heading,
    /// whether a field before a fence belongs to the example below it, and
    /// what an unrecognised label means. Every one of those was a policy with
    /// tests, and every one is now a declared field that serde either reads or
    /// refuses.
    ///
    /// The prose sections are still read from the body by [`section_source`],
    /// because they ARE markdown and are republished as markdown.
    ///
    /// # Errors
    ///
    /// When the frontmatter is absent, malformed, violates the schema, or the
    /// body carries no `## Description`.
    pub fn from_frontmatter(path: impl AsRef<Path>, content: &str) -> Result<Self, String> {
        let path = path.as_ref();
        let (front, body): (
            talkbank_spec_vocabulary::frontmatter::SpecFrontmatter<ErrorKind>,
            &str,
        ) = talkbank_spec_vocabulary::frontmatter::read(content).map_err(|why| why.to_string())?;

        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, body, &comrak::Options::default());

        let description: SpecDescription = match section_source(body, root, "Description")
            .as_deref()
            .map(str::parse::<SpecDescription>)
        {
            Some(Ok(parsed)) => parsed,
            Some(Err(_)) | None => {
                return Err("no `## Description` section, or an empty one. Every spec \
                     must describe what it rejects."
                    .to_string());
            }
        };
        let chat_rule =
            section_source(body, root, "CHAT Rule").filter(|text| !text.trim().is_empty());

        let examples = front
            .examples
            .into_iter()
            .map(|example| ErrorExample {
                input: example.chat.into(),
                level: example.level,
                claim: example.claim,
                source: example.source,
            })
            .collect();

        Ok(ErrorSpec {
            metadata: ErrorMetadata {
                description,
                status: front.status,
                kind: front.kind,
            },
            error: ErrorDefinition {
                code: front.code,
                name: front.name,
                chat_rule,
                examples,
            },
            source_path: path.to_path_buf(),
        })
    }
}

impl ErrorExample {
    /// The transcript's name, taken from the `**Source**` line's stem.
    ///
    /// `None` when the example declares no source, which is the honest answer:
    /// an example that names no file has no file name, and rules about the
    /// file's name do not apply to it.
    pub fn source_stem(&self) -> Option<&str> {
        self.source
            .as_deref()
            .and_then(|source| source.rsplit('/').next())
            .map(|file| file.strip_suffix(".cha").unwrap_or(file))
            .filter(|stem| !stem.is_empty())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    /// Load one spec from frontmatter written to a temporary directory.
    ///
    /// `from_frontmatter` takes a path AND the text, because a spec IS a file
    /// and the path is provenance the manifest records. These tests write one
    /// rather than adding a second constructor production has no use for.
    fn load_from_source(file_name: &str, source: &str) -> Result<ErrorSpec, String> {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(file_name);
        std::fs::write(&path, source).expect("write spec");
        // The path is provenance the loader records; the TEXT is the input.
        // Reading the bytes back to hand them to the loader proved nothing
        // about the loader, only about the filesystem.
        ErrorSpec::from_frontmatter(&path, source)
    }

    /// A minimal but valid error spec.
    ///
    /// # Why this no longer has a hole where the status bullet went
    ///
    /// It did, so that one body could serve the absent case that
    /// `status_is_required` asserted was refused. That test is gone and the
    /// hole with it: `status` is a required field of a `deny_unknown_fields`
    /// struct, so a spec omitting it does not deserialize at all, and the
    /// refusal is the schema's, tested where the schema is. A fixture shaped
    /// around a test that no longer needs to exist is a fixture with a hole in
    /// it for no reason.
    fn spec_source(status: &str) -> String {
        format!(
            "+++
code = 'E999'
name = 'Test error'
kind = 'Invalidity'
status = '{status}'

[[example]]
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@End
'''
+++

## Description

A test error description.
"
        )
    }

    /// The declared status reaches the model.
    ///
    /// Wiring, not parsing. That a status is one of a closed set is
    /// [`Status`]'s property and that the field is required is the schema's;
    /// what is checked here is that this loader puts the value where the
    /// generators read it.
    #[test]
    fn the_declared_status_reaches_the_model() {
        let spec = load_from_source("E999_test.md", &spec_source("not_implemented"))
            .expect("spec should parse");
        assert_eq!(spec.metadata.status, Status::NotImplemented);
    }

    /// A loaded spec retains the path it came from, so the generator can record
    /// each fixture's `source_spec` provenance in the manifest. Without it the
    /// manifest could not point a failing fixture back at its spec.
    #[test]
    fn spec_retains_its_source_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("E999_test.md");
        let source = spec_source("implemented");
        std::fs::write(&path, &source).expect("write");
        let spec = ErrorSpec::from_frontmatter(&path, &source).expect("spec should parse");
        assert_eq!(spec.source_path, path);
        assert_eq!(spec.source_file(), "E999_test.md");
    }

    /// The example's input reaches the model without its closing newline.
    ///
    /// The block convention is the schema's and is applied by its
    /// deserializer; this pins that the loader takes the value as given rather
    /// than applying a second copy of the rule, which is what it did for the
    /// first hour of this format's existence.
    #[test]
    fn the_example_input_arrives_without_the_closing_newline() {
        let spec =
            load_from_source("E999_test.md", &spec_source("implemented")).expect("should parse");
        assert_eq!(spec.error.examples[0].input, "@UTF8\n@Begin\n@End");
    }

    /// The prose sections still come from the BODY, and are still required.
    ///
    /// The description is markdown republished as markdown, so it does not
    /// belong in a TOML string; the split between declared fields and prose is
    /// the point of the format. Behaviour no type here holds, because the
    /// section is found by reading the document.
    #[test]
    fn a_body_without_a_description_is_refused() {
        let source = spec_source("implemented").replace("## Description", "## Notes");
        let why = load_from_source("E999_test.md", &source)
            .expect_err("a spec with no Description must be refused");
        assert!(why.contains("Description"), "{why}");
    }
}
