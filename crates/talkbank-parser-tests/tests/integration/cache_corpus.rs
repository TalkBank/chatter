//! Cache facts must originate in real canonical-file validation.

use crate::cache_shim::{cached, content, verdict};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use talkbank_cache::{CacheOutcome, CachePool};
use talkbank_model::ParseError;
use talkbank_model::validation::AlignmentValidation;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_transform::validation_runner::{
    FileStatus, RoundtripCheck, RoundtripVerdict, RunCache, RunEnding, ValidationConfig,
    ValidationEvent, ValidationStatsSnapshot, validate_files_streaming,
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
        jobs: std::num::NonZeroUsize::new(2),
        ..Default::default()
    };
    let cache =
        Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated SQLite cache"));
    assert_eq!(cache.stats().expect("cache stats").total_entries, 0);
    let cold = collect_run(
        &paths,
        &config,
        RunCache::ReadWrite(cache.clone()),
        &BTreeSet::new(),
    );
    let mut hits = BTreeSet::new();
    for path in &paths {
        let expected = if cold.diagnostics.contains_key(path) {
            CacheOutcome::Invalid
        } else {
            CacheOutcome::Valid
        };
        assert_eq!(
            verdict(talkbank_cache::VerdictReader::get(
                cache.as_ref(),
                &cached(path),
                &content(path),
                config.alignment
            )),
            Some(expected),
            "{}",
            path.display()
        );
        if expected == CacheOutcome::Valid {
            assert_eq!(
                verdict(talkbank_cache::VerdictReader::get_roundtrip(
                    cache.as_ref(),
                    &cached(path),
                    &content(path),
                    config.alignment
                )),
                None
            );
            hits.insert(path.clone());
        }
    }
    assert!(
        !hits.is_empty(),
        "canonical files must establish real warm hits"
    );
    let warm = collect_run(&paths, &config, RunCache::ReadOnly(cache.clone()), &hits);
    assert_eq!(cold.verdicts, warm.verdicts);
    assert_eq!(
        cold.diagnostics, warm.diagnostics,
        "invalid/warning files still need full evidence"
    );
    assert_eq!(cold.stats.roundtrip_passed(), warm.stats.roundtrip_passed());
    assert_eq!(cold.stats.valid_files(), warm.stats.valid_files());
    assert_eq!(cold.stats.invalid_files(), warm.stats.invalid_files());
    // A cached validation result is not evidence that a roundtrip ran. First
    // request must do the work and persist its independently learned result.
    let roundtrip_config = ValidationConfig {
        roundtrip: RoundtripCheck::Run,
        ..config
    };
    let backfilled = collect_run(
        &paths,
        &roundtrip_config,
        RunCache::ReadWrite(cache.clone()),
        &BTreeSet::new(),
    );
    assert_eq!(cold.diagnostics, backfilled.diagnostics);
    assert_eq!(cold.stats.valid_files(), backfilled.stats.valid_files());
    assert_eq!(cold.stats.invalid_files(), backfilled.stats.invalid_files());
    for path in &hits {
        assert_eq!(
            verdict(talkbank_cache::VerdictReader::get_roundtrip(
                cache.as_ref(),
                &cached(path),
                &content(path),
                roundtrip_config.alignment
            )),
            Some(talkbank_cache::RoundtripOutcome::Passed)
        );
    }
    let reused = collect_run(
        &paths,
        &roundtrip_config,
        RunCache::ReadOnly(cache.clone()),
        &hits,
    );
    assert_eq!(backfilled.verdicts, reused.verdicts);
    assert_eq!(backfilled.diagnostics, reused.diagnostics);
    assert_eq!(
        backfilled.stats.roundtrip_passed(),
        reused.stats.roundtrip_passed()
    );
    assert!(
        cache.stats().expect("cache stats").storage == talkbank_cache::StorageStats::InMemory,
        "no disk cache created"
    );
}

/// Persistence failures must not replace actual validation with invented facts.
#[test]
fn canonical_cache_write_failure_preserves_results_and_forces_revalidation() {
    use talkbank_parser_tests::repo_paths::workspace_root;

    use crate::cache_shim::{Attempt, RefusingCache};
    let root = workspace_root();
    let good = root.join("corpus/reference/core/basic-conversation.cha");
    let bad =
        root.join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E714_4.cha");
    let paths = [good.clone(), bad.clone()];
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        roundtrip: RoundtripCheck::Run,
        alignment: AlignmentValidation::IncludeTierAlignment,
        ..Default::default()
    };
    let cache = Arc::new(RefusingCache::new(config.cache_identity()));
    let first = collect_run(
        &paths,
        &config,
        RunCache::ReadWrite(cache.clone()),
        &BTreeSet::new(),
    );
    let second = collect_run(
        &paths,
        &config,
        RunCache::ReadWrite(cache.clone()),
        &BTreeSet::new(),
    );
    assert_eq!(first.verdicts, second.verdicts);
    assert_eq!(first.diagnostics, second.diagnostics);
    assert_eq!(first.stats.valid_files(), 1);
    assert_eq!(first.stats.invalid_files(), 1);
    assert_eq!(first.stats.roundtrip_passed(), 1);
    let per_run = [
        Attempt::Roundtrip(
            cached(&good),
            config.alignment,
            talkbank_cache::RoundtripOutcome::Passed,
        ),
        Attempt::Validation(cached(&good), config.alignment, CacheOutcome::Valid),
        Attempt::Validation(cached(&bad), config.alignment, CacheOutcome::Invalid),
    ];
    assert_eq!(
        cache.attempts(),
        per_run.iter().chain(&per_run).cloned().collect::<Vec<_>>()
    );
}

fn collect_run(
    paths: &[PathBuf],
    config: &ValidationConfig,
    cache: RunCache,
    hits: &BTreeSet<PathBuf>,
) -> CompletedRun {
    let expected: BTreeSet<_> = paths.iter().cloned().collect();
    let (events, _cancel) = validate_files_streaming(
        paths.to_vec(),
        &talkbank_transform::ValidationRun::new(config.clone(), cache)
            .expect("a cache opened for this run's identity"),
    );
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
            ValidationEvent::FileComplete(event) => {
                assert!(expected.contains(&event.path));
                if let Some(shown) = event.status.shown() {
                    assert!(
                        diagnostics
                            .insert(
                                event.path.clone(),
                                (Arc::from(shown.source), shown.errors.to_vec())
                            )
                            .is_none()
                    );
                }
                let expected_hit = hits.contains(&event.path);
                let hit = matches!(
                    event.cache,
                    talkbank_transform::validation_runner::CacheUse::Hit
                );
                let verdict = match event.status {
                    FileStatus::Valid { roundtrip, .. } => {
                        assert_eq!(hit, expected_hit);
                        let expected_roundtrip = match config.roundtrip {
                            RoundtripCheck::Run => RoundtripVerdict::Passed,
                            RoundtripCheck::Skip => RoundtripVerdict::NotRequested,
                        };
                        assert_eq!(roundtrip, expected_roundtrip);
                        if let RoundtripVerdict::Passed = roundtrip {
                            assert!(roundtrips.insert(event.path.clone()));
                        }
                        Verdict::Valid(roundtrip)
                    }
                    FileStatus::Invalid { diagnostics: shown } => {
                        assert!(!hit && !expected_hit);
                        assert!(diagnostics.contains_key(&event.path));
                        Verdict::Invalid(shown.error_count().get())
                    }
                    other => panic!("unexpected canonical status: {other:?}"),
                };
                assert!(verdicts.insert(event.path, verdict).is_none());
            }
            ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                let stats = stats.snapshot();
                assert_eq!(stats.total_files().get(), paths.len());
                assert_eq!(stats.cache_hits(), hits.len());
                assert_eq!(stats.cache_misses(), paths.len() - hits.len());
                assert_eq!(stats.roundtrip_passed(), roundtrips.len());
                assert_eq!(stats.roundtrip_failed(), 0);
                assert_eq!(verdicts.keys().cloned().collect::<BTreeSet<_>>(), expected);
                terminal = Some(stats.clone());
            }
            ValidationEvent::Finished(RunEnding::Stopped { stats, reason }) => panic!(
                "stopped ({reason:?}) with {} left: {stats:?}",
                stats.missing_files()
            ),
            ValidationEvent::Finished(RunEnding::Incomplete { stats, .. }) => {
                panic!("lost {}: {stats:?}", stats.missing_files())
            }
            ValidationEvent::Finished(RunEnding::Aborted(reason)) => panic!("aborted: {reason}"),
            ValidationEvent::Finished(RunEnding::NothingFound) => panic!("the run found nothing"),
        }
    }
    CompletedRun {
        verdicts,
        diagnostics,
        stats: terminal.expect("complete terminal receipt"),
    }
}
