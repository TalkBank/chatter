//! Real file scheduling must preserve every canonical diagnostic and completion.

#[path = "runner_lifecycle_corpus.rs"]
mod lifecycle_contracts;

#[path = "runner_cache_corpus.rs"]
mod cache_contracts;

use std::collections::BTreeMap;
use std::path::Path;
use talkbank_cache::{CacheError, CacheLookup, ContentHash};
use talkbank_model::model::TranscriptName;
use talkbank_model::validation::AlignmentValidation;
use talkbank_model::{ErrorCollector, ParseError, Severity};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, repo_paths::workspace_root};
use talkbank_transform::validation_runner::{
    CacheOutcome, FileStatus, RoundtripCheck, RoundtripVerdict, RunCache, RunEnding,
    ValidationCache, ValidationConfig, ValidationEvent, validate_directory_streaming,
    validate_files_streaming,
};

/// What one reference file's result must say: its diagnostics (with the
/// source they point into) and its verdict.
struct ExpectedFile {
    source: String,
    errors: Vec<ParseError>,
    error_count: usize,
    roundtrip: RoundtripVerdict,
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
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..Default::default()
    };
    let (events, _cancel) = validate_files_streaming(
        vec![missing.clone(), bad.clone(), good.clone()],
        &talkbank_transform::ValidationRun::uncached(config.clone()),
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
                            roundtrip: RoundtripVerdict::NotRequested,
                            ..
                        }
                    ));
                }
            }
            ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                let stats = stats.snapshot();
                assert!(pending.is_empty());
                assert_eq!(stats.total_files().get(), 3);
                assert_eq!(stats.valid_files(), 1);
                // Read failure is a failed file, not a parser diagnostic.
                assert_eq!(stats.invalid_files(), 2);
                assert_eq!(stats.cache_hits(), 0);
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
        roundtrip: RoundtripCheck,
        cache: talkbank_transform::validation_runner::CacheUse,
        verdict: RoundtripVerdict,
    }
    let mut config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..Default::default()
    };
    let cache =
        Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated SQLite cache"));
    let path = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    for step in [
        Step {
            roundtrip: RoundtripCheck::Skip,
            cache: talkbank_transform::validation_runner::CacheUse::Miss,
            verdict: RoundtripVerdict::NotRequested,
        },
        Step {
            roundtrip: RoundtripCheck::Run,
            cache: talkbank_transform::validation_runner::CacheUse::Miss,
            verdict: RoundtripVerdict::Passed,
        },
        Step {
            roundtrip: RoundtripCheck::Run,
            cache: talkbank_transform::validation_runner::CacheUse::Hit,
            verdict: RoundtripVerdict::Passed,
        },
        Step {
            roundtrip: RoundtripCheck::Skip,
            cache: talkbank_transform::validation_runner::CacheUse::Hit,
            verdict: RoundtripVerdict::NotRequested,
        },
    ] {
        config.roundtrip = step.roundtrip;
        let (events, _cancel) = validate_files_streaming(
            vec![path.clone()],
            &talkbank_transform::ValidationRun::new(
                config.clone(),
                RunCache::ReadWrite(cache.clone()),
            )
            .expect("a cache opened for this run's identity"),
        );
        let mut completions = 0;
        let mut finished = false;
        for event in events {
            assert!(!finished, "Finished is terminal");
            match event {
                ValidationEvent::Discovering => {}
                ValidationEvent::Started { total_files } => assert_eq!(total_files, 1),
                ValidationEvent::FileComplete(event) => {
                    assert_eq!(event.path, path);
                    assert!(
                        event.status.shown().is_none(),
                        "valid reference diagnostics: {:?}",
                        event.status
                    );
                    let FileStatus::Valid { roundtrip, .. } = event.status else {
                        panic!("reference must remain valid");
                    };
                    assert_eq!(event.cache, step.cache);
                    assert_eq!(roundtrip, step.verdict);
                    completions += 1;
                }
                ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                    let stats = stats.snapshot();
                    assert_eq!(stats.total_files().get(), 1);
                    assert_eq!(stats.valid_files(), 1);
                    assert_eq!(stats.invalid_files() + stats.roundtrip_failed(), 0);
                    assert_eq!(
                        (stats.cache_hits(), stats.cache_misses()),
                        match step.cache {
                            talkbank_transform::validation_runner::CacheUse::Hit => (1, 0),
                            talkbank_transform::validation_runner::CacheUse::Miss => (0, 1),
                            talkbank_transform::validation_runner::CacheUse::NotConsulted => (0, 0),
                        }
                    );
                    assert_eq!(stats.roundtrip_passed(), roundtrips_of(step.roundtrip));
                    finished = true;
                }
                other => panic!("unexpected cache workflow event: {other:?}"),
            }
        }
        assert!(finished);
        assert_eq!(completions, 1);
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
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..Default::default()
    };
    let cache = Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated cache"));
    let path = workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E714_4.cha");
    let source = std::fs::read_to_string(&path).expect("canonical grouped mismatch");
    for pass in [Pass::BasicCold, Pass::AlignmentRequired, Pass::BasicWarm] {
        config.alignment = match pass {
            Pass::AlignmentRequired => AlignmentValidation::IncludeTierAlignment,
            Pass::BasicCold | Pass::BasicWarm => AlignmentValidation::Structure,
        };
        let expected_hit = matches!(pass, Pass::BasicWarm);
        let (events, _cancel) = validate_files_streaming(
            vec![path.clone()],
            &talkbank_transform::ValidationRun::new(
                config.clone(),
                RunCache::ReadWrite(cache.clone()),
            )
            .expect("a cache opened for this run's identity"),
        );
        let mut completed = false;
        let mut finished = false;
        for event in events {
            assert!(!finished, "Finished is terminal");
            match event {
                ValidationEvent::Discovering => {}
                ValidationEvent::Started { total_files } => assert_eq!(total_files, 1),
                ValidationEvent::FileComplete(event) => {
                    assert!(!completed);
                    assert_eq!(event.path, path);
                    let diagnostics = match event.status.shown() {
                        Some(shown) => {
                            assert_eq!(shown.source, source);
                            shown.errors.to_vec()
                        }
                        None => Vec::new(),
                    };
                    match pass {
                        Pass::AlignmentRequired => {
                            assert!(matches!(
                                event.status,
                                FileStatus::Invalid { ref diagnostics }
                                    if diagnostics.error_count().get() == 1
                            ));
                            assert_eq!(diagnostics.len(), 1);
                            assert_eq!(diagnostics[0].code, ErrorCode::PhoCountMismatchTooFew);
                        }
                        Pass::BasicCold | Pass::BasicWarm => {
                            assert!(diagnostics.is_empty());
                            assert!(matches!(
                                event.status,
                                FileStatus::Valid {
                                    roundtrip: RoundtripVerdict::NotRequested,
                                    ..
                                }
                            ));
                            assert_eq!(
                                matches!(
                                    event.cache,
                                    talkbank_transform::validation_runner::CacheUse::Hit
                                ),
                                expected_hit
                            );
                        }
                    }
                    completed = true;
                }
                ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                    let stats = stats.snapshot();
                    assert!(completed);
                    assert_eq!(stats.total_files().get(), 1);
                    assert_eq!(
                        stats.invalid_files(),
                        usize::from(config.alignment == AlignmentValidation::IncludeTierAlignment)
                    );
                    assert_eq!(
                        stats.valid_files(),
                        usize::from(config.alignment == AlignmentValidation::Structure)
                    );
                    assert_eq!(stats.cache_hits(), usize::from(expected_hit));
                    assert_eq!(stats.cache_misses(), usize::from(!expected_hit));
                    assert_eq!(stats.roundtrip_failed() + stats.roundtrip_passed(), 0);
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
    use std::sync::Arc;

    enum Phase {
        Completion,
        Finished,
        Closed,
    }
    let path = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    let cache = Arc::new(crate::cache_shim::RefusingCache::new(
        crate::default_run_identity().clone(),
    ));
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        roundtrip: RoundtripCheck::Run,
        ..Default::default()
    };
    let (events, _cancel) = validate_files_streaming(
        vec![path.clone()],
        &talkbank_transform::ValidationRun::new(config.clone(), RunCache::ReadWrite(cache.clone()))
            .expect("a cache opened for this run's identity"),
    );
    let mut phase = Phase::Completion;
    for event in events {
        match event {
            ValidationEvent::Discovering => assert!(matches!(phase, Phase::Completion)),
            ValidationEvent::Started { total_files } => {
                assert!(matches!(phase, Phase::Completion));
                assert_eq!(total_files, 1);
            }
            ValidationEvent::FileComplete(event) => {
                assert!(matches!(phase, Phase::Completion));
                assert_eq!(event.path, path);
                assert!(matches!(
                    event.status,
                    FileStatus::Valid {
                        roundtrip: RoundtripVerdict::Passed,
                        ..
                    }
                ));
                phase = Phase::Finished;
            }
            ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                let stats = stats.snapshot();
                assert!(matches!(phase, Phase::Finished));
                assert_eq!(stats.total_files().get(), 1);
                assert_eq!(stats.valid_files(), 1);
                assert_eq!(stats.roundtrip_passed(), 1);
                assert_eq!(stats.cache_misses(), 1);
                assert_eq!(
                    stats.cache_hits() + stats.invalid_files() + stats.roundtrip_failed(),
                    0
                );
                phase = Phase::Closed;
            }
            other => panic!("unexpected cache-failure event: {other:?}"),
        }
    }
    assert!(matches!(phase, Phase::Closed));
    use crate::cache_shim::Attempt;
    assert_eq!(
        cache.attempts(),
        [
            Attempt::Roundtrip(
                crate::cache_shim::cached(&path),
                AlignmentValidation::IncludeTierAlignment,
                talkbank_cache::RoundtripOutcome::Passed,
            ),
            Attempt::Validation(
                crate::cache_shim::cached(&path),
                AlignmentValidation::IncludeTierAlignment,
                CacheOutcome::Valid,
            ),
        ]
    );
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
    impl talkbank_cache::VerdictReader for RoundtripOnlyCache {
        fn identity(&self) -> &talkbank_cache::CacheIdentity {
            crate::default_run_identity()
        }

        fn get(
            &self,
            _: &talkbank_cache::ResolvedPath,
            _: &ContentHash,
            _: AlignmentValidation,
        ) -> Result<CacheLookup<CacheOutcome>, CacheError> {
            Ok(CacheLookup::Miss)
        }

        fn get_roundtrip(
            &self,
            path: &talkbank_cache::ResolvedPath,
            content: &ContentHash,
            alignment: AlignmentValidation,
        ) -> Result<CacheLookup<talkbank_cache::RoundtripOutcome>, CacheError> {
            talkbank_cache::VerdictReader::get_roundtrip(&self.store, path, content, alignment)
        }
    }

    impl ValidationCache for RoundtripOnlyCache {
        fn set(
            &self,
            _: &talkbank_cache::ResolvedPath,
            _: &ContentHash,
            _: AlignmentValidation,
            _: CacheOutcome,
        ) -> Result<(), CacheError> {
            self.validation_attempts.fetch_add(1, Ordering::SeqCst);
            Err(crate::cache_shim::refused(
                "test validation cache write failure",
            ))
        }
        fn set_roundtrip(
            &self,
            path: &talkbank_cache::ResolvedPath,
            content: &ContentHash,
            alignment: AlignmentValidation,
            outcome: talkbank_cache::RoundtripOutcome,
        ) -> Result<(), CacheError> {
            self.roundtrip_writes.fetch_add(1, Ordering::SeqCst);
            ValidationCache::set_roundtrip(&self.store, path, content, alignment, outcome)
        }
    }
    enum Pass {
        Cold,
        StoredRoundtrip,
    }
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        roundtrip: RoundtripCheck::Run,
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
        let (events, _cancel) = validate_files_streaming(
            vec![path.clone()],
            &talkbank_transform::ValidationRun::new(
                config.clone(),
                RunCache::ReadWrite(cache.clone()),
            )
            .expect("a cache opened for this run's identity"),
        );
        let mut completions = 0;
        let mut finished = false;
        for event in events {
            assert!(!finished, "Finished is terminal");
            match event {
                ValidationEvent::Discovering => {}
                ValidationEvent::Started { total_files } => assert_eq!(total_files, 1),
                ValidationEvent::FileComplete(event) => {
                    assert_eq!(event.path, path);
                    assert!(matches!(
                        event.status,
                        FileStatus::Valid {
                            roundtrip: RoundtripVerdict::Passed,
                            ..
                        }
                    ));
                    assert_eq!(
                        matches!(
                            event.cache,
                            talkbank_transform::validation_runner::CacheUse::Hit
                        ),
                        expected_hit
                    );
                    completions += 1;
                }
                ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                    let stats = stats.snapshot();
                    assert_eq!(completions, 1);
                    assert_eq!(stats.total_files().get(), 1);
                    assert_eq!(stats.valid_files(), 1);
                    assert_eq!(stats.roundtrip_passed(), 1);
                    assert_eq!(stats.cache_hits(), usize::from(expected_hit));
                    assert_eq!(stats.cache_misses(), usize::from(!expected_hit));
                    assert_eq!(stats.invalid_files() + stats.roundtrip_failed(), 0);
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
fn empty_reference_selection_ends_nothing_found_without_file_events() {
    // An empty selection is a workflow boundary, not an invented CHAT file.
    // Reuse the same event-state assertions as populated runs.
    check_run(&[], Workflow::FileValidation);
}

fn check_run(corpora: &[ChatCorpus], workflow: Workflow<'_>) {
    let config = ValidationConfig {
        jobs: std::num::NonZeroUsize::new(2),
        roundtrip: match workflow {
            Workflow::DirectoryRoundtrip(_) => RoundtripCheck::Run,
            Workflow::FileValidation => RoundtripCheck::Skip,
        },
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
        let error_count = errors
            .iter()
            .filter(|error| error.severity == Severity::Error)
            .count();
        if error_count > 0 {
            expected_invalid += 1;
        }
        let expected = ExpectedFile {
            source: fixture.source().to_owned(),
            errors,
            error_count,
            roundtrip: match (config.roundtrip, error_count) {
                (RoundtripCheck::Run, 0) => RoundtripVerdict::Passed,
                (RoundtripCheck::Run | RoundtripCheck::Skip, _) => RoundtripVerdict::NotRequested,
            },
        };
        assert!(
            pending
                .insert(fixture.path().to_owned(), expected)
                .is_none()
        );
    }
    let total = pending.len();
    if let RoundtripCheck::Run = config.roundtrip {
        assert!(
            total > expected_invalid,
            "reference workflow must actually run roundtrips"
        );
    }
    let (events, _cancel) = match workflow {
        Workflow::FileValidation => validate_files_streaming(
            pending.keys().cloned().collect(),
            &talkbank_transform::ValidationRun::uncached(config.clone()),
        ),
        Workflow::DirectoryRoundtrip(directory) => validate_directory_streaming(
            directory,
            &talkbank_transform::ValidationRun::uncached(config.clone()),
        ),
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
            ValidationEvent::FileComplete(event) => {
                assert!(matches!(phase, StreamPhase::Running));
                let Some(expected) = pending.remove(&event.path) else {
                    panic!(
                        "unexpected or duplicate completion: {}",
                        event.path.display()
                    );
                };
                match event.status.shown() {
                    Some(shown) => {
                        assert_eq!(expected.source.as_str(), shown.source);
                        assert_eq!(expected.errors, shown.errors, "{}", event.path.display());
                    }
                    None => assert!(expected.errors.is_empty(), "{}", event.path.display()),
                }
                match event.status {
                    FileStatus::Valid { roundtrip, .. } => {
                        assert_eq!(expected.error_count, 0);
                        assert_eq!(roundtrip, expected.roundtrip);
                    }
                    FileStatus::Invalid { diagnostics } => {
                        assert_eq!(diagnostics.error_count().get(), expected.error_count);
                    }
                    other => panic!("unexpected file status: {} {other:?}", event.path.display()),
                }
            }
            ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                let stats = stats.snapshot();
                assert!(matches!(phase, StreamPhase::Running));
                assert!(
                    pending.is_empty(),
                    "terminal event must account for every file"
                );
                assert_eq!(stats.total_files().get(), total);
                assert_eq!(stats.invalid_files(), expected_invalid);
                assert_eq!(stats.valid_files(), total - expected_invalid);
                // No cache: nothing consulted, so no hit, no miss, no rate.
                assert_eq!(stats.cache_hits() + stats.cache_misses(), 0);
                assert_eq!(stats.cache_hit_rate(), None);
                assert_eq!(
                    stats.roundtrip_passed(),
                    match config.roundtrip {
                        RoundtripCheck::Run => total - expected_invalid,
                        RoundtripCheck::Skip => 0,
                    }
                );
                assert_eq!(stats.roundtrip_failed(), 0);
                phase = StreamPhase::Finished;
            }
            ValidationEvent::Finished(RunEnding::Stopped { stats, reason }) => panic!(
                "stopped ({reason:?}) with {} left: {stats:?}",
                stats.missing_files()
            ),
            ValidationEvent::Finished(RunEnding::Incomplete { stats, .. }) => {
                panic!("lost {} files: {stats:?}", stats.missing_files())
            }
            ValidationEvent::Finished(RunEnding::Aborted(reason)) => {
                panic!("runner aborted: {reason}")
            }
            // An empty selection ends here, and only an empty one.
            ValidationEvent::Finished(RunEnding::NothingFound) => {
                assert!(matches!(phase, StreamPhase::Running));
                assert_eq!(total, 0, "a run over {total} files found nothing");
                phase = StreamPhase::Finished;
            }
        }
    }
    assert!(
        matches!(phase, StreamPhase::Finished),
        "channel closure alone is not completion"
    );
}

/// How many roundtrips a one-file run with this check performs.
fn roundtrips_of(check: RoundtripCheck) -> usize {
    match check {
        RoundtripCheck::Run => 1,
        RoundtripCheck::Skip => 0,
    }
}
