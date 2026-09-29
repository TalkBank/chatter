//! Cache inputs are external evidence, not newly established CHAT validity.

use super::{
    CacheMode, CacheOutcome, FileStatus, Path, RunCoverage, ValidationCache, ValidationConfig,
    ValidationEvent, validate_files_streaming, workspace_root,
};

#[test]
fn reference_cached_roundtrip_failure_is_not_promoted_to_success() {
    use std::sync::Arc;
    use talkbank_cache::CachePool;

    enum ValidationEntry {
        Retained,
        Withheld,
    }
    struct ReplayCache {
        store: Arc<CachePool>,
        validation: ValidationEntry,
    }
    impl ValidationCache for ReplayCache {
        fn get(&self, path: &Path, alignment: bool) -> Option<CacheOutcome> {
            match self.validation {
                ValidationEntry::Retained => ValidationCache::get(&*self.store, path, alignment),
                ValidationEntry::Withheld => None,
            }
        }
        fn set(&self, path: &Path, alignment: bool, outcome: CacheOutcome) -> Result<(), String> {
            ValidationCache::set(&*self.store, path, alignment, outcome)
        }
        fn get_roundtrip(&self, path: &Path, alignment: bool) -> Option<CacheOutcome> {
            ValidationCache::get_roundtrip(&*self.store, path, alignment)
        }
        fn set_roundtrip(
            &self,
            path: &Path,
            alignment: bool,
            outcome: CacheOutcome,
        ) -> Result<(), String> {
            ValidationCache::set_roundtrip(&*self.store, path, alignment, outcome)
        }
    }

    let mut config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        ..Default::default()
    };
    let path = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    let store = Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated cache"));
    // Establish validation from the real file rather than inventing that verdict.
    let (events, _cancel) =
        validate_files_streaming(vec![path.clone()], &config, Some(store.clone()));
    let mut completed = false;
    for event in events {
        if let ValidationEvent::Finished(stats) = event {
            assert_eq!(stats.coverage(), RunCoverage::Complete);
            assert_eq!(stats.valid_files, 1);
            completed = true;
        }
    }
    assert!(completed);
    assert_eq!(
        ValidationCache::get(&*store, &path, config.check_alignment),
        Some(CacheOutcome::Valid)
    );
    // Deliberately supply a negative external cache result. This is a cache
    // replay contract, NOT evidence that this reference fails a fresh roundtrip.
    ValidationCache::set_roundtrip(
        &*store,
        &path,
        config.check_alignment,
        CacheOutcome::Invalid,
    )
    .expect("negative roundtrip cache entry");
    config.roundtrip = true;

    enum Phase {
        Discovery,
        Start,
        Roundtrip,
        Completion,
        Finished,
        Closed,
    }
    for validation in [ValidationEntry::Retained, ValidationEntry::Withheld] {
        let cache = Arc::new(ReplayCache {
            store: store.clone(),
            validation,
        });
        let (events, _cancel) = validate_files_streaming(vec![path.clone()], &config, Some(cache));
        let mut phase = Phase::Discovery;
        for event in events {
            phase = match (phase, event) {
                (Phase::Discovery, ValidationEvent::Discovering) => Phase::Start,
                (Phase::Start, ValidationEvent::Started { total_files: 1 }) => Phase::Roundtrip,
                (Phase::Roundtrip, ValidationEvent::RoundtripComplete(event)) => {
                    assert_eq!(event.path, path);
                    assert!(!event.passed);
                    assert_eq!(
                        event.failure_reason.as_deref(),
                        Some("Roundtrip failed (cached)")
                    );
                    assert!(
                        event.diff.is_none(),
                        "cached verdict cannot fabricate a fresh diff"
                    );
                    Phase::Completion
                }
                (Phase::Completion, ValidationEvent::FileComplete(event)) => {
                    assert_eq!(event.path, path);
                    let FileStatus::RoundtripFailed { cache_hit, reason } = event.status else {
                        panic!("negative roundtrip result must not be reported as valid");
                    };
                    assert!(cache_hit);
                    assert_eq!(reason, "Roundtrip failed (cached)");
                    Phase::Finished
                }
                (Phase::Finished, ValidationEvent::Finished(stats)) => {
                    assert_eq!(stats.coverage(), RunCoverage::Complete);
                    assert_eq!(stats.total_files, 1);
                    assert_eq!(stats.invalid_files, 1);
                    assert_eq!(stats.roundtrip_failed, 1);
                    assert_eq!(stats.cache_hits, 1);
                    assert_eq!(
                        stats.valid_files
                            + stats.parse_errors
                            + stats.internal_failures
                            + stats.roundtrip_passed
                            + stats.cache_misses,
                        0
                    );
                    assert!(!stats.cancelled);
                    Phase::Closed
                }
                (_, event) => panic!("unexpected cached-failure event: {event:?}"),
            };
        }
        assert!(matches!(phase, Phase::Closed));
    }
}
