//! Check that every error spec's example actually produces the code it claims.
//!
//! # Why this is a library module
//!
//! It lived entirely inside `bin/validate_error_specs.rs`, so the only way to
//! run it was `cargo run`. CI runs `cargo test --manifest-path spec/Cargo.toml
//! --workspace`, which never invokes a `main`, and this tool is named as THE
//! validation step in ten documents across `spec/`. It had therefore asserted
//! nothing in CI for its entire existence.
//!
//! Running it revealed exactly one real disagreement out of 330 examples,
//! which is the argument for gates in one line: the discrepancy was neither
//! large nor hard to find, it was merely never looked at.
//!
//! # What changed on the way out of `main`
//!
//! The logic took `&Args`, a clap struct, which is what tied it to the binary.
//! It now takes [`Request`], and the two `bool` parameters that were passed
//! positionally and adjacently to `validate_example` (`check_codes`,
//! `include_skipped`, trivially swappable, and the compiler could not care)
//! are [`CodeCheck`] and [`SkippedSpecs`].
//!
//! Three smaller repairs, each the same shape as the missing gate:
//!
//! - `Ok(())` when NO specs were found. A validator that validates nothing
//!   reported success.
//! - `Err("Validation failed".to_string())`, a constant, after printing every
//!   mismatch to stderr. The detail was computed and then discarded from the
//!   value, so no caller could act on it. [`Report`] carries it.
//! - A panicking example was reported as a code mismatch whose "actual" list
//!   was `vec!["PANIC"]`, a fake error code inside a list of real ones.
//!   [`ExampleOutcome::Panicked`] is its own variant.
//!
//! And `ExampleOutcome::Pass` meant two different things: "the codes were
//! checked and matched" and "codes were not checked at all". Under
//! `CodeCheck::ParseOnly` every example returned `Pass`, so a summary reading
//! "256 passed" was counting examples nobody had verified.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use talkbank_model::model::FileStem;
use talkbank_model::model::TranscriptName;
use talkbank_model::model::WriteChat;
use talkbank_spec_vocabulary::registry::CodeRegistry;

use generators::repo_paths::RepoRoot;
use generators::spec::error::{ErrorExample, ErrorSpec};
use generators::spec::metadata::{SpecErrorCode, Status};
use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;

/// Whether a run verifies error codes, or only that examples do not crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeCheck {
    /// Compare each example's emitted codes against the ones it declares.
    Verify,
    /// Parse and validate, but assert nothing about which codes appeared.
    ParseOnly,
}

/// Whether specs marked `not_implemented` or `deprecated` take part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkippedSpecs {
    Include,
    Omit,
}

/// Which error codes a run covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeFilter {
    All,
    /// Only these codes. `Option<Vec<String>>` previously carried this, where
    /// `None` and `Some(vec![])` were different spellings of two DIFFERENT
    /// things (everything, and nothing) that no reader could distinguish.
    Only(BTreeSet<String>),
}

impl CodeFilter {
    fn covers(&self, code: &str) -> bool {
        match self {
            Self::All => true,
            Self::Only(codes) => codes.contains(code),
        }
    }
}

/// Whether this run skips a spec with the given status.
///
/// POLICY, deliberately kept out of [`Status`] itself: two other readers of the
/// same vocabulary make different calls (`Status::is_enforced`, which the
/// generated enum asks, treats only `NotImplemented` as unenforced; the re2c
/// parity gate measures everything except it). Three defensible judgements
/// about one fact, so the fact is shared and the judgements are not.
///
/// This replaced a private `SpecStatus` enum that re-parsed the metadata string
/// into a five-variant copy of the closed set. Its fifth variant existed to
/// report a value outside the set, which the loader now refuses outright.
fn is_skipped(status: Status) -> bool {
    matches!(
        status,
        Status::NotImplemented | Status::Deprecated | Status::UnreachableFromChat
    )
}

/// One run's inputs.
pub struct Request {
    /// Which specs to read, and the registry they resolve against.
    pub specs: SpecTree,
    pub code_check: CodeCheck,
    pub skipped: SkippedSpecs,
    pub filter: CodeFilter,
}

impl Request {
    /// What the CI gate means: verify every code, in every non-deferred spec,
    /// in this checkout's own spec directory.
    ///
    /// # Why this is not a `Default`
    ///
    /// It was one, and it resolved the repository root from the filesystem.
    /// `Default::default()` has no way to fail, so a checkout the resolver
    /// could not recognise had to abort the process; that panic was what kept
    /// `RepoRoot::from_manifest_dir` from returning a `Result`. Requiring an
    /// already-proved root moves the failure to the caller that can report it.
    /// # Errors
    ///
    /// When the per-code registry cannot be read or violates its own rules.
    /// Fallible since R1: an infallible constructor holding a file read has
    /// nowhere to report a bad registry but a panic, which is the shape this
    /// workspace bans by name.
    pub fn for_repo(root: &RepoRoot) -> Result<Self, String> {
        Ok(Self {
            specs: SpecTree::for_repo(root)?,
            code_check: CodeCheck::Verify,
            skipped: SkippedSpecs::Omit,
            filter: CodeFilter::All,
        })
    }
}

/// A directory of error specs, and the registry its files resolve against.
///
/// # Why this is one value and not two fields
///
/// `Request` carried `spec_dir` and `registry` as two `pub` fields under a
/// docstring claiming "holding both together is the only way to state that
/// they belong to each other". Two `pub` fields of a `pub struct` state
/// nothing: a literal can pair any directory with any registry, and the one
/// caller that is not [`Self::for_repo`] does exactly that ON PURPOSE. So the
/// sentence was false in the same commit that wrote it.
///
/// Both modes are real. Naming them as constructors makes the odd one visible
/// to a reader instead of inferable from a comment, and makes the claim true
/// where it is made.
#[derive(Debug)]
pub struct SpecTree {
    dir: PathBuf,
    registry: CodeRegistry,
}

impl SpecTree {
    /// Both halves from one proved checkout.
    ///
    /// # Errors
    ///
    /// When the registry cannot be read or violates its own rules.
    pub fn for_repo(root: &RepoRoot) -> Result<Self, String> {
        Ok(Self {
            dir: spec_dir(root),
            registry: root.code_registry().map_err(|why| why.to_string())?,
        })
    }

    /// Specs from a directory the operator named, against THIS checkout's
    /// registry.
    ///
    /// The `validate_error_specs --spec-dir` mode. The mismatch is deliberate:
    /// the registry is the vocabulary of codes, which is a property of the
    /// checkout, while the directory chooses which documents to read. A
    /// separate constructor so that intent is written down rather than
    /// reconstructed from a struct literal.
    ///
    /// # Errors
    ///
    /// When the registry cannot be read or violates its own rules.
    pub fn with_foreign_specs(dir: PathBuf, root: &RepoRoot) -> Result<Self, String> {
        Ok(Self {
            dir,
            registry: root.code_registry().map_err(|why| why.to_string())?,
        })
    }

    /// The directory the specs are read from.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The registry those specs resolve against.
    #[must_use]
    pub fn registry(&self) -> &CodeRegistry {
        &self.registry
    }
}

/// Which example a finding is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleLabel {
    /// The code the spec declares, as the spec wrote it: a spec may name a code
    /// the model does not have, and that is itself a finding.
    pub code: String,
    /// `example 2` / `def 1`, when a spec has more than one.
    pub position: Option<String>,
}

impl std::fmt::Display for ExampleLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.position {
            Some(position) => write!(f, "{} ({position})", self.code),
            None => f.write_str(&self.code),
        }
    }
}

/// What happened to one example.
pub enum ExampleOutcome {
    /// Codes were compared and every declared one appeared.
    Verified,
    /// Parsed and validated; codes deliberately not compared.
    Parsed,
    /// Not run: the spec is not_implemented, deprecated, or unreachable.
    ///
    /// This was `Skipped(SkipReason)` while there were two reasons; R2 made
    /// the other one ("declares nothing to check") unwritable, and a
    /// one-variant enum inside a variant was indirection carrying nothing.
    Deferred,
    /// Codes were compared and at least one declared code did not appear.
    CodeMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    /// The parser or validator panicked on this input.
    Panicked { message: String },
}

/// One disagreement, STRUCTURED.
///
/// These were pre-formatted `String`s built in the same expression that
/// consumed the `ExampleOutcome`, so the expected/actual lists were
/// unrecoverable: this module's doc argues against exactly that one level up
/// and then did it here.
///
/// Not merely inelegant. The gate matched its exemption list with
/// `line.starts_with(code)` over those strings, and a `NoErrorDefinitions` line
/// began with a FILE NAME (`E531_media_no_timing.md`), so a structural loading
/// fault in any spec whose filename starts with an exempted code was silently
/// swallowed AND kept the stale-exemption check satisfied. Both directions of a
/// both-directions check, corrupted by one prefix match.
///
/// That variant is GONE as of 2026-08-18, and with it the whole hazard: it
/// reported a spec whose definition list was empty, which stopped being
/// possible when the list stopped being a list. Every finding is now about a
/// CODE, so [`Self::code`] returns one rather than an `Option` nothing could
/// excuse.
pub enum Failure {
    CodeMismatch {
        label: ExampleLabel,
        expected: Vec<String>,
        actual: Vec<String>,
    },
    Panicked {
        label: ExampleLabel,
        message: String,
    },
}

impl Failure {
    /// The declared error code this finding is about.
    pub fn code(&self) -> &str {
        match self {
            Self::CodeMismatch { label, .. } | Self::Panicked { label, .. } => &label.code,
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodeMismatch {
                label,
                expected,
                actual,
            } => write!(f, "{label}: expected {expected:?}, got {actual:?}"),
            Self::Panicked { label, message } => write!(f, "{label}: PANICKED: {message}"),
        }
    }
}

/// Examples that cannot produce their declared code IN THIS HARNESS.
///
/// Not "known failures". Each entry is a case where the spec is right, the
/// validator is right, and the harness cannot express the precondition.
///
/// It lives HERE rather than in the gate because the limitation is a property
/// of [`check_example`], which validates in-memory strings with no path. Held
/// test-side, `cargo run` and CI applied different rules to the same corpus.
/// Currently EMPTY, and that is the healthy state: every declared code can now
/// be produced by the harness. The list is kept because the limitation it
/// describes is real and can recur, and because the gate checks it in both
/// directions: an entry that stops corresponding to a failure is reported as
/// stale and must be deleted, which is how the last one (E531, a `@Media`
/// filename-context case) left on 2026-08-11.
pub const HARNESS_CANNOT_TRIGGER: &[(&str, &str)] = &[];

/// One run's result, in full.
///
/// `total` is DERIVED rather than accumulated beside three other counters that
/// nothing forced to agree with it.
pub struct Report {
    pub verified: u32,
    pub parsed: u32,
    pub deferred: u32,
    pub failures: Vec<Failure>,
}

impl Report {
    pub fn total(&self) -> usize {
        self.verified as usize + self.parsed as usize + self.deferred as usize + self.failures.len()
    }

    pub fn summary(&self) -> String {
        format!(
            "{} verified, {} parsed-only, {} deferred, {} failing (of {} examples)",
            self.verified,
            self.parsed,
            self.deferred,
            self.failures.len(),
            self.total()
        )
    }

    /// The operator-facing result, with [`HARNESS_CANNOT_TRIGGER`] applied.
    ///
    /// ONE call, consumed by both the renderer and the gate, so the two cannot
    /// print different text for the same state. `is_clean()` beside `summary()`
    /// let each caller assemble its own, and they had already diverged.
    pub fn outcome(&self) -> Result<String, String> {
        let exempt: BTreeSet<&str> = HARNESS_CANNOT_TRIGGER
            .iter()
            .map(|(code, _)| *code)
            .collect();

        // Matched on the finding's OWN code, never a prefix of its rendered
        // text, so a file-level finding cannot be swallowed by a code exemption.
        let unexpected: Vec<String> = self
            .failures
            .iter()
            .filter(|failure| !exempt.contains(failure.code()))
            .map(ToString::to_string)
            .collect();

        let stale: Vec<&str> = exempt
            .iter()
            .copied()
            .filter(|code| !self.failures.iter().any(|failure| failure.code() == *code))
            .collect();

        if unexpected.is_empty() && stale.is_empty() {
            return Ok(self.summary());
        }

        let mut out = self.summary();
        if !unexpected.is_empty() {
            out.push_str(&format!(
                "\n\n{} spec example(s) do not emit their declared code:",
                unexpected.len()
            ));
            for line in &unexpected {
                out.push_str(&format!("\n  {line}"));
            }
        }
        if !stale.is_empty() {
            out.push_str(&format!(
                "\n\n{} exemption(s) in HARNESS_CANNOT_TRIGGER no longer correspond \
                 to a failure. Delete them:",
                stale.len()
            ));
            for code in &stale {
                out.push_str(&format!("\n  {code}"));
            }
        }
        Err(out)
    }
}

/// Load every spec under `request.spec_dir` and check its examples.
///
/// # Errors
///
/// When the specs cannot be loaded, when the directory holds none (a validator
/// that validates nothing must not report success), or when the parser cannot
/// be constructed.
pub fn run(request: &Request) -> Result<Report, String> {
    let specs = ErrorSpec::load_all(request.specs.dir(), request.specs.registry())
        .map_err(|err| format!("failed to load specs from {:?}: {err}", request.specs.dir()))?;

    if specs.is_empty() {
        return Err(format!(
            "no specs found in {:?}. This was a warning and an `Ok(())`, so a \
             mistyped path reported every spec valid.",
            request.specs.dir()
        ));
    }

    let parser =
        TreeSitterParser::new().map_err(|err| format!("failed to create parser: {err}"))?;

    let mut report = Report {
        verified: 0,
        parsed: 0,
        deferred: 0,
        failures: Vec::new(),
    };

    for spec in &specs {
        if !request.filter.covers(spec.error.code.as_str()) {
            continue;
        }
        let status = spec.status();

        for (example_idx, example) in spec.error.examples.iter().enumerate() {
            match check_example(&parser, status, &spec.error.code, example, request) {
                ExampleOutcome::Verified => report.verified += 1,
                ExampleOutcome::Parsed => report.parsed += 1,
                ExampleOutcome::Deferred => report.deferred += 1,
                // The label is built ONLY here, on the two failure paths. It
                // was built for all 330 examples and read by the two, so a
                // clean run allocated 328 strings it dropped unread.
                ExampleOutcome::CodeMismatch { expected, actual } => {
                    report.failures.push(Failure::CodeMismatch {
                        label: label_for(spec, example_idx),
                        expected,
                        actual,
                    });
                }
                ExampleOutcome::Panicked { message } => {
                    report.failures.push(Failure::Panicked {
                        label: label_for(spec, example_idx),
                        message,
                    });
                }
            }
        }
    }

    Ok(report)
}

/// Name one failing example, for a report a human reads.
///
/// This took a `def_idx` and looked the definition up with
/// `.map_or("<unknown>", ...)`, a fabricated code for a lookup that could not
/// miss, plus a `spec.errors.len() > 1` arm that could not be reached. Both
/// were artefacts of the definition list being a `Vec` that always held one.
fn label_for(spec: &ErrorSpec, example_idx: usize) -> ExampleLabel {
    let position = if spec.error.examples.len() > 1 {
        Some(format!("example {}", example_idx + 1))
    } else {
        None
    };
    ExampleLabel {
        code: spec.error.code.as_str().to_owned(),
        position,
    }
}

/// The diagnostics one example produced, kept apart by the STAGE that emitted
/// them.
///
/// # Why the split is in the type and not a field on the diagnostic
///
/// `ParseError` does not carry which stage produced it, and the only place the
/// fact is knowable is the seam between the two calls inside [`emit_for`].
/// Flattening both stages into one `Vec` is how that fact used to die: R4 of
/// the spec-system redesign derives a spec's layer-of-capture from "where the
/// diagnostic actually came", and the observation snapshot (R3) records it, so
/// the seam now returns what it knows instead of discarding it (preserve
/// information; a total function silently discarding what it learned is shape
/// C).
///
/// Two sinks rather than one drained twice: the [`talkbank_model::ErrorSink`]
/// trait is write-only, and parse taint travels in the `ChatFile` itself, so
/// the split is observationally identical to the single sink it replaces.
pub struct StagedDiagnostics {
    /// What the parser emitted while building the tree.
    pub parse: Vec<talkbank_model::ParseError>,
    /// What validation (with alignment) emitted over the parsed model.
    pub validation: Vec<talkbank_model::ParseError>,
    /// Whether serializing the parsed model reproduced the example's text.
    ///
    /// Measured HERE, in the one sanctioned example-running path, so the
    /// snapshot and every other consumer see the same answer. The example's
    /// text arrives without its closing newline (the spec loader strips it)
    /// while a serialized file always ends in one, so the comparison is
    /// against the example AS A FILE: one trailing newline appended when
    /// absent, and nothing else normalized.
    pub roundtrip: talkbank_spec_vocabulary::observations::Roundtrip,
}

impl StagedDiagnostics {
    /// Every diagnostic, both stages, in emission order.
    pub fn all(&self) -> impl Iterator<Item = &talkbank_model::ParseError> {
        self.parse.iter().chain(self.validation.iter())
    }

    /// The distinct codes across both stages, sorted.
    ///
    /// The snapshot's normalization ("a sorted, deduplicated SET per stage"),
    /// stated as policy in the observations module and then implemented three
    /// times by hand across this crate. One owner; the per-stage variant is
    /// [`distinct_codes`].
    #[must_use]
    pub fn all_distinct_codes(&self) -> Vec<String> {
        let mut codes: Vec<String> = self.all().map(|err| err.code.as_str().to_owned()).collect();
        codes.sort();
        codes.dedup();
        codes
    }
}

/// The distinct codes of ONE stage's diagnostics, sorted.
///
/// Free, because the snapshot builder normalizes each stage separately while
/// every other caller wants the union.
#[must_use]
pub fn distinct_codes(errors: &[talkbank_model::ParseError]) -> Vec<String> {
    let mut codes: Vec<String> = errors
        .iter()
        .map(|err| err.code.as_str().to_owned())
        .collect();
    codes.sort();
    codes.dedup();
    codes
}

/// Parse and validate one example, returning every diagnostic it emits.
///
/// Public and factored out so that tools which LIST what an example emits use
/// the same code path as the gate that CHECKS it. A second implementation of
/// "run an example" would be a second thing to drift, and the whole point of
/// listing is to decide what the example should assert.
pub fn emit_for(parser: &TreeSitterParser, example: &ErrorExample) -> StagedDiagnostics {
    let parse_sink = ErrorCollector::new();
    let mut chat_file = parser.parse_chat_file_streaming(&example.input, &parse_sink);
    // A spec example is text, not a file, but some rules are ABOUT the file's
    // name: E531 compares the `@Media` filename against the transcript's own
    // stem. The name comes from the example's own `**Source**` line; an example
    // with no source is `Anonymous`, which is the honest answer rather than a
    // synthetic stem that would make such rules fire by construction.
    let name = example
        .source_stem()
        .map_or(TranscriptName::Anonymous, |stem| {
            TranscriptName::Named(FileStem::from_stem(stem))
        });
    let validation_sink = ErrorCollector::new();
    chat_file.validate_with_alignment(&validation_sink, name);
    let as_file = if example.input.ends_with('\n') {
        example.input.clone()
    } else {
        format!("{}\n", example.input)
    };
    let roundtrip = if chat_file.to_chat_string() == as_file {
        talkbank_spec_vocabulary::observations::Roundtrip::ByteExact
    } else {
        talkbank_spec_vocabulary::observations::Roundtrip::Diverged
    };
    StagedDiagnostics {
        parse: parse_sink.into_vec(),
        validation: validation_sink.into_vec(),
        roundtrip,
    }
}

/// Run one example through parse + validate, judging its CLAIM.
///
/// # The negative halves are new capability
///
/// The pre-R2 check was subset-only ("every declared code appears"), so no
/// spec could assert that a code does NOT fire, and the book said so plainly.
/// `legal` asserts exactly that, and `subsumed_by` asserts its targets fire
/// AND the spec's own code does not, which is what makes a subsumption
/// verifiable rather than merely recorded.
fn check_example(
    parser: &TreeSitterParser,
    status: Status,
    own_code: &SpecErrorCode,
    example: &ErrorExample,
    request: &Request,
) -> ExampleOutcome {
    use talkbank_spec_vocabulary::frontmatter::Claim;

    if request.skipped == SkippedSpecs::Omit && is_skipped(status) {
        return ExampleOutcome::Deferred;
    }

    // A spec example that panics must be reported, not allowed to abort the
    // whole run; historically E245's lone stress marker reached a
    // `new_unchecked` and brought the tool down.
    let emitted = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        emit_for(parser, example)
    })) {
        Ok(errors) => errors,
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic")
                .to_owned();
            return ExampleOutcome::Panicked { message };
        }
    };

    if request.code_check == CodeCheck::ParseOnly {
        return ExampleOutcome::Parsed;
    }

    let actual = emitted.all_distinct_codes();
    let fired = |code: &SpecErrorCode| actual.iter().any(|got| got == code.as_str());

    // The MEANING lives on the claim (`Claim::satisfied_by`); only the
    // rendering of what was wanted is local.
    let satisfied = example.claim.satisfied_by(own_code, fired);
    let expected = match &example.claim {
        Claim::Violates => vec![own_code.to_string()],
        Claim::Legal => vec![format!("absent: {own_code}")],
        Claim::SubsumedBy(targets) => {
            let mut expected: Vec<String> = targets
                .as_slice()
                .iter()
                .map(SpecErrorCode::to_string)
                .collect();
            expected.push(format!("absent: {own_code}"));
            expected
        }
    };

    if satisfied {
        ExampleOutcome::Verified
    } else {
        // `expected` renders the CLAIM, absences included; `actual` is what
        // the pipeline emitted. They stay different types on purpose.
        ExampleOutcome::CodeMismatch { expected, actual }
    }
}

/// The spec directory of a given checkout.
///
/// Takes an already-proved [`RepoRoot`] rather than resolving one itself, so
/// the "which checkout" question is answered once per run, by whoever can
/// report the failure, instead of once per helper.
/// Takes `impl AsRef<Path>` rather than `&RepoRoot`, so the artifact registry
/// (whose shared `build` signature hands it a bare `&Path`) and the binaries
/// (which hold a proved `RepoRoot`, and it implements `AsRef<Path>`) reach the
/// same function. A second `spec_dir_of` door existed briefly, with an eight-line
/// doc justifying itself; one function with a permissive argument is the same
/// ownership with less to explain.
#[must_use]
pub fn spec_dir(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join("spec").join("errors")
}
