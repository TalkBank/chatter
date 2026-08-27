//! # Error Specification Types
//!
//! Structured representation of the error spec files in `spec/errors/`.
//!
//! Each Markdown file defines one error code with its metadata (kind,
//! category, layer), a human-readable description, and one or more bad-input
//! examples that should trigger the error. Generators consume these types to
//! emit Rust validation tests and error documentation pages.

use std::path::Path;

use super::comrak_text::section_source;
use super::metadata::{SpecDescription, SpecErrorCode, SpecLevel, Status};

/// The per-code facts a SPEC needs: which axis it reports on, and whether the
/// validator runs it.
///
/// `Copy`, and deliberately a subset of [`CodeEntry`]. A spec never reads the
/// code's variant name or rustdoc back off itself; only the two generators do,
/// and they hold the registry directly. Carrying the whole entry meant cloning
/// three `String`s per spec file to reach two bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CodeFacts {
    kind: ErrorKind,
    status: Status,
}

impl CodeFacts {
    /// Take the two facts a spec needs from a registry entry.
    ///
    /// Private, and takes a `&CodeEntry`, so the only way to reach one is to
    /// have resolved a code against the registry.
    fn of(entry: &CodeEntry) -> Self {
        Self {
            kind: entry.kind(),
            status: entry.status(),
        }
    }
}
use crate::repo_paths::RepoRoot;
use talkbank_spec_vocabulary::registry::{CodeEntry, CodeRegistry};

/// The four `DiagnosticKind` axis values a code can carry.
///
/// Re-exported, not defined: it moved to the vocabulary crate with `kind`
/// itself under R1, since a `kind` is a fact about a CODE and the registry is
/// where codes live.
///
/// The re-export justified itself as keeping the name working "for the
/// generators that name it", and after R1 no generator does: they read
/// `CodeEntry::kind` off the registry. What it still serves is this module's
/// own [`ErrorSpec::kind`] signature, which is a fair reason on its own.
pub use talkbank_spec_vocabulary::registry::ErrorKind;

/// Root structure for an error specification file.
///
/// Loaded from a single `spec/errors/E###_*.md` Markdown file: the metadata its
/// `## Metadata` block declares, and the one error it defines.
///
/// # There is exactly one way to build one, and it is [`Self::load_all`]
///
/// This derived `Deserialize` until 2026-08-18, along with the since-deleted
/// `ErrorMetadata`, `ErrorDefinition` and `ErrorExample`, and nothing in the workspace ever
/// deserialized any of them: there is no JSON or TOML route into a spec, only
/// the markdown loader below. What the derive actually did was make the
/// `#[serde(default)]` attributes read as a documented schema, so five fields
/// no spec file can declare looked like an optional part of the format rather
/// than what they were, which is dead weight the loader filled with empty
/// values. Removing it is what let those fields be deleted.
#[derive(Debug)]
pub struct ErrorSpec {
    /// This document's `## Description` section.
    ///
    /// Was `metadata: ErrorMetadata` until R1's review. Once `kind` and
    /// `status` moved to the registry, `ErrorMetadata` wrapped one already
    /// validated newtype and had one reader, so it was a level of nesting
    /// standing for nothing. `level` is a fact about each EXAMPLE and lives
    /// there.
    pub description: SpecDescription,
    /// The code's own facts, resolved from the registry at load.
    ///
    /// PRIVATE and `Copy`, reached only through [`Self::kind`] and
    /// [`Self::status`]. Their presence is the proof that this file names a
    /// code that exists: [`Self::from_frontmatter`] is the only constructor
    /// and it resolves, so the `spec/errors <-> ErrorCode` divergence check
    /// the DiagnosticKind generator used to run has nothing left to find.
    ///
    /// This was a cloned `CodeEntry`, and cloning the whole record deep-copied
    /// three `String`s per spec file (236 of them) to reach two one-byte
    /// enums; nothing ever read the code, variant or rustdoc back off a spec.
    /// The code STRING stays on [`ErrorDefinition::code`], which is what the
    /// document declares.
    code_facts: CodeFacts,
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
    /// Every error spec in THIS checkout, with the registry it resolves
    /// against, both derived from one root.
    ///
    /// The default route. Eight of the eleven `load_all` call sites derived
    /// both from one root and spelled `CodeRegistry::load(root)` themselves,
    /// which is a relationship held by each caller's discipline; the sibling
    /// loader in the other workspace had already made this move and this one
    /// had not. The two-argument form below stays for the two callers that
    /// genuinely read a FOREIGN tree (`--spec-dir`), where the mismatch is the
    /// point.
    ///
    /// # Errors
    ///
    /// When the registry or any spec fails to load.
    pub fn load_for_repo(root: &RepoRoot) -> Result<Vec<Self>, String> {
        let registry = root.code_registry().map_err(|why| why.to_string())?;
        Self::load_all(crate::artifacts::error_dir(root.as_path()), &registry)
    }

    pub fn load_all(root: impl AsRef<Path>, registry: &CodeRegistry) -> Result<Vec<Self>, String> {
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
                .and_then(|content| Self::from_frontmatter(path, &content, registry));
            match loaded {
                Ok(spec) => specs.push(spec),
                Err(err) => issues.push(format!("Failed to load {}: {}", path.display(), err)),
            }
        }

        // A load failure (an unregistered code, malformed metadata, a WalkDir
        // error) must actually fail the whole load: this used to be collected
        // into `issues` and then silently discarded (the `println!` below was
        // commented out), which meant a spec file that failed to parse was
        // just dropped from the result with NO signal to the caller. Every
        // caller of `load_all` already propagates a `Result` with `?`, so
        // surfacing failures here costs nothing and closes that hole. This is
        // also what makes the registry resolution genuinely BINDING rather
        // than "binding unless the loader swallows the error."
        if issues.is_empty() {
            Ok(specs)
        } else {
            Err(issues.join("\n"))
        }
    }

    /// What this diagnostic intrinsically IS.
    ///
    /// From the registry, not from this file. Every spec file for a code used
    /// to declare it, so eleven codes carried two or three copies and the
    /// DiagnosticKind generator had a loop whose only job was refusing to
    /// proceed when they disagreed. There is one copy now and no loop.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.code_facts.kind
    }

    /// Whether the validator actually enforces this code's rule.
    ///
    /// From the registry, for the same reason as [`Self::kind`]. Callers
    /// apply their own policy to it: the example runner skips `Deprecated`,
    /// the fixture corpus excuses `UnreachableFromChat`, and the generated
    /// enum asks [`Status::is_enforced`]. Three policies over one fact, which
    /// is why `Status` carries the vocabulary and not the verdict.
    #[must_use]
    pub fn status(&self) -> Status {
        self.code_facts.status
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
    pub fn from_frontmatter(
        path: impl AsRef<Path>,
        content: &str,
        registry: &CodeRegistry,
    ) -> Result<Self, String> {
        let path = path.as_ref();
        // ONE call, because the vocabulary crate owns the verb. Reading and
        // resolving were two steps here and two more in the other workspace,
        // with four error spellings between them, which is exactly why
        // `frontmatter::read` exists one step down.
        let (front, entry, body) =
            talkbank_spec_vocabulary::frontmatter::read_resolved(content, registry)
                .map_err(|why| why.to_string())?;
        let code_facts = CodeFacts::of(entry);

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
            description,
            code_facts,
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
        ErrorSpec::from_frontmatter(&path, source, &test_registry())
    }

    /// A registry declaring the one code these fixtures use.
    ///
    /// Since R1 a spec cannot load without resolving its code, which is the
    /// whole point: the corpus-wide divergence check became a per-file parse.
    fn test_registry() -> CodeRegistry {
        crate::test_registry::declaring(&[("E999", Status::NotImplemented)])
    }

    /// A minimal but valid error spec.
    ///
    /// # It declares no status and no kind, because a spec file cannot
    ///
    /// Both were fields here until R1 moved them to the registry. The fixture
    /// took a `status` parameter solely so one test could assert the declared
    /// value reached the model; there is no declared value now, and that test
    /// went with the field.
    fn spec_source() -> String {
        // No interpolation left: `kind` and `status` were the only values
        // this fixture varied, and a spec file no longer declares either.
        "+++
code = 'E999'
name = 'Test error'

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
        .to_owned()
    }

    /// The REGISTRY's status reaches the model, and the spec file has no say.
    ///
    /// This replaces `the_declared_status_reaches_the_model`, which asserted
    /// that a `status = ` line in the file reached `metadata.status`. There is
    /// no such line any more; a document does not get to state a fact about
    /// the code it describes.
    #[test]
    fn the_registrys_status_and_kind_reach_the_model() {
        let spec = load_from_source("E999_test.md", &spec_source()).expect("spec should parse");
        assert_eq!(spec.status(), Status::NotImplemented);
        assert_eq!(spec.kind(), ErrorKind::Invalidity);
    }

    /// A spec naming a code the registry does not declare is REFUSED.
    ///
    /// The corpus-wide `spec/errors <-> ErrorCode` divergence check, which the
    /// DiagnosticKind generator ran over every file at generation time, is
    /// this one line at the boundary. An orphaned spec file (W602's survived
    /// its variant's deletion for two weeks) now cannot load at all.
    #[test]
    fn a_spec_naming_an_unregistered_code_is_refused() {
        let source = spec_source().replace("E999", "E998");
        let why = load_from_source("E998_test.md", &source)
            .expect_err("an unregistered code must be refused");
        assert!(
            why.contains("E998"),
            "the message must name the code: {why}"
        );
    }

    /// A loaded spec retains the path it came from, so the generator can record
    /// each fixture's `source_spec` provenance in the manifest. Without it the
    /// manifest could not point a failing fixture back at its spec.
    #[test]
    fn spec_retains_its_source_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("E999_test.md");
        let source = spec_source();
        std::fs::write(&path, &source).expect("write");
        let spec =
            ErrorSpec::from_frontmatter(&path, &source, &test_registry()).expect("should parse");
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
        let spec = load_from_source("E999_test.md", &spec_source()).expect("should parse");
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
        let source = spec_source().replace("## Description", "## Notes");
        let why = load_from_source("E999_test.md", &source)
            .expect_err("a spec with no Description must be refused");
        assert!(why.contains("Description"), "{why}");
    }
}
