// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Concurrent and stress tests for cache, parser, and validation pipeline.
//!
//! These tests verify that:
//! - `CachePool` (Send + Sync) handles concurrent access without corruption
//! - `TreeSitterParser` (!Send + !Sync) works correctly one-per-thread
//! - `validate_directory_streaming` processes files in parallel and handles
//!   cancellation
//!
//! Tests marked `#[ignore]` are stress tests that take longer to run.
//! Run them explicitly with `cargo test --test concurrent_tests -- --ignored`.

use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use talkbank_parser::TreeSitterParser;
use talkbank_transform::{
    CachePool, ParserKind, RoundtripCheck, RunEnding, ValidationConfig, ValidationEvent,
    validate_directory_streaming,
};

/// Minimal valid CHAT file content for testing.
const VALID_CHAT: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thello world .
%mor:\tn|hello n|world .
@End
";

/// A second valid CHAT variant (different utterance content).
const VALID_CHAT_B: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tMOT Mother
@ID:\teng|corpus|MOT|||||Mother|||
*MOT:\tgoodbye world .
%mor:\tn|goodbye n|world .
@End
";

/// Invalid CHAT content (missing required headers).
const INVALID_CHAT: &str = "\
@UTF8
@Begin
*CHI:\thello .
@End
";

/// Create a temp .cha file with the given content, returning its path.
fn write_temp_cha(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).expect("create temp file");
    f.write_all(content.as_bytes())
        .expect("write temp file content");
    path
}

/// Four workers, the pool size the concurrency tests run with. Evaluated at
/// compile time, so a zero here would be a build error, not a test failure.
const FOUR_WORKERS: std::num::NonZeroUsize = std::num::NonZeroUsize::new(4).expect("4 is not zero");

/// Helper to build a `ValidationConfig` with cache disabled and fixed job count.
fn test_config(jobs: std::num::NonZeroUsize) -> ValidationConfig {
    ValidationConfig {
        jobs: Some(jobs),
        alignment: talkbank_model::validation::AlignmentValidation::Structure,
        roundtrip: RoundtripCheck::Skip,
        error_limit: talkbank_transform::ErrorLimit::Unlimited,
        parser_kind: ParserKind::TreeSitter,
        rules: talkbank_model::RuleSelection::new(),
        presentation: talkbank_transform::PresentationPolicy::new(),
    }
}

// =============================================================================
// Cache concurrency (5 tests)
// =============================================================================

/// Spawn 4 threads, each writing 50 different entries to the same in-memory
/// cache. All writes should succeed without corruption or panic.
#[test]
fn concurrent_cache_writes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = Arc::new(
        CachePool::in_memory(talkbank_cache::CacheIdentity::new(
            talkbank_cache::RulesVersion::current(),
            talkbank_model::ParserKind::TreeSitter,
        ))
        .expect("create in-memory cache"),
    );

    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let cache = Arc::clone(&cache);
            let dir_path = dir.path().to_path_buf();
            thread::spawn(move || {
                for i in 0..50 {
                    let name = format!("t{thread_id}_f{i}.cha");
                    let path = write_temp_cha(&dir_path, &name, VALID_CHAT);
                    crate::cache_shim::set_validation(
                        &cache,
                        &path,
                        talkbank_model::validation::AlignmentValidation::Structure,
                        talkbank_transform::CacheOutcome::Valid,
                    )
                    .expect("set_validation should not fail");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("worker thread should not panic");
    }

    // Verify: spot-check a few entries from each thread.
    for thread_id in 0..4u32 {
        let name = format!("t{thread_id}_f0.cha");
        let path = dir.path().join(name);
        let result = crate::cache_shim::get_validation(
            &cache,
            &path,
            talkbank_model::validation::AlignmentValidation::Structure,
        );
        assert_eq!(
            result,
            Some(talkbank_transform::CacheOutcome::Valid),
            "Entry written by thread {thread_id} should be readable"
        );
    }
}

/// One thread writes entries while another reads concurrently. Reads should
/// return either `None` (miss) or the correct value, never garbage.
#[test]
fn concurrent_cache_read_write() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = Arc::new(
        CachePool::in_memory(talkbank_cache::CacheIdentity::new(
            talkbank_cache::RulesVersion::current(),
            talkbank_model::ParserKind::TreeSitter,
        ))
        .expect("create in-memory cache"),
    );

    // Pre-create all files so the writer can use them.
    let entry_count = 100;
    let paths: Vec<_> = (0..entry_count)
        .map(|i| write_temp_cha(dir.path(), &format!("rw_{i}.cha"), VALID_CHAT))
        .collect();

    let writer_cache = Arc::clone(&cache);
    let writer_paths = paths.clone();
    let writer = thread::spawn(move || {
        for (i, path) in writer_paths.iter().enumerate() {
            let verdict = crate::cache_shim::alternating(i);
            crate::cache_shim::set_validation(
                &writer_cache,
                path,
                talkbank_model::validation::AlignmentValidation::Structure,
                verdict,
            )
            .expect("write should succeed");
        }
    });

    let reader_cache = Arc::clone(&cache);
    let reader_paths = paths.clone();
    let reader = thread::spawn(move || {
        let mut none_count = 0usize;
        let mut some_count = 0usize;
        // Read all entries multiple times; every non-None result must be
        // the correct boolean for that index.
        for _round in 0..3 {
            for (i, path) in reader_paths.iter().enumerate() {
                match crate::cache_shim::get_validation(
                    &reader_cache,
                    path,
                    talkbank_model::validation::AlignmentValidation::Structure,
                ) {
                    None => none_count += 1,
                    Some(val) => {
                        let expected = crate::cache_shim::alternating(i);
                        assert_eq!(
                            val, expected,
                            "Read garbage: index {i} expected {expected:?}, got {val:?}"
                        );
                        some_count += 1;
                    }
                }
            }
        }
        (none_count, some_count)
    });

    writer.join().expect("writer should not panic");
    let (none_count, some_count) = reader.join().expect("reader should not panic");

    // At least some reads should have seen values (the writer is fast).
    // We cannot assert exact counts because of scheduling, but we can
    // assert that the total is correct.
    assert_eq!(
        none_count + some_count,
        entry_count * 3,
        "Total read attempts should equal entry_count * rounds"
    );
}

/// One thread writes entries while another calls `clear_all` mid-way.
/// Neither thread should panic or leave the cache in a corrupted state.
#[test]
fn concurrent_cache_clear_during_write() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = Arc::new(
        CachePool::in_memory(talkbank_cache::CacheIdentity::new(
            talkbank_cache::RulesVersion::current(),
            talkbank_model::ParserKind::TreeSitter,
        ))
        .expect("create in-memory cache"),
    );

    let writer_cache = Arc::clone(&cache);
    let dir_path = dir.path().to_path_buf();
    let writer = thread::spawn(move || {
        for i in 0..200 {
            let path = write_temp_cha(&dir_path, &format!("cw_{i}.cha"), VALID_CHAT);
            // Writes may fail if clear_all is running concurrently; that is
            // acceptable as long as there is no panic or corruption.
            let _ = crate::cache_shim::set_validation(
                &writer_cache,
                &path,
                talkbank_model::validation::AlignmentValidation::Structure,
                talkbank_transform::CacheOutcome::Valid,
            );
        }
    });

    let clearer_cache = Arc::clone(&cache);
    let clearer = thread::spawn(move || {
        for _ in 0..10 {
            clearer_cache
                .clear(&talkbank_transform::CacheScope::All)
                .expect("clear_all should not fail");
            // Yield to let the writer make progress between clears.
            thread::yield_now();
        }
    });

    writer.join().expect("writer should not panic");
    clearer.join().expect("clearer should not panic");

    // The cache should still be usable after concurrent clear + write.
    let stats = cache
        .stats()
        .expect("stats should work after concurrent ops");
    // Stats.total_entries can be anything (depending on timing), but must
    // not be negative or cause an error.
    assert!(
        stats.total_entries < 300,
        "Entry count should be bounded (got {})",
        stats.total_entries,
    );
}

/// Multiple threads write entries, then check stats. The total should be
/// consistent (equal to all surviving writes).
#[test]
fn concurrent_cache_stats_consistency() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = Arc::new(
        CachePool::in_memory(talkbank_cache::CacheIdentity::new(
            talkbank_cache::RulesVersion::current(),
            talkbank_model::ParserKind::TreeSitter,
        ))
        .expect("create in-memory cache"),
    );

    let threads_count = 4u32;
    let entries_per_thread = 25u32;

    let handles: Vec<_> = (0..threads_count)
        .map(|tid| {
            let cache = Arc::clone(&cache);
            let dir_path = dir.path().to_path_buf();
            thread::spawn(move || {
                for i in 0..entries_per_thread {
                    let name = format!("stat_t{tid}_f{i}.cha");
                    let path = write_temp_cha(&dir_path, &name, VALID_CHAT);
                    crate::cache_shim::set_validation(
                        &cache,
                        &path,
                        talkbank_model::validation::AlignmentValidation::Structure,
                        talkbank_transform::CacheOutcome::Valid,
                    )
                    .expect("set_validation should succeed");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let stats = cache.stats().expect("stats should succeed");
    let expected = (threads_count * entries_per_thread) as usize;
    assert_eq!(
        stats.total_entries, expected,
        "Total entries should equal sum of all thread writes"
    );
}

/// 4 threads each writing to non-overlapping path prefixes. All entries
/// should be independently readable after all threads complete.
#[test]
fn concurrent_cache_different_paths() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = Arc::new(
        CachePool::in_memory(talkbank_cache::CacheIdentity::new(
            talkbank_cache::RulesVersion::current(),
            talkbank_model::ParserKind::TreeSitter,
        ))
        .expect("create in-memory cache"),
    );

    let threads_count = 4u32;
    let entries_per_thread = 20u32;

    // Create subdirectories for each thread (non-overlapping prefixes).
    let subdirs: Vec<_> = (0..threads_count)
        .map(|tid| {
            let subdir = dir.path().join(format!("corpus_{tid}"));
            std::fs::create_dir_all(&subdir).expect("create subdir");
            subdir
        })
        .collect();

    let handles: Vec<_> = (0..threads_count)
        .map(|tid| {
            let cache = Arc::clone(&cache);
            let subdir = subdirs[tid as usize].clone();
            thread::spawn(move || {
                for i in 0..entries_per_thread {
                    // A mix: every third file invalid.
                    let verdict = every_third_invalid(i);
                    let path = write_temp_cha(&subdir, &format!("f{i}.cha"), VALID_CHAT);
                    crate::cache_shim::set_validation(
                        &cache,
                        &path,
                        talkbank_model::validation::AlignmentValidation::Structure,
                        verdict,
                    )
                    .expect("set_validation should succeed");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should not panic");
    }

    // Verify all entries from all threads.
    for tid in 0..threads_count {
        for i in 0..entries_per_thread {
            let expected = every_third_invalid(i);
            let path = subdirs[tid as usize].join(format!("f{i}.cha"));
            let result = crate::cache_shim::get_validation(
                &cache,
                &path,
                talkbank_model::validation::AlignmentValidation::Structure,
            );
            assert_eq!(
                result,
                Some(expected),
                "Thread {tid}, file {i}: expected {expected:?}"
            );
        }
    }
}

// =============================================================================
// Parser thread-per-file (4 tests)
// =============================================================================

/// Spawn 4 threads, each creates its own `TreeSitterParser`, parses the same
/// CHAT content. All should produce equivalent `ChatFile` results.
#[test]
fn parser_per_thread_deterministic() {
    let handles: Vec<_> = (0..4)
        .map(|_| {
            thread::spawn(|| {
                let parser = TreeSitterParser::new().expect("parser should initialize");
                let result = parser.parse_chat_file(VALID_CHAT);
                // Return the number of utterances as a determinism check.
                let chat_file = result.expect_built();
                chat_file.lines.len()
            })
        })
        .collect();

    let results: Vec<usize> = handles
        .into_iter()
        .map(|h: thread::JoinHandle<usize>| h.join().expect("thread should not panic"))
        .collect();

    // All threads should produce the same structure.
    let first = results[0];
    for (i, count) in results.iter().enumerate() {
        assert_eq!(
            *count, first,
            "Thread {i} produced {count} utterances, expected {first}"
        );
    }
}

/// Each thread parses a different CHAT string. All succeed independently.
#[test]
fn parser_per_thread_different_files() {
    let inputs = [VALID_CHAT, VALID_CHAT_B, VALID_CHAT, VALID_CHAT_B];

    let handles: Vec<_> = inputs
        .iter()
        .enumerate()
        .map(|(i, input)| {
            let input = input.to_string();
            thread::spawn(move || {
                let parser = TreeSitterParser::new().expect("parser should initialize");
                let result = parser.parse_chat_file(&input);
                assert!(
                    result.is_built(),
                    "Thread {i} should parse valid CHAT without error"
                );
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should not panic");
    }
}

/// One thread creates one parser and parses 100 different strings
/// sequentially. No leaks or crashes.
#[test]
fn parser_many_sequential_parses() {
    let parser = TreeSitterParser::new().expect("parser should initialize");

    for i in 0..100 {
        // Alternate between two valid inputs to exercise parser reuse.
        let input = if i % 2 == 0 { VALID_CHAT } else { VALID_CHAT_B };
        let result = parser.parse_chat_file(input);
        assert!(
            result.is_built(),
            "Parse #{i} should succeed (parser reuse test)"
        );
    }
}

/// One thread parses invalid CHAT (should get errors), another parses valid
/// CHAT concurrently. The invalid thread's errors must not leak into the
/// valid thread's results.
#[test]
fn parser_error_isolation() {
    let valid_handle = thread::spawn(|| {
        let parser = TreeSitterParser::new().expect("parser init");
        for _ in 0..20 {
            let result = parser.parse_chat_file(VALID_CHAT);
            assert!(
                result.is_built(),
                "Valid CHAT should always parse without error"
            );
        }
    });

    let invalid_handle = thread::spawn(|| {
        let parser = TreeSitterParser::new().expect("parser init");
        for _ in 0..20 {
            let result = parser.parse_chat_file(INVALID_CHAT);
            // Invalid CHAT should either build a recovered ChatFile (with
            // diagnostics) or come back Unbuildable, either way, it should
            // not panic.
            match result {
                talkbank_parser::ParseProduct::Built { .. } => {
                    // Parser may recover and return a ChatFile; that is fine.
                }
                talkbank_parser::ParseProduct::Unbuildable { diagnostics } => {
                    assert!(
                        !diagnostics.is_empty(),
                        "Unbuildable result should contain at least one diagnostic"
                    );
                }
            }
        }
    });

    valid_handle.join().expect("valid thread should not panic");
    invalid_handle
        .join()
        .expect("invalid thread should not panic");
}

// =============================================================================
// Pipeline concurrency (3 tests)
// =============================================================================

/// Use `validate_directory_streaming` with a temp dir of 10 .cha files.
/// Verify all files are processed and a `Complete` event arrives.
#[test]
fn pipeline_parallel_validate() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file_count = 10;
    for i in 0..file_count {
        write_temp_cha(dir.path(), &format!("file_{i}.cha"), VALID_CHAT);
    }

    let config = test_config(FOUR_WORKERS);
    let (events, _cancel) = validate_directory_streaming(
        dir.path(),
        &talkbank_transform::ValidationRun::uncached(config.clone()),
    );

    let mut started = false;
    let mut file_complete_count = 0usize;
    let mut finished = false;

    for event in events {
        match event {
            ValidationEvent::Discovering => {}
            ValidationEvent::Started { total_files } => {
                assert_eq!(total_files, file_count, "Should discover all files");
                started = true;
            }
            ValidationEvent::FileComplete(_) => {
                file_complete_count += 1;
            }
            ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                let stats = stats.snapshot();
                finished = true;
                assert_eq!(
                    stats.total_files().get(),
                    file_count,
                    "Final stats should reflect all files"
                );
            }
            // A healthy parallel run must cover every file it discovered and
            // must not die. Every other terminal is named explicitly rather
            // than swallowed by a wildcard, so a regression that starts
            // losing files under concurrency fails HERE instead of passing as
            // "well, it terminated".
            ValidationEvent::Finished(RunEnding::Stopped { stats, reason, .. }) => {
                let unprocessed = stats.missing_files();
                panic!("nobody stopped this run, yet {unprocessed} files were left ({reason:?})");
            }
            ValidationEvent::Finished(RunEnding::Incomplete { stats, .. }) => {
                let lost_files = stats.missing_files();
                let stats = stats.snapshot();
                panic!(
                    "parallel run lost {lost_files} of {} files",
                    stats.total_files().get()
                );
            }
            ValidationEvent::Finished(RunEnding::Aborted(reason)) => {
                panic!("parallel run aborted: {reason:?}");
            }
            ValidationEvent::Finished(RunEnding::NothingFound) => {
                panic!("the run found none of the {file_count} files");
            }
        }
    }

    assert!(started, "Should have received Started event");
    assert!(finished, "Should have received Complete event");
    assert_eq!(
        file_complete_count, file_count,
        "Should receive FileComplete for every file"
    );
}

/// Start validation of 10 files and cancel after 3 have completed. The run
/// ends `Stopped` with the caller's reason when files were left, or
/// `Complete` when the one worker had already reached the end: either way
/// it ends, and never as a `Complete` that left files unvalidated.
#[test]
fn pipeline_cancel_midway() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file_count = 10;
    for i in 0..file_count {
        write_temp_cha(dir.path(), &format!("cancel_{i}.cha"), VALID_CHAT);
    }

    // Use 1 job to make cancellation timing more predictable.
    let config = test_config(std::num::NonZeroUsize::MIN);
    let (events, canceller) = validate_directory_streaming(
        dir.path(),
        &talkbank_transform::ValidationRun::uncached(config.clone()),
    );

    let mut file_complete_count = 0usize;
    let mut terminal = None;

    for event in events {
        match event {
            ValidationEvent::FileComplete(_) => {
                file_complete_count += 1;
                if file_complete_count == 3 {
                    canceller.cancel();
                }
            }
            ValidationEvent::Discovering | ValidationEvent::Started { .. } => {}
            ValidationEvent::Finished(ending) => terminal = Some(ending),
        }
    }

    match terminal.expect("the run must end with an ending") {
        RunEnding::Stopped { stats, reason } => {
            let unprocessed = stats.missing_files();
            let stats = stats.snapshot();
            assert_eq!(reason, talkbank_transform::CancelReason::Requested);
            assert_eq!(stats.files_accounted_for() + unprocessed.get(), file_count);
        }
        RunEnding::Complete(stats) => {
            let stats = stats.snapshot();
            assert_eq!(stats.files_accounted_for(), file_count);
        }
        other => panic!("a cancelled run must be Stopped or Complete, got {other:?}"),
    }
}

/// `validate_directory_streaming` on an empty directory. Should get
/// `Started { 0 }` then `NothingFound` immediately.
#[test]
fn pipeline_empty_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let config = test_config(FOUR_WORKERS);
    let (events, _cancel) = validate_directory_streaming(
        dir.path(),
        &talkbank_transform::ValidationRun::uncached(config.clone()),
    );

    let mut started_total = None;
    let mut finished = false;

    for event in events {
        match event {
            ValidationEvent::Discovering => {}
            ValidationEvent::Started { total_files } => {
                started_total = Some(total_files);
            }
            ValidationEvent::Finished(ending) => {
                finished = true;
                assert_eq!(ending, RunEnding::NothingFound, "Empty dir finds nothing");
            }
            _ => {
                panic!("Unexpected event for empty directory: {event:?}");
            }
        }
    }

    assert_eq!(
        started_total,
        Some(0),
        "Should receive Started with 0 files"
    );
    assert!(finished, "Should receive Finished event");
}

// =============================================================================
// Stress tests (3 tests, #[ignore])
// =============================================================================

/// Temp dir with 100 minimal .cha files. `validate_directory_streaming`
/// should complete without hang or crash.
#[test]
#[ignore = "a stress test: minutes of wall clock for a property the \
           ordinary concurrent tests above already pin at a smaller size. Run \
           with --ignored."]
fn stress_100_files_parallel() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file_count = 100;
    for i in 0..file_count {
        write_temp_cha(dir.path(), &format!("stress_{i}.cha"), VALID_CHAT);
    }

    let config = test_config(FOUR_WORKERS);
    let (events, _cancel) = validate_directory_streaming(
        dir.path(),
        &talkbank_transform::ValidationRun::uncached(config.clone()),
    );

    let mut file_complete_count = 0usize;
    let mut finished = false;

    for event in events {
        match event {
            ValidationEvent::FileComplete(_) => {
                file_complete_count += 1;
            }
            ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                let stats = stats.snapshot();
                finished = true;
                assert_eq!(
                    stats.total_files().get(),
                    file_count,
                    "Should process all 100 files"
                );
            }
            _ => {}
        }
    }

    assert!(finished, "Should receive Finished event");
    assert_eq!(
        file_complete_count, file_count,
        "All 100 files should complete"
    );
}

/// Write 1000 entries to in-memory cache, read them all back.
#[test]
#[ignore = "a stress test: minutes of wall clock for a property the \
           ordinary concurrent tests above already pin at a smaller size. Run \
           with --ignored."]
fn stress_cache_1000_entries() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = CachePool::in_memory(talkbank_cache::CacheIdentity::new(
        talkbank_cache::RulesVersion::current(),
        talkbank_model::ParserKind::TreeSitter,
    ))
    .expect("create in-memory cache");

    let entry_count = 1000;
    let paths: Vec<_> = (0..entry_count)
        .map(|i| write_temp_cha(dir.path(), &format!("stress_{i}.cha"), VALID_CHAT))
        .collect();

    // Write all entries.
    for (i, path) in paths.iter().enumerate() {
        let verdict = crate::cache_shim::alternating(i);
        crate::cache_shim::set_validation(
            &cache,
            path,
            talkbank_model::validation::AlignmentValidation::Structure,
            verdict,
        )
        .expect("set_validation should succeed");
    }

    // Read them all back.
    for (i, path) in paths.iter().enumerate() {
        let expected = crate::cache_shim::alternating(i);
        let result = crate::cache_shim::get_validation(
            &cache,
            path,
            talkbank_model::validation::AlignmentValidation::Structure,
        );
        assert_eq!(
            result,
            Some(expected),
            "Entry {i} should read back correctly"
        );
    }

    let stats = cache.stats().expect("stats should succeed");
    assert_eq!(
        stats.total_entries, entry_count,
        "Stats should reflect all 1000 entries"
    );
}

/// Create and drop 50 parsers rapidly. No resource leaks or crashes.
#[test]
#[ignore = "a stress test: minutes of wall clock for a property the \
           ordinary concurrent tests above already pin at a smaller size. Run \
           with --ignored."]
fn stress_parser_rapid_creation() {
    for i in 0..50 {
        let parser = TreeSitterParser::new().expect("parser should initialize");
        // Parse one file to ensure the parser is fully initialized and usable.
        let result = parser.parse_chat_file(VALID_CHAT);
        assert!(
            result.is_built(),
            "Parser #{i} should parse valid CHAT successfully"
        );
        // Parser is dropped here; resources should be freed cleanly.
    }
}

/// Test data: every third file invalid, the rest valid.
fn every_third_invalid(index: u32) -> talkbank_transform::CacheOutcome {
    match index % 3 {
        0 => talkbank_transform::CacheOutcome::Invalid,
        _ => talkbank_transform::CacheOutcome::Valid,
    }
}
