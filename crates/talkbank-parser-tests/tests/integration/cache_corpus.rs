//! Cache facts must originate in real canonical-file validation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use talkbank_cache::{CacheOutcome, CachePool, ValidationCache};
use talkbank_model::ParseError;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_transform::validation_runner::{
    CacheMode, FileStatus, RoundtripVerdict, RunCoverage, ValidationConfig, ValidationEvent,
    ValidationStatsSnapshot, validate_files_streaming,
};

#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Valid(RoundtripVerdict),
    Invalid(usize),
}

struct CompletedRun {
    verdicts: BTreeMap<PathBuf, Verdict>,
    diagnostics: BTreeMap<PathBuf, (Arc<str>, Vec<ParseError>)>,
    stats: ValidationStatsSnapshot,
}

#[test]
fn canonical_cache_is_learned_cold_and_reused_without_losing_diagnostics() {
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let paths: Vec<_> = corpus
        .fixtures()
        .iter()
        .map(|fixture| fixture.path().to_owned())
        .collect();
    let config = ValidationConfig {
        jobs: Some(2),
        ..Default::default()
    };
    let cache =
        Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated SQLite cache"));
    assert_eq!(cache.stats().expect("cache stats").total_entries, 0);
    let cold = collect_run(&paths, &config, cache.clone(), &BTreeSet::new());
    let mut hits = BTreeSet::new();
    for path in &paths {
        let expected = if cold.diagnostics.contains_key(path) {
            CacheOutcome::Invalid
        } else {
            CacheOutcome::Valid
        };
        assert_eq!(
            cache.get(path, config.check_alignment),
            Some(expected),
            "{}",
            path.display()
        );
        let equivalent_path = path
            .parent()
            .expect("canonical fixture has a parent")
            .join(".")
            .join(path.file_name().expect("canonical fixture has a filename"));
        assert_eq!(path, &equivalent_path);
        assert_eq!(
            cache.get(&equivalent_path, config.check_alignment),
            Some(expected),
            "equivalent path must reuse the canonical cache fact: {}",
            equivalent_path.display()
        );
        if expected == CacheOutcome::Valid {
            assert_eq!(
                ValidationCache::get_roundtrip(cache.as_ref(), path, config.check_alignment),
                None
            );
            hits.insert(path.clone());
        }
    }
    assert!(
        !hits.is_empty(),
        "canonical files must establish real warm hits"
    );
    let warm_config = ValidationConfig {
        cache: CacheMode::ReadOnly,
        ..config.clone()
    };
    let warm = collect_run(&paths, &warm_config, cache.clone(), &hits);
    assert_eq!(cold.verdicts, warm.verdicts);
    assert_eq!(
        cold.diagnostics, warm.diagnostics,
        "invalid/warning files still need full evidence"
    );
    assert_eq!(cold.stats.roundtrip_passed, warm.stats.roundtrip_passed);
    assert_eq!(cold.stats.valid_files, warm.stats.valid_files);
    assert_eq!(cold.stats.invalid_files, warm.stats.invalid_files);
    // A cached validation result is not evidence that a roundtrip ran. First
    // request must do the work and persist its independently learned result.
    let roundtrip_config = ValidationConfig {
        roundtrip: true,
        ..config
    };
    let backfilled = collect_run(&paths, &roundtrip_config, cache.clone(), &BTreeSet::new());
    assert_eq!(cold.diagnostics, backfilled.diagnostics);
    assert_eq!(cold.stats.valid_files, backfilled.stats.valid_files);
    assert_eq!(cold.stats.invalid_files, backfilled.stats.invalid_files);
    for path in &hits {
        assert_eq!(
            ValidationCache::get_roundtrip(cache.as_ref(), path, roundtrip_config.check_alignment),
            Some(CacheOutcome::Valid)
        );
    }
    let warm_roundtrip_config = ValidationConfig {
        cache: CacheMode::ReadOnly,
        ..roundtrip_config
    };
    let reused = collect_run(&paths, &warm_roundtrip_config, cache.clone(), &hits);
    assert_eq!(backfilled.verdicts, reused.verdicts);
    assert_eq!(backfilled.diagnostics, reused.diagnostics);
    assert_eq!(
        backfilled.stats.roundtrip_passed,
        reused.stats.roundtrip_passed
    );
    assert!(
        cache.stats().expect("cache stats").cache_dir.is_none(),
        "no disk cache created"
    );
}

fn collect_run(
    paths: &[PathBuf],
    config: &ValidationConfig,
    cache: Arc<CachePool>,
    hits: &BTreeSet<PathBuf>,
) -> CompletedRun {
    let expected: BTreeSet<_> = paths.iter().cloned().collect();
    let (events, _cancel) = validate_files_streaming(paths.to_vec(), config, Some(cache));
    let mut verdicts = BTreeMap::new();
    let mut diagnostics = BTreeMap::new();
    let mut roundtrips = BTreeSet::new();
    let mut terminal = None;
    for event in events {
        assert!(
            terminal.is_none(),
            "no events may follow the terminal receipt"
        );
        match event {
            ValidationEvent::Discovering => {}
            ValidationEvent::Started { total_files } => assert_eq!(total_files, paths.len()),
            ValidationEvent::Errors(event) => {
                assert!(expected.contains(&event.path));
                assert!(
                    diagnostics
                        .insert(event.path, (event.source, event.errors))
                        .is_none()
                );
            }
            ValidationEvent::RoundtripComplete(event) => {
                assert!(config.roundtrip, "unrequested roundtrip event");
                assert!(expected.contains(&event.path));
                assert!(event.passed && event.failure_reason.is_none() && event.diff.is_none());
                assert!(roundtrips.insert(event.path));
            }
            ValidationEvent::FileComplete(event) => {
                let expected_hit = hits.contains(&event.path);
                let verdict = match event.status {
                    FileStatus::Valid {
                        cache_hit,
                        roundtrip,
                    } => {
                        assert_eq!(cache_hit, expected_hit);
                        let expected_roundtrip = if config.roundtrip {
                            RoundtripVerdict::Passed
                        } else {
                            RoundtripVerdict::NotRequested
                        };
                        assert_eq!(roundtrip, expected_roundtrip);
                        assert_eq!(roundtrips.contains(&event.path), config.roundtrip);
                        Verdict::Valid(roundtrip)
                    }
                    FileStatus::Invalid {
                        error_count,
                        cache_hit,
                    } => {
                        assert!(!cache_hit && !expected_hit);
                        assert!(diagnostics.contains_key(&event.path));
                        Verdict::Invalid(error_count)
                    }
                    other => panic!("unexpected canonical status: {other:?}"),
                };
                assert!(verdicts.insert(event.path, verdict).is_none());
            }
            ValidationEvent::Finished(stats) => {
                assert_eq!(stats.coverage(), RunCoverage::Complete);
                assert!(!stats.cancelled);
                assert_eq!(stats.total_files, paths.len());
                assert_eq!(stats.cache_hits, hits.len());
                assert_eq!(stats.cache_misses, paths.len() - hits.len());
                assert_eq!(stats.roundtrip_passed, roundtrips.len());
                assert_eq!(stats.roundtrip_failed, 0);
                assert_eq!(verdicts.keys().cloned().collect::<BTreeSet<_>>(), expected);
                terminal = Some(stats);
            }
            ValidationEvent::FinishedIncomplete { stats, lost_files } => {
                panic!("lost {lost_files}: {stats:?}")
            }
            ValidationEvent::Aborted(reason) => panic!("aborted: {reason}"),
        }
    }
    CompletedRun {
        verdicts,
        diagnostics,
        stats: terminal.expect("complete terminal receipt"),
    }
}
