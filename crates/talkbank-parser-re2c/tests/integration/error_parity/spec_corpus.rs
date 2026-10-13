//! Load every authored example through the existing specification owner.
//!
//! A claim asserts presence or absence of particular codes, not whole-input
//! validity. Neither a legal claim nor a deferred code hides its example from
//! the one-way detection requirement.

use std::collections::BTreeSet;
use std::path::PathBuf;
use talkbank_model::model::{ChatFile, TranscriptName};
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_parser_tests::error_specs::{self, Status};
use talkbank_spec_vocabulary::frontmatter::{Claim, ExampleFrontmatter};

use super::model::{BackendReport, ExampleOrdinal, ParsingFindings, SpecLabel, ValidationFindings};

/// One authored example. The actual tree-sitter outcome issues its obligation.
pub(super) struct SpecCase {
    pub(super) label: SpecLabel,
    pub(super) input: SpecInput,
}

/// Carry source identity and rule selection alongside the original CHAT bytes.
pub(super) struct SpecInput {
    text: String,
    source: Option<PathBuf>,
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

    /// Capture parser findings before entering shared, named model validation.
    pub(super) fn measure_with(
        &self,
        lower: impl FnOnce(&str, &ErrorCollector) -> ChatFile,
    ) -> BackendReport {
        let parse_sink = ErrorCollector::new();
        let mut file = lower(&self.text, &parse_sink);
        let parsing = ParsingFindings::from_sink(parse_sink);
        let validation_sink = ErrorCollector::new();
        let name = self
            .source
            .as_deref()
            .map_or(TranscriptName::Anonymous, TranscriptName::for_path);
        file.validate_with_alignment_and_rules(self.rules.selection(), &validation_sink, name);
        BackendReport::new(parsing, ValidationFindings::from_sink(validation_sink))
    }
}

/// All examples, including legal claims and not-implemented specifications.
pub(super) struct SpecCorpus {
    pub(super) files_scanned: usize,
    pub(super) cases: Vec<SpecCase>,
    /// Informational file count only; these specifications are still measured.
    pub(super) not_implemented: usize,
    /// Informational example count only; these examples are still measured.
    pub(super) legal: usize,
}

/// Use the repository loader once. Empty and colliding measurements are refused.
pub(super) fn load_spec_corpus() -> Result<SpecCorpus, String> {
    let repo_root = talkbank_parser_tests::repo_paths::workspace_root();
    let specs = error_specs::load(repo_root)?;
    let mut corpus = SpecCorpus {
        files_scanned: specs.len(),
        cases: Vec::new(),
        not_implemented: 0,
        legal: 0,
    };
    let mut labels = BTreeSet::new();
    for spec in &specs {
        if spec.status() == Status::NotImplemented {
            corpus.not_implemented += 1;
        }
        for (index, example) in spec.examples().iter().enumerate() {
            let label = SpecLabel::new(&spec.filename, ExampleOrdinal::from_zero_based(index));
            if !labels.insert(label.to_string()) {
                return Err(format!("duplicate measured specification example: {label}"));
            }
            if matches!(example.claim, Claim::Legal) {
                corpus.legal += 1;
            }
            corpus.cases.push(SpecCase {
                label,
                input: SpecInput::from_example(example, spec.rules()),
            });
        }
    }
    match corpus.cases.first() {
        None => Err(format!(
            "no spec cases found under {}",
            error_specs::spec_dir(repo_root).display()
        )),
        Some(_) => Ok(corpus),
    }
}

/// Survives: source identity must reach filename-dependent validation.
#[test]
fn declared_source_controls_filename_validation() {
    enum FilenameExpectation {
        Matching,
        Mismatching,
    }
    let corpus = load_spec_corpus().unwrap();
    let mut case = corpus
        .cases
        .into_iter()
        .find(|case| case.label.to_string() == "E531.md#1")
        .unwrap();
    let declared = case.input.source.take().expect("E531 declares its source");
    let canonical = talkbank_parser::TreeSitterParser::new().unwrap();
    for (source, expected) in [
        (None, FilenameExpectation::Matching),
        (
            Some(PathBuf::from("different.cha")),
            FilenameExpectation::Matching,
        ),
        (Some(declared), FilenameExpectation::Mismatching),
    ] {
        case.input.source = source;
        let reports = [
            case.input
                .measure_with(|text, errors| canonical.parse_chat_file_streaming(text, errors)),
            case.input
                .measure_with(talkbank_parser_re2c::parser::parse_chat_file_to_model),
        ];
        for reported in reports {
            let mismatch = reported.codes().contains(&ErrorCode::MediaFilenameMismatch);
            assert_eq!(
                mismatch,
                matches!(expected, FilenameExpectation::Mismatching),
                "source {:?}: {reported}",
                case.input.source
            );
        }
    }
}
