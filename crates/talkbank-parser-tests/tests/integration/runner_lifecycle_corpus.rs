//! Reference-file runner termination contracts at external callback boundaries.

use super::{
    CacheMode, CacheOutcome, FileStatus, Path, RoundtripVerdict, RunCoverage, StreamPhase,
    ValidationCache, ValidationConfig, ValidationEvent, validate_files_streaming, workspace_root,
};

#[test]
fn reference_receiver_disconnect_stops_before_the_next_file() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;
    use talkbank_cache::CachePool;

    enum Route {
        Fresh,
        ReadFailure,
        CachedValidation,
        CachedRoundtrip,
    }

    struct PausedCache {
        entered: mpsc::SyncSender<()>,
        resume: Mutex<Option<mpsc::Receiver<()>>>,
        stopped: mpsc::SyncSender<(usize, usize, usize)>,
        lookups: AtomicUsize,
        writes: AtomicUsize,
        roundtrip_lookups: AtomicUsize,
        store: Option<Arc<CachePool>>,
    }
    impl ValidationCache for PausedCache {
        fn get(&self, path: &Path, alignment: bool) -> Option<CacheOutcome> {
            self.lookups.fetch_add(1, Ordering::Relaxed);
            let resume = self
                .resume
                .lock()
                .expect("pause lock")
                .take()
                .expect("disconnected run must not begin another lookup");
            self.entered.send(()).expect("awaited lookup");
            resume
                .recv_timeout(Duration::from_secs(30))
                .expect("resume in-flight file");
            self.store
                .as_ref()
                .and_then(|store| ValidationCache::get(&**store, path, alignment))
        }
        fn set(&self, _: &Path, _: bool, outcome: CacheOutcome) -> Result<(), String> {
            assert_eq!(outcome, CacheOutcome::Valid);
            self.writes.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn get_roundtrip(&self, path: &Path, alignment: bool) -> Option<CacheOutcome> {
            self.roundtrip_lookups.fetch_add(1, Ordering::Relaxed);
            self.store
                .as_ref()
                .and_then(|store| ValidationCache::get_roundtrip(&**store, path, alignment))
        }
    }
    impl Drop for PausedCache {
        fn drop(&mut self) {
            // No test-owned Arc remains: this reports release by the worker
            // and orchestrator, not just a failed event send from one thread.
            let _ = self.stopped.send((
                self.lookups.load(Ordering::Relaxed),
                self.writes.load(Ordering::Relaxed),
                self.roundtrip_lookups.load(Ordering::Relaxed),
            ));
        }
    }
    let files = [
        "core/basic-conversation.cha",
        "content/terminators-standard.cha",
    ]
    .map(|name| workspace_root().join("corpus/reference").join(name));
    assert!(files.iter().all(|path| path.is_file()));
    for route in [
        Route::Fresh,
        Route::ReadFailure,
        Route::CachedValidation,
        Route::CachedRoundtrip,
    ] {
        let config = ValidationConfig {
            jobs: Some(1),
            cache: CacheMode::Enabled,
            roundtrip: matches!(route, Route::CachedRoundtrip),
            ..Default::default()
        };
        let store = match route {
            Route::Fresh | Route::ReadFailure => None,
            Route::CachedValidation | Route::CachedRoundtrip => {
                let store = Arc::new(
                    CachePool::in_memory(config.cache_identity()).expect("isolated cache"),
                );
                // Warm entries come only from completed reference-file runs.
                let (events, _cancel) =
                    validate_files_streaming(files.to_vec(), &config, Some(store.clone()));
                let mut finished = false;
                for event in events {
                    if let ValidationEvent::Finished(stats) = event {
                        assert_eq!(stats.coverage(), RunCoverage::Complete);
                        assert_eq!(stats.valid_files, files.len());
                        assert_eq!(
                            stats.roundtrip_passed,
                            if config.roundtrip { files.len() } else { 0 }
                        );
                        finished = true;
                    }
                }
                assert!(finished);
                Some(store)
            }
        };
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = mpsc::sync_channel(1);
        let (stopped_tx, stopped_rx) = mpsc::sync_channel(1);
        let cache = Arc::new(PausedCache {
            entered: entered_tx,
            resume: Mutex::new(Some(resume_rx)),
            stopped: stopped_tx,
            lookups: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
            roundtrip_lookups: AtomicUsize::new(0),
            store,
        });
        // The same real reference bytes with an external encoding defect
        // exercise read failure, not an invented CHAT diagnostic. Keep the
        // test-owned file alive until the worker releases its cache.
        let unreadable = if matches!(route, Route::ReadFailure) {
            use std::io::Write;
            let mut file = tempfile::Builder::new()
                .prefix("disconnect-encoding-")
                .suffix(".cha")
                .tempfile()
                .expect("test-owned input");
            file.write_all(&std::fs::read(&files[0]).expect("reference bytes"))
                .expect("copy reference");
            file.write_all(&[0xff]).expect("external encoding defect");
            file.flush().expect("flush input");
            Some(file)
        } else {
            None
        };
        let inputs = match &unreadable {
            Some(file) => vec![file.path().to_path_buf(), files[1].clone()],
            None => files.to_vec(),
        };
        let (events, _cancel) = validate_files_streaming(inputs, &config, Some(cache));
        // Timeouts are coarse hang guards; messages establish the ordering.
        entered_rx
            .recv_timeout(Duration::from_secs(30))
            .expect("first lookup reached");
        drop(events);
        resume_tx
            .send(())
            .expect("resume after receiver disconnection");
        assert_eq!(
            stopped_rx
                .recv_timeout(Duration::from_secs(30))
                .expect("runner releases cache"),
            match route {
                Route::Fresh => (1, 1, 0),
                Route::ReadFailure => (1, 0, 0),
                Route::CachedValidation => (1, 0, 0),
                Route::CachedRoundtrip => (1, 0, 1),
            },
            "disconnect must stop before another file or an unnecessary cache write"
        );
    }
}

#[test]
fn reference_worker_failure_cannot_report_a_completed_validation() {
    struct UnwindingCache;
    impl ValidationCache for UnwindingCache {
        fn get(&self, _: &Path, _: bool) -> Option<CacheOutcome> {
            // Deliberate external callback failure. The runner, not the CHAT
            // parser, must account for the abandoned reference-file job.
            panic!("injected cache callback unwind");
        }
        fn set(&self, _: &Path, _: bool, _: CacheOutcome) -> Result<(), String> {
            panic!("failed lookup cannot reach a cache write");
        }
    }
    let path = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    assert!(path.is_file());
    let config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        ..Default::default()
    };
    let (events, _cancel) = validate_files_streaming(
        vec![path],
        &config,
        Some(std::sync::Arc::new(UnwindingCache)),
    );
    let mut phase = StreamPhase::Discovery;
    for event in events {
        match event {
            ValidationEvent::Discovering => {
                assert!(matches!(phase, StreamPhase::Discovery));
                phase = StreamPhase::Start;
            }
            ValidationEvent::Started { total_files } => {
                assert!(matches!(phase, StreamPhase::Start));
                assert_eq!(total_files, 1);
                phase = StreamPhase::Running;
            }
            ValidationEvent::FinishedIncomplete { stats, lost_files } => {
                assert!(matches!(phase, StreamPhase::Running));
                assert_eq!(lost_files, 1);
                assert_eq!(stats.total_files, 1);
                assert_eq!(stats.files_accounted_for(), 0);
                assert_eq!(stats.valid_files, 0);
                assert_eq!(stats.invalid_files, 0);
                assert_eq!(stats.parse_errors, 0);
                assert_eq!(stats.internal_failures, 0);
                assert!(!stats.cancelled);
                assert_eq!(stats.coverage(), RunCoverage::Lost { lost_files: 1 });
                phase = StreamPhase::Finished;
            }
            other => panic!("abandoned file must not acquire a validity verdict: {other:?}"),
        }
    }
    assert!(
        matches!(phase, StreamPhase::Finished),
        "worker failure needs a terminal result"
    );
}

#[test]
fn reference_cancellation_accounts_for_unprocessed_files() {
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    struct PausedCacheMiss {
        entered: mpsc::SyncSender<()>,
        resume: Mutex<Option<mpsc::Receiver<()>>>,
    }
    impl ValidationCache for PausedCacheMiss {
        fn get(&self, _: &Path, _: bool) -> Option<CacheOutcome> {
            let resume = self
                .resume
                .lock()
                .expect("pause lock")
                .take()
                .expect("cancelled worker must not start another cache lookup");
            self.entered.send(()).expect("test awaits first lookup");
            // Only a coarse hang guard; synchronization, not elapsed time,
            // establishes the ordering of lookup, cancellation and resumption.
            resume
                .recv_timeout(Duration::from_secs(30))
                .expect("test resumes lookup");
            None
        }
        fn set(&self, _: &Path, _: bool, _: CacheOutcome) -> Result<(), String> {
            Ok(())
        }
    }
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = mpsc::sync_channel(1);
    let cache = Arc::new(PausedCacheMiss {
        entered: entered_tx,
        resume: Mutex::new(Some(resume_rx)),
    });
    let first = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    let second = workspace_root().join("corpus/reference/content/terminators-standard.cha");
    assert!(first.is_file() && second.is_file());
    let config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        ..Default::default()
    };
    let (events, cancel) =
        validate_files_streaming(vec![first.clone(), second], &config, Some(cache));
    entered_rx
        .recv_timeout(Duration::from_secs(30))
        .expect("worker reaches cache boundary");
    cancel.send(()).expect("runner accepts cancellation");
    resume_tx.send(()).expect("release in-flight reference job");

    let mut phase = StreamPhase::Discovery;
    let mut completed = 0;
    for event in events {
        match event {
            ValidationEvent::Discovering => {
                assert!(matches!(phase, StreamPhase::Discovery));
                phase = StreamPhase::Start;
            }
            ValidationEvent::Started { total_files } => {
                assert!(matches!(phase, StreamPhase::Start));
                assert_eq!(total_files, 2);
                phase = StreamPhase::Running;
            }
            ValidationEvent::FileComplete(event) => {
                assert!(matches!(phase, StreamPhase::Running));
                assert_eq!(event.path, first);
                assert!(matches!(
                    event.status,
                    FileStatus::Valid {
                        cache_hit: false,
                        roundtrip: RoundtripVerdict::NotRequested
                    }
                ));
                completed += 1;
                assert_eq!(completed, 1, "at most the in-flight file completes");
            }
            ValidationEvent::Finished(stats) => {
                assert!(matches!(phase, StreamPhase::Running));
                assert!(stats.cancelled);
                assert_eq!(stats.total_files, 2);
                assert_eq!(stats.files_accounted_for(), completed);
                assert_eq!(
                    stats.coverage(),
                    RunCoverage::Cancelled {
                        unprocessed_files: 2 - completed
                    }
                );
                assert_eq!(stats.cache_hit_rate(), 0.0);
                phase = StreamPhase::Finished;
            }
            other => panic!("unexpected cancellation event: {other:?}"),
        }
    }
    assert!(
        matches!(phase, StreamPhase::Finished),
        "cancellation requires a terminal result"
    );
}
