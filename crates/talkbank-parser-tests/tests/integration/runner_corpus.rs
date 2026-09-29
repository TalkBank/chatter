//! Real file scheduling must preserve every canonical diagnostic and completion.

#[path = "runner_lifecycle_corpus.rs"]
mod lifecycle_contracts;

#[path = "runner_cache_corpus.rs"]
mod cache_contracts;

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
fn canonical_file_sibling_survives_unreadable_input() {
    use std::io::Write;
    let good = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    let mut unreadable = tempfile::Builder::new()
        .prefix("invalid-encoding-")
        .suffix(".cha")
        .tempfile()
        .expect("isolated invalid-encoding input");
    unreadable
        .write_all(&std::fs::read(&good).expect("canonical bytes"))
        .expect("copy control");
    // This is an external file-encoding defect, not a CHAT edit or a
    // synthesized recovered model. It must fail before parsing.
    unreadable.write_all(&[0xff]).expect("invalid UTF-8 suffix");
    unreadable.flush().expect("flush input");
    let bad = unreadable.path().to_path_buf();
    let reservation = tempfile::Builder::new()
        .prefix("missing-transcript-")
        .suffix(".cha")
        .tempfile()
        .expect("reserve test-owned missing path");
    let missing = reservation.path().to_path_buf();
    reservation
        .close()
        .expect("remove only the test-owned empty reservation");
    assert!(!missing.exists());
    let config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Disabled,
        ..Default::default()
    };
    let (events, _cancel) = validate_files_streaming::<AbsentCache>(
        vec![missing.clone(), bad.clone(), good.clone()],
        &config,
        None,
    );
    let mut pending =
        std::collections::BTreeSet::from([missing.clone(), bad.clone(), good.clone()]);
    let mut finished = false;
    for event in events {
        assert!(!finished, "completion event must be terminal");
        match event {
            ValidationEvent::Discovering => {}
            ValidationEvent::Started { total_files } => assert_eq!(total_files, 3),
            ValidationEvent::FileComplete(event) => {
                assert!(
                    pending.remove(&event.path),
                    "duplicate or unexpected completion"
                );
                if event.path == bad || event.path == missing {
                    assert!(
                        matches!(event.status, FileStatus::ReadError { ref message } if !message.is_empty())
                    );
                } else {
                    assert!(matches!(
                        event.status,
                        FileStatus::Valid {
                            cache_hit: false,
                            roundtrip: RoundtripVerdict::NotRequested,
                        }
                    ));
                }
            }
            ValidationEvent::Finished(stats) => {
                assert!(pending.is_empty());
                assert_eq!(stats.coverage(), RunCoverage::Complete);
                assert_eq!(stats.total_files, 3);
                assert_eq!(stats.valid_files, 1);
                // Read failure is a failed file, not a parser diagnostic.
                assert_eq!(stats.parse_errors, 0);
                assert_eq!(stats.invalid_files, 2);
                assert_eq!(stats.cache_hits, 0);
                assert!(!stats.cancelled);
                finished = true;
            }
            other => panic!("unexpected file-read-failure event: {other:?}"),
        }
    }
    assert!(finished);
}

#[test]
fn canonical_file_cache_replays_only_completed_validation_and_roundtrips() {
    use std::sync::Arc;
    use talkbank_cache::CachePool;
    struct Step {
        roundtrip: bool,
        cache_hit: bool,
        verdict: RoundtripVerdict,
    }
    let mut config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        ..Default::default()
    };
    let cache =
        Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated SQLite cache"));
    let path = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    for step in [
        Step {
            roundtrip: false,
            cache_hit: false,
            verdict: RoundtripVerdict::NotRequested,
        },
        Step {
            roundtrip: true,
            cache_hit: false,
            verdict: RoundtripVerdict::Passed,
        },
        Step {
            roundtrip: true,
            cache_hit: true,
            verdict: RoundtripVerdict::Passed,
        },
        Step {
            roundtrip: false,
            cache_hit: true,
            verdict: RoundtripVerdict::NotRequested,
        },
    ] {
        config.roundtrip = step.roundtrip;
        let (events, _cancel) =
            validate_files_streaming(vec![path.clone()], &config, Some(cache.clone()));
        let mut completions = 0;
        let mut roundtrips = 0;
        let mut finished = false;
        for event in events {
            assert!(!finished, "Finished is terminal");
            match event {
                ValidationEvent::Discovering => {}
                ValidationEvent::Started { total_files } => assert_eq!(total_files, 1),
                ValidationEvent::Errors(errors) => {
                    panic!("valid reference diagnostics: {errors:?}")
                }
                ValidationEvent::FileComplete(event) => {
                    assert_eq!(event.path, path);
                    let FileStatus::Valid {
                        cache_hit,
                        roundtrip,
                    } = event.status
                    else {
                        panic!("reference must remain valid");
                    };
                    assert_eq!(cache_hit, step.cache_hit);
                    assert_eq!(roundtrip, step.verdict);
                    completions += 1;
                }
                ValidationEvent::RoundtripComplete(event) => {
                    assert_eq!(event.path, path);
                    assert!(event.passed && event.failure_reason.is_none() && event.diff.is_none());
                    roundtrips += 1;
                }
                ValidationEvent::Finished(stats) => {
                    assert_eq!(stats.coverage(), RunCoverage::Complete);
                    assert_eq!(stats.total_files, 1);
                    assert_eq!(stats.valid_files, 1);
                    assert_eq!(
                        stats.invalid_files + stats.parse_errors + stats.roundtrip_failed,
                        0
                    );
                    assert_eq!(stats.cache_hits, usize::from(step.cache_hit));
                    assert_eq!(stats.cache_misses, usize::from(!step.cache_hit));
                    assert_eq!(stats.roundtrip_passed, usize::from(step.roundtrip));
                    assert!(!stats.cancelled);
                    finished = true;
                }
                other => panic!("unexpected cache workflow event: {other:?}"),
            }
        }
        assert!(finished);
        assert_eq!(completions, 1);
        assert_eq!(roundtrips, usize::from(step.roundtrip));
    }
}

#[test]
fn alignment_spec_cache_does_not_promote_basic_validation() {
    use std::sync::Arc;
    use talkbank_cache::CachePool;
    use talkbank_model::ErrorCode;

    enum Pass {
        BasicCold,
        AlignmentRequired,
        BasicWarm,
    }
    let mut config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        ..Default::default()
    };
    let cache = Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated cache"));
    let path = workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E714_4.cha");
    let source = std::fs::read_to_string(&path).expect("canonical grouped mismatch");
    for pass in [Pass::BasicCold, Pass::AlignmentRequired, Pass::BasicWarm] {
        config.check_alignment = matches!(pass, Pass::AlignmentRequired);
        let expected_hit = matches!(pass, Pass::BasicWarm);
        let (events, _cancel) =
            validate_files_streaming(vec![path.clone()], &config, Some(cache.clone()));
        let mut diagnostics = Vec::new();
        let mut completed = false;
        let mut finished = false;
        for event in events {
            assert!(!finished, "Finished is terminal");
            match event {
                ValidationEvent::Discovering => {}
                ValidationEvent::Started { total_files } => assert_eq!(total_files, 1),
                ValidationEvent::Errors(event) => {
                    assert!(!completed);
                    assert_eq!(event.path, path);
                    assert_eq!(event.source.as_ref(), source);
                    diagnostics.extend(event.errors);
                }
                ValidationEvent::FileComplete(event) => {
                    assert!(!completed);
                    assert_eq!(event.path, path);
                    match pass {
                        Pass::AlignmentRequired => {
                            assert!(matches!(
                                event.status,
                                FileStatus::Invalid {
                                    error_count: 1,
                                    cache_hit: false
                                }
                            ));
                            assert_eq!(diagnostics.len(), 1);
                            assert_eq!(diagnostics[0].code, ErrorCode::PhoCountMismatchTooFew);
                        }
                        Pass::BasicCold | Pass::BasicWarm => {
                            assert!(diagnostics.is_empty());
                            assert!(matches!(event.status, FileStatus::Valid {
                                cache_hit, roundtrip: RoundtripVerdict::NotRequested,
                            } if cache_hit == expected_hit));
                        }
                    }
                    completed = true;
                }
                ValidationEvent::Finished(stats) => {
                    assert!(completed);
                    assert_eq!(stats.coverage(), RunCoverage::Complete);
                    assert_eq!(stats.total_files, 1);
                    assert_eq!(stats.invalid_files, usize::from(config.check_alignment));
                    assert_eq!(stats.valid_files, usize::from(!config.check_alignment));
                    assert_eq!(stats.cache_hits, usize::from(expected_hit));
                    assert_eq!(stats.cache_misses, usize::from(!expected_hit));
                    assert_eq!(
                        stats.parse_errors + stats.roundtrip_failed + stats.roundtrip_passed,
                        0
                    );
                    assert!(!stats.cancelled);
                    finished = true;
                }
                other => panic!("unexpected alignment-policy event: {other:?}"),
            }
        }
        assert!(finished);
    }
    assert_eq!(
        std::fs::read_to_string(path).expect("unchanged spec"),
        source
    );
}

#[test]
fn reference_completion_survives_cache_write_failures() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct RefuseCacheWrites {
        path: std::path::PathBuf,
        validation: AtomicUsize,
        roundtrip: AtomicUsize,
    }
    impl ValidationCache for RefuseCacheWrites {
        fn get(&self, _: &Path, _: bool) -> Option<CacheOutcome> {
            None
        }
        fn set(&self, path: &Path, alignment: bool, outcome: CacheOutcome) -> Result<(), String> {
            assert_eq!(path, self.path);
            assert!(alignment);
            assert_eq!(outcome, CacheOutcome::Valid);
            self.validation.fetch_add(1, Ordering::SeqCst);
            Err("test cache refuses validation write".to_owned())
        }
        fn set_roundtrip(
            &self,
            path: &Path,
            alignment: bool,
            outcome: CacheOutcome,
        ) -> Result<(), String> {
            assert_eq!(path, self.path);
            assert!(alignment);
            assert_eq!(outcome, CacheOutcome::Valid);
            self.roundtrip.fetch_add(1, Ordering::SeqCst);
            Err("test cache refuses roundtrip write".to_owned())
        }
    }
    enum Phase {
        Roundtrip,
        Completion,
        Finished,
        Closed,
    }
    let path = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    let cache = Arc::new(RefuseCacheWrites {
        path: path.clone(),
        validation: AtomicUsize::new(0),
        roundtrip: AtomicUsize::new(0),
    });
    let config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        roundtrip: true,
        ..Default::default()
    };
    let (events, _cancel) =
        validate_files_streaming(vec![path.clone()], &config, Some(cache.clone()));
    let mut phase = Phase::Roundtrip;
    for event in events {
        match event {
            ValidationEvent::Discovering => assert!(matches!(phase, Phase::Roundtrip)),
            ValidationEvent::Started { total_files } => {
                assert!(matches!(phase, Phase::Roundtrip));
                assert_eq!(total_files, 1);
            }
            ValidationEvent::RoundtripComplete(event) => {
                assert!(matches!(phase, Phase::Roundtrip));
                assert_eq!(event.path, path);
                assert!(event.passed && event.failure_reason.is_none() && event.diff.is_none());
                phase = Phase::Completion;
            }
            ValidationEvent::FileComplete(event) => {
                assert!(matches!(phase, Phase::Completion));
                assert_eq!(event.path, path);
                assert!(matches!(
                    event.status,
                    FileStatus::Valid {
                        cache_hit: false,
                        roundtrip: RoundtripVerdict::Passed,
                    }
                ));
                phase = Phase::Finished;
            }
            ValidationEvent::Finished(stats) => {
                assert!(matches!(phase, Phase::Finished));
                assert_eq!(stats.coverage(), RunCoverage::Complete);
                assert_eq!(stats.total_files, 1);
                assert_eq!(stats.valid_files, 1);
                assert_eq!(stats.roundtrip_passed, 1);
                assert_eq!(stats.cache_misses, 1);
                assert_eq!(
                    stats.cache_hits
                        + stats.invalid_files
                        + stats.parse_errors
                        + stats.roundtrip_failed,
                    0
                );
                assert!(!stats.cancelled);
                phase = Phase::Closed;
            }
            other => panic!("unexpected cache-failure event: {other:?}"),
        }
    }
    assert!(matches!(phase, Phase::Closed));
    assert_eq!(cache.validation.load(Ordering::SeqCst), 1);
    assert_eq!(cache.roundtrip.load(Ordering::SeqCst), 1);
}

#[test]
fn reference_roundtrip_cache_survives_missing_validation_entry() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use talkbank_cache::CachePool;

    // Only genuine completed roundtrips are persisted. Validation writes fail
    // at their public boundary, so no validation result can bypass fresh work.
    struct RoundtripOnlyCache {
        store: CachePool,
        validation_attempts: AtomicUsize,
        roundtrip_writes: AtomicUsize,
    }
    impl ValidationCache for RoundtripOnlyCache {
        fn get(&self, _: &Path, _: bool) -> Option<CacheOutcome> {
            None
        }
        fn set(&self, _: &Path, _: bool, _: CacheOutcome) -> Result<(), String> {
            self.validation_attempts.fetch_add(1, Ordering::SeqCst);
            Err("test validation cache write failure".to_owned())
        }
        fn get_roundtrip(&self, path: &Path, alignment: bool) -> Option<CacheOutcome> {
            ValidationCache::get_roundtrip(&self.store, path, alignment)
        }
        fn set_roundtrip(
            &self,
            path: &Path,
            alignment: bool,
            outcome: CacheOutcome,
        ) -> Result<(), String> {
            self.roundtrip_writes.fetch_add(1, Ordering::SeqCst);
            ValidationCache::set_roundtrip(&self.store, path, alignment, outcome)
        }
    }
    enum Pass {
        Cold,
        StoredRoundtrip,
    }
    let config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        roundtrip: true,
        ..Default::default()
    };
    let cache = Arc::new(RoundtripOnlyCache {
        store: CachePool::in_memory(config.cache_identity()).expect("isolated roundtrip store"),
        validation_attempts: AtomicUsize::new(0),
        roundtrip_writes: AtomicUsize::new(0),
    });
    let path = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    for pass in [Pass::Cold, Pass::StoredRoundtrip] {
        let expected_hit = matches!(pass, Pass::StoredRoundtrip);
        let (events, _cancel) =
            validate_files_streaming(vec![path.clone()], &config, Some(cache.clone()));
        let mut roundtrips = 0;
        let mut completions = 0;
        let mut finished = false;
        for event in events {
            assert!(!finished, "Finished is terminal");
            match event {
                ValidationEvent::Discovering => {}
                ValidationEvent::Started { total_files } => assert_eq!(total_files, 1),
                ValidationEvent::RoundtripComplete(event) => {
                    assert_eq!(completions, 0);
                    assert_eq!(event.path, path);
                    assert!(event.passed && event.failure_reason.is_none() && event.diff.is_none());
                    roundtrips += 1;
                }
                ValidationEvent::FileComplete(event) => {
                    assert_eq!(roundtrips, 1);
                    assert_eq!(event.path, path);
                    assert!(matches!(event.status, FileStatus::Valid {
                        cache_hit, roundtrip: RoundtripVerdict::Passed,
                    } if cache_hit == expected_hit));
                    completions += 1;
                }
                ValidationEvent::Finished(stats) => {
                    assert_eq!(completions, 1);
                    assert_eq!(stats.coverage(), RunCoverage::Complete);
                    assert_eq!(stats.total_files, 1);
                    assert_eq!(stats.valid_files, 1);
                    assert_eq!(stats.roundtrip_passed, 1);
                    assert_eq!(stats.cache_hits, usize::from(expected_hit));
                    assert_eq!(stats.cache_misses, usize::from(!expected_hit));
                    assert_eq!(
                        stats.invalid_files + stats.parse_errors + stats.roundtrip_failed,
                        0
                    );
                    assert!(!stats.cancelled);
                    finished = true;
                }
                other => panic!("unexpected partial-cache event: {other:?}"),
            }
        }
        assert!(finished);
    }
    assert_eq!(cache.validation_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(cache.roundtrip_writes.load(Ordering::SeqCst), 1);
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

#[test]
fn empty_reference_selection_completes_without_file_events() {
    // An empty selection is a workflow boundary, not an invented CHAT file.
    // Reuse the same event-state and accounting assertions as populated runs.
    check_run(&[], Workflow::FileValidation);
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
                assert_eq!(stats.cache_hit_rate(), 0.0);
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
