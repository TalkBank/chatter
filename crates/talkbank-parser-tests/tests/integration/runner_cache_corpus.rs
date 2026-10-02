//! Cache inputs are external evidence, not newly established CHAT validity.

use super::{
    CacheOutcome, FileStatus, RoundtripCheck, RunCache, RunEnding, ValidationCache,
    ValidationConfig, ValidationEvent, validate_files_streaming, workspace_root,
};

#[test]
fn reference_cached_roundtrip_failure_is_not_promoted_to_success() {
    use std::sync::Arc;
    use talkbank_cache::{CacheError, CacheLookup, CachePool, ContentHash};
    use talkbank_model::validation::AlignmentValidation;

    enum ValidationEntry {
        Retained,
        Withheld,
    }
    struct ReplayCache {
        store: Arc<CachePool>,
        validation: ValidationEntry,
    }
    impl talkbank_cache::VerdictReader for ReplayCache {
        fn identity(&self) -> &talkbank_cache::CacheIdentity {
            crate::default_run_identity()
        }

        fn get(
            &self,
            path: &talkbank_cache::ResolvedPath,
            content: &ContentHash,
            alignment: AlignmentValidation,
        ) -> Result<CacheLookup<CacheOutcome>, CacheError> {
            match self.validation {
                ValidationEntry::Retained => {
                    talkbank_cache::VerdictReader::get(&*self.store, path, content, alignment)
                }
                ValidationEntry::Withheld => Ok(CacheLookup::Miss),
            }
        }

        fn get_roundtrip(
            &self,
            path: &talkbank_cache::ResolvedPath,
            content: &ContentHash,
            alignment: AlignmentValidation,
        ) -> Result<CacheLookup<talkbank_cache::RoundtripOutcome>, CacheError> {
            talkbank_cache::VerdictReader::get_roundtrip(&*self.store, path, content, alignment)
        }
    }

    impl ValidationCache for ReplayCache {
        fn set(
            &self,
            path: &talkbank_cache::ResolvedPath,
            content: &ContentHash,
            alignment: AlignmentValidation,
            outcome: CacheOutcome,
        ) -> Result<(), CacheError> {
            ValidationCache::set(&*self.store, path, content, alignment, outcome)
        }
        fn set_roundtrip(
            &self,
            path: &talkbank_cache::ResolvedPath,
            content: &ContentHash,
            alignment: AlignmentValidation,
            outcome: talkbank_cache::RoundtripOutcome,
        ) -> Result<(), CacheError> {
            ValidationCache::set_roundtrip(&*self.store, path, content, alignment, outcome)
        }
    }

    let mut config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..Default::default()
    };
    let path = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    let store = Arc::new(CachePool::in_memory(config.cache_identity()).expect("isolated cache"));
    // Establish validation from the real file rather than inventing that verdict.
    let (events, _cancel) = validate_files_streaming(
        vec![path.clone()],
        &talkbank_transform::ValidationRun::new(config.clone(), RunCache::ReadWrite(store.clone()))
            .expect("a cache opened for this run's identity"),
    );
    let mut completed = false;
    for event in events {
        if let ValidationEvent::Finished(RunEnding::Complete(stats)) = event {
            let stats = stats.snapshot();
            assert_eq!(stats.valid_files(), 1);
            completed = true;
        }
    }
    assert!(completed);
    assert_eq!(
        crate::cache_shim::verdict(talkbank_cache::VerdictReader::get(
            &*store,
            &crate::cache_shim::cached(&path),
            &crate::cache_shim::content(&path),
            config.alignment
        )),
        Some(CacheOutcome::Valid)
    );
    // Deliberately supply a negative external cache result. This is a cache
    // replay contract, NOT evidence that this reference fails a fresh roundtrip.
    ValidationCache::set_roundtrip(
        &*store,
        &crate::cache_shim::cached(&path),
        &crate::cache_shim::content(&path),
        config.alignment,
        talkbank_cache::RoundtripOutcome::Failed,
    )
    .expect("negative roundtrip cache entry");
    config.roundtrip = RoundtripCheck::Run;

    enum Phase {
        Discovery,
        Start,
        Completion,
        Finished,
        Closed,
    }
    for validation in [ValidationEntry::Retained, ValidationEntry::Withheld] {
        let cache = Arc::new(ReplayCache {
            store: store.clone(),
            validation,
        });
        let (events, _cancel) = validate_files_streaming(
            vec![path.clone()],
            &talkbank_transform::ValidationRun::new(config.clone(), RunCache::ReadWrite(cache))
                .expect("a cache opened for this run's identity"),
        );
        let mut phase = Phase::Discovery;
        for event in events {
            phase = match (phase, event) {
                (Phase::Discovery, ValidationEvent::Discovering) => Phase::Start,
                (Phase::Start, ValidationEvent::Started { total_files: 1 }) => Phase::Completion,
                (Phase::Completion, ValidationEvent::FileComplete(event)) => {
                    assert_eq!(event.path, path);
                    let FileStatus::RoundtripFailed { reason, diff, .. } = event.status else {
                        panic!("negative roundtrip result must not be reported as valid");
                    };
                    assert_eq!(
                        event.cache,
                        talkbank_transform::validation_runner::CacheUse::Hit
                    );
                    assert_eq!(reason, "Roundtrip failed (cached)");
                    // A cached verdict cannot fabricate a fresh diff.
                    assert_eq!(diff, None);
                    Phase::Finished
                }
                (Phase::Finished, ValidationEvent::Finished(RunEnding::Complete(stats))) => {
                    let stats = stats.snapshot();
                    assert_eq!(stats.total_files().get(), 1);
                    assert_eq!(stats.invalid_files(), 1);
                    assert_eq!(stats.roundtrip_failed(), 1);
                    assert_eq!(stats.cache_hits(), 1);
                    assert_eq!(
                        stats.valid_files()
                            + stats.internal_failures()
                            + stats.roundtrip_passed()
                            + stats.cache_misses(),
                        0
                    );
                    Phase::Closed
                }
                (_, event) => panic!("unexpected cached-failure event: {event:?}"),
            };
        }
        assert!(matches!(phase, Phase::Closed));
    }
}
