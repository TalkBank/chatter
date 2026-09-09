//! Reading `spec/errors` into cases that can be measured.
//!
//! Split out of `error_parity.rs` when that file passed the workspace's 800
//! line hard limit. Everything here is about getting from markdown on disk to
//! a testable [`SpecCase`], including deciding which specs are in scope at all.

use std::collections::BTreeSet;

use std::path::PathBuf;
use talkbank_model::model::{ChatFile, TranscriptName};
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_parser_tests::error_specs::{self, Status};
use talkbank_spec_vocabulary::frontmatter::ExampleFrontmatter;

use super::model::{ExampleOrdinal, Expected, Reported, SpecLabel};

// ---------------------------------------------------------------------------
// Reading the spec suite
// ---------------------------------------------------------------------------

/// One example from a spec file: the input, and what it must produce.
pub(super) struct SpecCase {
    pub(super) label: SpecLabel,
    pub(super) input: SpecInput,
    pub(super) expected: Expected,
}

/// A spec's CHAT bytes and declared source travel through measurement together.
/// A filename-dependent rule must not become anonymous when loading the corpus.
pub(super) struct SpecInput {
    text: String,
    source: Option<PathBuf>,
    /// The rules this example's CODE runs under, carried here for the same
    /// reason its source is: a rule-dependent code must not become invisible
    /// when the corpus is loaded.
    ///
    /// Without it this harness validated every example under the DEFAULT rule
    /// set, so the eight opt-in linker codes reported nothing on BOTH
    /// backends and their parity cases agreed vacuously. Two parsers that
    /// both run no check agree about nothing.
    rules: talkbank_spec_vocabulary::frontmatter::RuleProfile,
}

impl SpecInput {
    fn from_example(
        example: &ExampleFrontmatter,
        rules: talkbank_spec_vocabulary::frontmatter::RuleProfile,
    ) -> Self {
        Self {
            text: example.chat.as_str().to_owned(),
            source: example.source.as_ref().map(PathBuf::from),
            rules,
        }
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn measure_with(
        &self,
        lower: impl FnOnce(&str, &ErrorCollector) -> ChatFile,
    ) -> Reported {
        let errors = ErrorCollector::new();
        let mut file = lower(&self.text, &errors);
        let name = self
            .source
            .as_deref()
            .map_or(TranscriptName::Anonymous, TranscriptName::for_path);
        file.validate_with_alignment_and_rules(self.rules.selection(), &errors, name);
        Reported::of(
            errors
                .into_vec()
                .into_iter()
                .map(|error| error.code)
                .collect(),
        )
    }
}

/// Everything `spec/errors` yielded, with every skip NAMED.
///
/// The old `unclassifiable` bucket ("chat blocks with no expectation") died
/// with R2: a claimless example is unwritable, so the state it caught cannot
/// recur, and the one claim that yields no positive expectation, `legal`, is
/// counted under its real name instead of debugging as data corruption.
pub(super) struct SpecCorpus {
    /// How many `.md` files the directory held, recorded at the point it is
    /// known rather than reconstructed downstream. The first cut of the summary
    /// line printed `cases + unclassifiable` under the word "file(s)", which is
    /// a count of one thing wearing the name of another.
    pub(super) files_scanned: usize,
    pub(super) cases: Vec<SpecCase>,
    /// How many files were skipped as `not_implemented`. A count, because a
    /// count is all anyone ever read: it had been a `Vec<SpecLabel>` whose only
    /// consumer was `.len()`, so 43 labels were built per run to produce a
    /// number, and `SpecLabel::whole_file` existed solely to build them.
    pub(super) not_implemented: usize,
    /// Examples claiming `legal`: their content is an ABSENCE, which this
    /// harness (a positive-codes comparison between two backends) cannot
    /// measure. The fixture runner enforces them; here they are counted so
    /// the denominator stays honest.
    pub(super) legal: usize,
}

/// What one example asserts, joined to the live `ErrorCode` enum.
///
/// The claim's positive codes come from the schema's one owner
/// (`Claim::positive_codes` via `effective_codes`); joining a well-formed
/// code to the live enum is a different question and stays here, because the
/// spec workspace cannot see that enum. `None` means the claim is `legal`,
/// whose content is an absence this positive-comparison harness cannot
/// measure; the caller counts it under that name.
fn expected_for(
    example: &talkbank_spec_vocabulary::frontmatter::ExampleFrontmatter,
    spec_code: &talkbank_spec_vocabulary::SpecErrorCode,
    filename: &str,
) -> Result<Option<Expected>, String> {
    let mut codes = BTreeSet::new();
    for spec_code in example.effective_codes(spec_code) {
        let code = ErrorCode::parse_exact(spec_code.as_str()).ok_or_else(|| {
            format!(
                "{filename}: the claim names {:?}, which is well formed but \
                 matches no declared ErrorCode variant",
                spec_code.as_str()
            )
        })?;
        codes.insert(code);
    }
    Ok(Expected::new(codes))
}

/// Whether a spec's examples are measured, over the SHARED
/// [`talkbank_parser_tests::error_specs::Status`] vocabulary, which is
/// `talkbank_spec_vocabulary::Status`, shared with the spec-side loader.
///
/// This module used to declare its own five-state copy of that enum, parsed by
/// its own line scan, which was one of four readers of the same markdown
/// format. The vocabulary is shared now; the POLICY below is not, because the
/// three readers legitimately disagree about it.
///
/// Only `NotImplemented` is skipped here. `spec/runtime-tools` also skips
/// `Deprecated` and `UnreachableFromChat`, so every spec in either of those
/// states is measured HERE and skipped THERE.
///
/// The set is deliberately not enumerated. It was, as three files, and two
/// status corrections on 2026-09-08 made it five without the comment moving,
/// which is the drift a list beside a live registry always has. The rule is
/// the durable statement; `rg 'status = "(deprecated|unreachable_from_chat)"'
/// over `spec/codes/error-codes.toml` is the membership.
///
/// Why the difference is left in place: a spec measured here that diverges
/// would appear in `KNOWN_DIVERGENCES`, and any such entry is adjudicated like
/// every other, so the wider scope adds evidence rather than noise.
fn measurable(status: Status) -> Measure {
    match status {
        Status::NotImplemented => Measure::Skip,
        Status::Implemented | Status::Deprecated | Status::UnreachableFromChat => Measure::Run,
    }
}

/// The decision itself, rather than a `bool` that the caller has to remember
/// reads "true means measure it". No bool crosses a function boundary in this
/// module, and this is the one that tried to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Measure {
    Run,
    Skip,
}

/// Load every spec case, in filename order so a run is reproducible.
pub(super) fn load_spec_corpus() -> Result<SpecCorpus, String> {
    let repo_root = talkbank_parser_tests::repo_paths::workspace_root();
    let specs = error_specs::load(repo_root)?;

    let mut corpus = SpecCorpus {
        files_scanned: specs.len(),
        cases: Vec::new(),
        not_implemented: 0,
        legal: 0,
    };

    for spec in &specs {
        let filename = spec.filename.clone();
        match measurable(spec.status()) {
            Measure::Skip => {
                corpus.not_implemented += 1;
                continue;
            }
            Measure::Run => {}
        }

        let examples = spec.examples();
        for (index, example) in examples.iter().enumerate() {
            let label = SpecLabel::new(&filename, ExampleOrdinal::from_zero_based(index));
            match expected_for(example, spec.declared_code(), &filename)? {
                Some(expected) => corpus.cases.push(SpecCase {
                    label,
                    input: SpecInput::from_example(example, spec.rules()),
                    expected,
                }),
                // A `legal` claim asserts an absence, which this harness
                // cannot measure; counted, never silently dropped.
                None => corpus.legal += 1,
            }
        }
    }

    match corpus.cases.first() {
        // An empty suite is a broken measurement, not a suite with nothing in
        // it, and the two read identically in every downstream count.
        None => Err(format!(
            "no spec cases found under {}",
            error_specs::spec_dir(repo_root).display()
        )),
        Some(_) => Ok(corpus),
    }
}

/// Exercise the real measurement seam: omitted or matching source names must
/// not fabricate E531, while the spec's declared mismatching name must enable it.
#[test]
fn declared_source_controls_filename_validation() {
    use super::model::Conformance;
    let corpus = load_spec_corpus().unwrap();
    let mut case = corpus
        .cases
        .into_iter()
        .find(|case| case.label.to_string() == "E531.md#1")
        .unwrap();
    let declared = case.input.source.take().expect("E531 declares its source");
    let canonical = talkbank_parser::TreeSitterParser::new().unwrap();
    for (source, expected) in [
        (None, Conformance::Misses),
        (Some(PathBuf::from("different.cha")), Conformance::Misses),
        (Some(declared), Conformance::Meets),
    ] {
        case.input.source = source;
        let reports = [
            case.input
                .measure_with(|text, errors| canonical.parse_chat_file_streaming(text, errors)),
            case.input
                .measure_with(talkbank_parser_re2c::parser::parse_chat_file_to_model),
        ];
        for reported in reports {
            let observed = Conformance::of(&case.expected, &reported);
            assert!(
                matches!(
                    (&expected, observed),
                    (Conformance::Meets, Conformance::Meets)
                        | (Conformance::Misses, Conformance::Misses)
                ),
                "source {:?}: {reported}",
                case.input.source
            );
        }
    }
}
