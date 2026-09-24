//! Real file scheduling must preserve every canonical diagnostic and completion.

use std::collections::BTreeMap;
use std::path::Path;
use talkbank_model::model::TranscriptName;
use talkbank_model::{ErrorCollector, ParseError, Severity};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, repo_paths::workspace_root};
use talkbank_transform::validation_runner::{
    CacheMode, CacheOutcome, FileStatus, RoundtripVerdict, RunCoverage, ValidationCache,
    ValidationConfig, ValidationEvent, validate_directory_streaming, validate_files_streaming,
};

// No cache instance can exist: this workflow must perform actual file work.
enum AbsentCache {}
impl ValidationCache for AbsentCache {
    fn get(&self, _: &Path, _: bool) -> Option<CacheOutcome> {
        match *self {}
    }
    fn set(&self, _: &Path, _: bool, _: CacheOutcome) -> Result<(), String> {
        match *self {}
    }
}

enum PendingFile {
    Diagnostics {
        source: String,
        errors: Vec<ParseError>,
    },
    Roundtrip,
    Completion {
        error_count: usize,
        roundtrip: RoundtripVerdict,
    },
}

impl PendingFile {
    fn after_diagnostics(error_count: usize, roundtrip_requested: bool) -> Self {
        if error_count == 0 && roundtrip_requested {
            Self::Roundtrip
        } else {
            Self::Completion {
                error_count,
                roundtrip: RoundtripVerdict::NotRequested,
            }
        }
    }
}

enum Workflow<'a> {
    FileValidation,
    DirectoryRoundtrip(&'a Path),
}

enum StreamPhase {
    Discovery,
    Start,
    Running,
    Finished,
}

/// Diagnostic text boundaries are distinct from CHAT validity. Canonical
/// transcript lines supply the report examples; no recovered model is repaired.
#[test]
fn canonical_roundtrip_diffs_report_only_actual_extra_differences() {
    use talkbank_transform::validation_runner::roundtrip::build_text_diff;
    let root = workspace_root().join("corpus/reference/annotation");
    let source =
        std::fs::read_to_string(root.join("errors-and-replacements.cha")).expect("reference");
    let other = std::fs::read_to_string(root.join("retrace.cha")).expect("reference");
    let pairs: Vec<_> = source
        .lines()
        .zip(other.lines())
        .filter(|(a, b)| a != b)
        .take(5)
        .collect();
    assert_eq!(
        pairs.len(),
        5,
        "canonical texts supply five differing report lines"
    );
    let left = pairs
        .iter()
        .map(|(a, _)| *a)
        .chain(["@End"])
        .collect::<Vec<_>>()
        .join("\n");
    let right = pairs
        .iter()
        .map(|(_, b)| *b)
        .chain(["@End"])
        .collect::<Vec<_>>()
        .join("\n");
    let report = build_text_diff(&left, &right);
    assert_eq!(report.matches("  line ").count(), 5);
    assert!(
        !report.contains("and more"),
        "equal trailing lines are not omitted differences"
    );
    let extended = format!("{right}\n@Comment:\textra report line");
    assert!(build_text_diff(&left, &extended).contains("and more"));

    let prefix = source
        .strip_suffix("@End\n")
        .expect("reference closing header");
    let line = source.lines().count();
    assert_eq!(
        build_text_diff(&source, prefix),
        format!("  line {line}:\n    pass1: \"@End\"\n    pass2: <missing>")
    );
    assert_eq!(
        build_text_diff(prefix, &source),
        format!("  line {line}:\n    pass1: <missing>\n    pass2: \"@End\"")
    );
    assert_eq!(
        build_text_diff(&source, &source),
        "no text differences found (possible trailing newline difference)"
    );
    // Literal diagnostic text is not an absent line. This tests the report
    // API's external string boundary, not a fabricated CHAT model.
    assert_eq!(
        build_text_diff("<missing>", ""),
        "  line 1:\n    pass1: \"<missing>\"\n    pass2: <missing>"
    );
}

#[test]
fn canonical_files_keep_diagnostics_and_complete_exactly_once() {
    let reference = ChatCorpus::reference().expect("reference corpus");
    let specs = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical specs");
    check_run(&[reference, specs], Workflow::FileValidation);
}

#[test]
fn discovered_reference_files_complete_requested_roundtrips() {
    let reference = ChatCorpus::reference().expect("reference corpus");
    let directory = workspace_root().join("corpus/reference");
    check_run(&[reference], Workflow::DirectoryRoundtrip(&directory));
}

fn check_run(corpora: &[ChatCorpus], workflow: Workflow<'_>) {
    let config = ValidationConfig {
        jobs: Some(2),
        cache: CacheMode::Disabled,
        roundtrip: matches!(workflow, Workflow::DirectoryRoundtrip(_)),
        ..Default::default()
    };
    let parser = TreeSitterParser::new().expect("parser");
    let mut pending = BTreeMap::new();
    let mut expected_invalid = 0;
    for fixture in corpora.iter().flat_map(ChatCorpus::fixtures) {
        let sink = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(fixture.source(), &sink);
        file.validate_with_alignment_and_rules(
            config.rules,
            &sink,
            TranscriptName::for_path(fixture.path()),
        );
        let errors = sink.into_vec();
        if errors.iter().any(|error| error.severity == Severity::Error) {
            expected_invalid += 1;
        }
        let state = if errors.is_empty() {
            PendingFile::after_diagnostics(0, config.roundtrip)
        } else {
            PendingFile::Diagnostics {
                source: fixture.source().to_owned(),
                errors,
            }
        };
        assert!(pending.insert(fixture.path().to_owned(), state).is_none());
    }
    let total = pending.len();
    if config.roundtrip {
        assert!(
            total > expected_invalid,
            "reference workflow must actually run roundtrips"
        );
    }
    let (events, _cancel) = match workflow {
        Workflow::FileValidation => validate_files_streaming::<AbsentCache>(
            pending.keys().cloned().collect(),
            &config,
            None,
        ),
        Workflow::DirectoryRoundtrip(directory) => {
            validate_directory_streaming::<AbsentCache>(directory, &config, None)
        }
    };
    let mut phase = StreamPhase::Discovery;
    for event in events {
        match event {
            ValidationEvent::Discovering => {
                assert!(matches!(phase, StreamPhase::Discovery));
                phase = StreamPhase::Start;
            }
            ValidationEvent::Started { total_files } => {
                assert!(matches!(phase, StreamPhase::Start));
                assert_eq!(total_files, total);
                phase = StreamPhase::Running;
            }
            ValidationEvent::Errors(event) => {
                assert!(matches!(phase, StreamPhase::Running));
                let Some(PendingFile::Diagnostics { source, errors }) = pending.remove(&event.path)
                else {
                    panic!(
                        "unexpected or duplicate diagnostic event: {}",
                        event.path.display()
                    );
                };
                assert_eq!(source.as_str(), event.source.as_ref());
                assert_eq!(errors, event.errors, "{}", event.path.display());
                let error_count = errors
                    .iter()
                    .filter(|error| error.severity == Severity::Error)
                    .count();
                pending.insert(
                    event.path,
                    PendingFile::after_diagnostics(error_count, config.roundtrip),
                );
            }
            ValidationEvent::FileComplete(event) => {
                assert!(matches!(phase, StreamPhase::Running));
                let Some(PendingFile::Completion {
                    error_count,
                    roundtrip: expected_roundtrip,
                }) = pending.remove(&event.path)
                else {
                    panic!(
                        "missing diagnostics or duplicate completion: {}",
                        event.path.display()
                    );
                };
                match event.status {
                    FileStatus::Valid {
                        cache_hit: false,
                        roundtrip,
                    } => {
                        assert_eq!(error_count, 0);
                        assert_eq!(roundtrip, expected_roundtrip);
                    }
                    FileStatus::Invalid {
                        error_count: actual,
                        cache_hit: false,
                    } => {
                        assert!(error_count > 0);
                        assert_eq!(actual, error_count);
                    }
                    other => panic!("unexpected file status: {} {other:?}", event.path.display()),
                }
            }
            ValidationEvent::Finished(stats) => {
                assert!(matches!(phase, StreamPhase::Running));
                assert!(
                    pending.is_empty(),
                    "terminal event must account for every file"
                );
                assert_eq!(stats.coverage(), RunCoverage::Complete);
                assert_eq!(stats.total_files, total);
                assert_eq!(stats.invalid_files, expected_invalid);
                assert_eq!(stats.valid_files, total - expected_invalid);
                assert_eq!(stats.cache_hits, 0);
                assert_eq!(stats.cache_misses, total);
                assert_eq!(stats.parse_errors, 0);
                assert_eq!(
                    stats.roundtrip_passed,
                    if config.roundtrip {
                        total - expected_invalid
                    } else {
                        0
                    }
                );
                assert_eq!(stats.roundtrip_failed, 0);
                assert!(!stats.cancelled);
                phase = StreamPhase::Finished;
            }
            ValidationEvent::RoundtripComplete(event) => {
                assert!(matches!(phase, StreamPhase::Running));
                assert!(
                    matches!(pending.remove(&event.path), Some(PendingFile::Roundtrip)),
                    "unexpected roundtrip: {}",
                    event.path.display()
                );
                assert!(
                    event.passed,
                    "reference serialization must be stable: {event:?}"
                );
                assert!(event.failure_reason.is_none() && event.diff.is_none());
                pending.insert(
                    event.path,
                    PendingFile::Completion {
                        error_count: 0,
                        roundtrip: RoundtripVerdict::Passed,
                    },
                );
            }
            ValidationEvent::FinishedIncomplete { stats, lost_files } => {
                panic!("lost {lost_files} files: {stats:?}")
            }
            ValidationEvent::Aborted(reason) => panic!("runner aborted: {reason}"),
        }
    }
    assert!(
        matches!(phase, StreamPhase::Finished),
        "channel closure alone is not completion"
    );
}
