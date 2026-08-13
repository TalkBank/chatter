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
use std::path::PathBuf;
use talkbank_model::model::FileStem;
use talkbank_model::model::TranscriptName;

use generators::spec::error::{ErrorExample, ErrorSpec};
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

/// A spec's implementation status.
///
/// Was a bare `&str` compared against two literals at the point of use, which
/// is a closed set wearing a string: a spec whose status is misspelled read as
/// "implemented" and was silently checked as though it claimed to work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecStatus {
    Implemented,
    NotImplemented,
    Deprecated,
    /// The rule is implemented and does fire, but no `.cha` input can reach it,
    /// so the spec carries no example and owes a named out-of-corpus test
    /// instead. `E768_media_filename_not_representable.md` is why the status
    /// exists: it first shipped as `implemented` with no example, which made
    /// the loader drop it silently. Enumerating it here was found necessary the
    /// moment `Unrecognised` stopped being silent.
    UnreachableFromChat,
    /// Anything else, kept verbatim so the report can NAME it. Reported as a
    /// failure: `is_skipped` answers false, so a misspelling was otherwise
    /// checked as though the spec claimed to be implemented.
    Unrecognised(String),
}

impl SpecStatus {
    fn parse(raw: &str) -> Self {
        match raw.trim() {
            "implemented" => Self::Implemented,
            "not_implemented" => Self::NotImplemented,
            "deprecated" => Self::Deprecated,
            "unreachable_from_chat" => Self::UnreachableFromChat,
            other => Self::Unrecognised(other.to_owned()),
        }
    }

    fn is_skipped(&self) -> bool {
        matches!(
            self,
            Self::NotImplemented | Self::Deprecated | Self::UnreachableFromChat
        )
    }
}

/// One run's inputs.
pub struct Request {
    pub spec_dir: PathBuf,
    pub code_check: CodeCheck,
    pub skipped: SkippedSpecs,
    pub filter: CodeFilter,
}

impl Default for Request {
    /// What the CI gate means: verify every code, in every non-deferred spec,
    /// in this repository's own spec directory.
    fn default() -> Self {
        Self {
            spec_dir: default_spec_dir(),
            code_check: CodeCheck::Verify,
            skipped: SkippedSpecs::Omit,
            filter: CodeFilter::All,
        }
    }
}

/// Why an example was not checked.
///
/// A closed set, not the `String` it was: `run` discarded the reason entirely,
/// so "skipped: 73" could not distinguish a spec deliberately deferred from one
/// that asserts nothing at all. Those want opposite follow-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Marked not_implemented, deprecated, or unreachable from CHAT.
    Deferred,
    /// Declares no Expected Error Codes, so there is nothing to check.
    NoExpectedCodes,
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
    /// Not run.
    Skipped(SkipReason),
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
/// begins with a FILE NAME (`E531_media_no_timing.md`), so a structural loading
/// fault in any spec whose filename starts with an exempted code was silently
/// swallowed AND kept the stale-exemption check satisfied. Both directions of a
/// both-directions check, corrupted by one prefix match. [`Self::code`] answers
/// `None` for findings about a FILE, so nothing can excuse them.
pub enum Failure {
    NoErrorDefinitions {
        source_file: String,
    },
    UnrecognisedStatus {
        source_file: String,
        status: String,
    },
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
    /// The declared error code this finding is about, when it has one.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::NoErrorDefinitions { .. } | Self::UnrecognisedStatus { .. } => None,
            Self::CodeMismatch { label, .. } | Self::Panicked { label, .. } => Some(&label.code),
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoErrorDefinitions { source_file } => {
                write!(f, "{source_file}: no error definitions")
            }
            Self::UnrecognisedStatus {
                source_file,
                status,
            } => write!(
                f,
                "{source_file}: unrecognised Status {status:?}, so it was checked as implemented"
            ),
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
    pub no_expected_codes: u32,
    pub failures: Vec<Failure>,
}

impl Report {
    pub fn total(&self) -> usize {
        self.verified as usize
            + self.parsed as usize
            + self.deferred as usize
            + self.no_expected_codes as usize
            + self.failures.len()
    }

    pub fn summary(&self) -> String {
        format!(
            "{} verified, {} parsed-only, {} deferred, {} without expected codes, \
             {} failing (of {} examples)",
            self.verified,
            self.parsed,
            self.deferred,
            self.no_expected_codes,
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
            .filter(|failure| !failure.code().is_some_and(|code| exempt.contains(code)))
            .map(ToString::to_string)
            .collect();

        let stale: Vec<&str> = exempt
            .iter()
            .copied()
            .filter(|code| {
                !self
                    .failures
                    .iter()
                    .any(|failure| failure.code() == Some(*code))
            })
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
    let specs = ErrorSpec::load_all(&request.spec_dir)
        .map_err(|err| format!("failed to load specs from {:?}: {err}", request.spec_dir))?;

    if specs.is_empty() {
        return Err(format!(
            "no specs found in {:?}. This was a warning and an `Ok(())`, so a \
             mistyped path reported every spec valid.",
            request.spec_dir
        ));
    }

    let parser =
        TreeSitterParser::new().map_err(|err| format!("failed to create parser: {err}"))?;

    let mut report = Report {
        verified: 0,
        parsed: 0,
        deferred: 0,
        no_expected_codes: 0,
        failures: Vec::new(),
    };

    for spec in &specs {
        let Some(first) = spec.errors.first() else {
            report.failures.push(Failure::NoErrorDefinitions {
                source_file: spec.source_file.clone(),
            });
            continue;
        };
        if !request.filter.covers(first.code.as_str()) {
            continue;
        }
        let status = SpecStatus::parse(&spec.metadata.status);
        if let SpecStatus::Unrecognised(raw) = &status {
            report.failures.push(Failure::UnrecognisedStatus {
                source_file: spec.source_file.clone(),
                status: raw.clone(),
            });
        }

        for (def_idx, error_def) in spec.errors.iter().enumerate() {
            for (example_idx, example) in error_def.examples.iter().enumerate() {
                match check_example(&parser, &status, example, request) {
                    ExampleOutcome::Verified => report.verified += 1,
                    ExampleOutcome::Parsed => report.parsed += 1,
                    ExampleOutcome::Skipped(SkipReason::Deferred) => report.deferred += 1,
                    ExampleOutcome::Skipped(SkipReason::NoExpectedCodes) => {
                        report.no_expected_codes += 1;
                    }
                    // The label is built ONLY here, on the two failure paths. It
                    // was built for all 330 examples and read by the two, so a
                    // clean run allocated 328 strings it dropped unread.
                    ExampleOutcome::CodeMismatch { expected, actual } => {
                        report.failures.push(Failure::CodeMismatch {
                            label: label_for(spec, error_def.examples.len(), def_idx, example_idx),
                            expected,
                            actual,
                        });
                    }
                    ExampleOutcome::Panicked { message } => {
                        report.failures.push(Failure::Panicked {
                            label: label_for(spec, error_def.examples.len(), def_idx, example_idx),
                            message,
                        });
                    }
                }
            }
        }
    }

    Ok(report)
}

fn label_for(
    spec: &ErrorSpec,
    examples: usize,
    def_idx: usize,
    example_idx: usize,
) -> ExampleLabel {
    let code = spec
        .errors
        .get(def_idx)
        .map_or("<unknown>", |def| def.code.as_str())
        .to_owned();
    let position = if examples > 1 {
        Some(format!("example {}", example_idx + 1))
    } else if spec.errors.len() > 1 {
        Some(format!("def {}", def_idx + 1))
    } else {
        None
    };
    ExampleLabel { code, position }
}

/// Parse and validate one example, returning every diagnostic it emits.
///
/// Public and factored out so that tools which LIST what an example emits use
/// the same code path as the gate that CHECKS it. A second implementation of
/// "run an example" would be a second thing to drift, and the whole point of
/// listing is to decide what the example should assert.
pub fn emit_for(
    parser: &TreeSitterParser,
    example: &ErrorExample,
) -> Vec<talkbank_model::ParseError> {
    let sink = ErrorCollector::new();
    let mut chat_file = parser.parse_chat_file_streaming(&example.input, &sink);
    // A spec example is text, not a file, but some rules are ABOUT the file's
    // name: E531 compares the `@Media` filename against the transcript's own
    // stem. The name comes from the example's own `**Source**` line; an example
    // with no source is `Anonymous`, which is the honest answer rather than a
    // synthetic stem that would make such rules fire by construction.
    let name = example
        .source_stem()
        .map_or(TranscriptName::Anonymous, |stem| {
            TranscriptName::Named(FileStem::from_str(stem))
        });
    chat_file.validate_with_alignment(&sink, name);
    sink.into_vec()
}

/// Run one example through parse + validate.
fn check_example(
    parser: &TreeSitterParser,
    status: &SpecStatus,
    example: &ErrorExample,
    request: &Request,
) -> ExampleOutcome {
    if request.skipped == SkippedSpecs::Omit && status.is_skipped() {
        return ExampleOutcome::Skipped(SkipReason::Deferred);
    }
    if request.code_check == CodeCheck::Verify && example.expected_codes.is_empty() {
        return ExampleOutcome::Skipped(SkipReason::NoExpectedCodes);
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

    let mut actual: Vec<String> = emitted
        .iter()
        .map(|err| err.code.as_str().to_owned())
        .collect();
    actual.sort();
    actual.dedup();

    if example
        .expected_codes
        .iter()
        .all(|expected| actual.iter().any(|got| got == expected))
    {
        ExampleOutcome::Verified
    } else {
        ExampleOutcome::CodeMismatch {
            expected: example.expected_codes.clone(),
            actual,
        }
    }
}

/// The spec directory.
///
/// Derived from the spec workspace's single root resolver rather than a second
/// `..`-chain of its own. Adding one here was the exact form that
/// `talkbank-parser-tests/src/repo_paths.rs`, written the same day, argues
/// breaks silently on a rename.
pub fn default_spec_dir() -> PathBuf {
    generators::node_coverage::repo_root()
        .join("spec")
        .join("errors")
}
