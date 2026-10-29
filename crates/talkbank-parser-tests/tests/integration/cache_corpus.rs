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

/// Cache maintenance must still address a deleted canonical subtree, without
/// recreating its directories or touching an adjacent prefix's learned facts.
#[test]
fn file_identity_recovery_deleted_subtree_retains_exact_cache_scope() {
    use talkbank_cache::CacheScope;
    use talkbank_model::ResolvedPrefix;
    use talkbank_parser_tests::repo_paths::workspace_root;

    let fixture = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    let directory = tempfile::tempdir().expect("isolated canonical copies");
    let removed = directory.path().join("corpus");
    let nested = removed.join("deeper");
    let sibling = directory.path().join("corpus-other");
    std::fs::create_dir_all(&nested).expect("nested subtree");
    std::fs::create_dir(&sibling).expect("sibling subtree");
    let paths = [
        removed.join("basic-conversation.cha"),
        nested.join("basic-conversation.cha"),
        sibling.join("basic-conversation.cha"),
    ];
    for path in &paths {
        std::fs::copy(&fixture, path).expect("original canonical bytes and basename");
    }
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..Default::default()
    };
    let cache = Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated cache"));
    let cold = collect_run(
        &paths,
        &config,
        RunCache::ReadWrite(cache.clone()),
        &BTreeSet::new(),
    );
    assert_eq!(cold.stats.valid_files(), 3);
    assert!(cold.diagnostics.is_empty());
    assert_eq!(cache.count(&CacheScope::All).expect("all rows"), 3);
    let prefix = ResolvedPrefix::of(&removed).expect("existing directory prefix");
    let file_prefix = CacheScope::Under(ResolvedPrefix::of(&paths[0]).expect("file prefix"));
    assert_eq!(cache.count(&file_prefix).expect("one file"), 1);
    for path in &paths[..2] {
        std::fs::remove_file(path).expect("remove only test copies");
    }
    std::fs::remove_dir(&nested).expect("empty nested test directory");
    std::fs::remove_dir(&removed).expect("empty parent test directory");
    let after = ResolvedPrefix::of(&removed).expect("deleted directory prefix");
    assert_eq!(prefix, after);
    for suffix in ["/", "//", "/."] {
        let mut spelling = removed.clone().into_os_string();
        spelling.push(suffix);
        assert_eq!(
            ResolvedPrefix::of(std::path::Path::new(&spelling))
                .expect("deleted directory spelling"),
            after
        );
    }
    let mut nested_spelling = nested.into_os_string();
    nested_spelling.push("/");
    let nested_scope = CacheScope::Under(
        ResolvedPrefix::of(std::path::Path::new(&nested_spelling)).expect("deleted nested prefix"),
    );
    assert_eq!(
        cache.count(&nested_scope).expect("nested transcript only"),
        1
    );
    let scope = CacheScope::Under(after);
    for _ in 0..2 {
        assert_eq!(cache.count(&scope).expect("read-only preview"), 2);
        assert_eq!(
            cache.count(&CacheScope::All).expect("preview leaves rows"),
            3
        );
        assert!(!removed.exists(), "preview must not recreate inputs");
    }
    assert_eq!(cache.clear(&scope).expect("clear deleted subtree"), 2);
    assert_eq!(cache.clear(&scope).expect("repeat clear"), 0);
    assert_eq!(cache.count(&CacheScope::All).expect("sibling retained"), 1);
    let retained = vec![paths[2].clone()];
    let warm = collect_run(
        &retained,
        &config,
        RunCache::ReadOnly(cache.clone()),
        &retained.iter().cloned().collect(),
    );
    assert_eq!(warm.stats.valid_files(), 1);
    assert_eq!(warm.stats.cache_hits(), 1);
    assert_eq!(
        std::fs::read(&paths[2]).expect("sibling source"),
        std::fs::read(&fixture).expect("fixture")
    );
    assert!(!removed.exists());
}

/// A file link has its own named validation/cache location. Removing and
/// clearing it must leave both its target's bytes and its learned verdict.
#[cfg(unix)]
#[test]
fn file_identity_recovery_deleted_link_keeps_target_verdict_and_media_name() {
    use talkbank_cache::CacheScope;
    use talkbank_model::ResolvedPrefix;
    use talkbank_parser_tests::repo_paths::workspace_root;

    let fixture = workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/W109_4.cha");
    let directory = tempfile::tempdir().expect("isolated named copies");
    let target_directory = directory.path().join("target");
    let link_directory = directory.path().join("linked");
    std::fs::create_dir(&target_directory).expect("target directory");
    std::fs::create_dir(&link_directory).expect("link directory");
    let target = target_directory.join("Schlüssel.cha");
    let link = link_directory.join("Schlüssel.cha");
    std::fs::copy(&fixture, &target).expect("canonical media bytes");
    std::os::unix::fs::symlink(&target, &link).expect("named link");
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..Default::default()
    };
    let cache = Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated cache"));
    let cold = collect_run(
        &[target.clone(), link.clone()],
        &config,
        RunCache::ReadWrite(cache.clone()),
        &BTreeSet::new(),
    );
    assert_eq!(
        cold.stats.valid_files(),
        2,
        "both stored names match @Media"
    );
    assert!(cold.diagnostics.is_empty());
    let prefix = ResolvedPrefix::of(&link).expect("file link prefix");
    assert_ne!(prefix, ResolvedPrefix::of(&target).expect("target prefix"));
    std::fs::remove_file(&link).expect("only the test link");
    assert_eq!(
        ResolvedPrefix::of(&link).expect("deleted file prefix"),
        prefix
    );
    let scope = CacheScope::Under(prefix);
    assert_eq!(cache.count(&scope).expect("link row"), 1);
    assert_eq!(cache.clear(&scope).expect("link-only clear"), 1);
    assert_eq!(
        cache.count(&CacheScope::All).expect("target row survives"),
        1
    );
    let retained = vec![target.clone()];
    let warm = collect_run(
        &retained,
        &config,
        RunCache::ReadOnly(cache.clone()),
        &retained.iter().cloned().collect(),
    );
    assert_eq!(warm.stats.cache_hits(), 1);
    assert_eq!(
        std::fs::read(&target).expect("target retained"),
        std::fs::read(&fixture).expect("fixture")
    );
    assert!(!link.exists());
}
