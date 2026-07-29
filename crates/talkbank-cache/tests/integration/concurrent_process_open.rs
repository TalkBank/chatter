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

//! Cross-PROCESS concurrency safety of cache initialization.
//!
//! The sibling `concurrent_open` test races many threads inside one process.
//! This test races many PROCESSES, which is the deployment reality: multiple
//! `chatter` invocations (or, under a process-per-test runner, multiple test
//! binaries) can open the same cache directory simultaneously. sqlx's SQLite
//! migrator has no cross-connection lock (`Migrate::lock` is a no-op for
//! SQLite), so without our own serialization, openers racing a FRESH database
//! both apply migration version 1 and the loser dies with `UNIQUE constraint
//! failed: _sqlx_migrations.version` (or with a first-connection WAL setup
//! collision). The parent test asserts BOTH properties: every opener
//! succeeds, and every opener finishes within a hard deadline (an
//! initialization that wedges fails the test instead of hanging the suite).
//!
//! Mechanism: the parent re-executes its own test binary (libtest harness
//! interface) to run the `#[ignore]`d child entry point below in N separate
//! processes, synchronizes their start on a "go" file so they hit the
//! fresh-database initialization window together, and collects results.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Env var carrying the cache directory the child must open.
const CHILD_CACHE_DIR_ENV: &str = "TALKBANK_CACHE_RACE_CHILD_CACHE_DIR";
/// Env var carrying the probe .cha file the child must set/get.
const CHILD_PROBE_ENV: &str = "TALKBANK_CACHE_RACE_CHILD_PROBE";
/// Env var carrying the path of the start-synchronization ("go") file.
const CHILD_GO_FILE_ENV: &str = "TALKBANK_CACHE_RACE_CHILD_GO_FILE";

/// How long a child waits for the go file before giving up.
const CHILD_GO_DEADLINE: Duration = Duration::from_secs(20);
/// How long the parent waits for all children in one round before declaring
/// a wedge. Generous: a healthy round completes in well under a second.
const ROUND_DEADLINE: Duration = Duration::from_secs(60);

/// Child-process entry point. Not a standalone test: it is spawned by
/// `concurrent_process_opens_on_fresh_cache_dir_all_succeed` with the env
/// vars above. When run without them (e.g. someone invoking `--ignored`
/// suites by hand) it is a no-op success.
#[test]
#[ignore = "child-process entry point spawned by concurrent_process_opens_on_fresh_cache_dir_all_succeed"]
fn race_child_open_cache() {
    use talkbank_cache::{CacheOutcome, CachePool, ValidationCache};

    let (Some(cache_dir), Some(probe), Some(go_file)) = (
        std::env::var_os(CHILD_CACHE_DIR_ENV),
        std::env::var_os(CHILD_PROBE_ENV),
        std::env::var_os(CHILD_GO_FILE_ENV),
    ) else {
        // Invoked outside the parent test harness: nothing to do.
        return;
    };
    let cache_dir = std::path::PathBuf::from(cache_dir);
    let probe = std::path::PathBuf::from(probe);
    let go_file = std::path::PathBuf::from(go_file);

    // Line up with the sibling processes: spin until the parent releases
    // the herd by creating the go file.
    let go_deadline = Instant::now() + CHILD_GO_DEADLINE;
    while !go_file.exists() {
        assert!(
            Instant::now() < go_deadline,
            "child never saw the go file at {}",
            go_file.display()
        );
        std::thread::sleep(Duration::from_micros(200));
    }

    // The race under test: open (create + migrate) the shared fresh cache.
    let cache = CachePool::with_directory(cache_dir).expect("concurrent cache open must succeed");

    // Exercise a write + read so the pool is actually usable, not merely
    // constructed.
    cache
        .set(probe.as_path(), false, CacheOutcome::Valid)
        .expect("cache set must succeed after concurrent open");
    match cache.get(probe.as_path(), false) {
        Some(CacheOutcome::Valid) => {}
        other => panic!("cache get returned {other:?} after concurrent open"),
    }
}

/// Spawn one child process running [`race_child_open_cache`] against the
/// given cache dir / probe / go file.
fn spawn_child(
    exe: &std::path::Path,
    cache_dir: &std::path::Path,
    probe: &std::path::Path,
    go_file: &std::path::Path,
) -> std::io::Result<Child> {
    Command::new(exe)
        // Standard libtest harness interface: run exactly the ignored
        // child entry point, nothing else.
        .args([
            "--exact",
            "race_child_open_cache",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD_CACHE_DIR_ENV, cache_dir)
        .env(CHILD_PROBE_ENV, probe)
        .env(CHILD_GO_FILE_ENV, go_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

#[test]
fn concurrent_process_opens_on_fresh_cache_dir_all_succeed() {
    /// Processes racing one fresh cache directory per round.
    const PROCESSES: usize = 8;
    /// Rounds, each against a brand-new cache directory (the race only
    /// exists on FIRST initialization, so every round needs a fresh dir).
    const ROUNDS: usize = 4;

    let exe = std::env::current_exe().expect("test binary path");

    for round in 0..ROUNDS {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let cache_dir = dir.path().join("cache");
        let go_file = dir.path().join("go");

        // The cache keys on a content fingerprint of the file, so the probe
        // must exist on disk for `set`/`get` to hash it.
        let probe = dir.path().join("probe.cha");
        std::fs::write(&probe, "@UTF8\n@Begin\n@End\n").expect("write probe");

        let mut children: Vec<Child> = (0..PROCESSES)
            .map(|i| {
                spawn_child(&exe, &cache_dir, &probe, &go_file)
                    .unwrap_or_else(|e| panic!("round {round}: spawn child {i}: {e}"))
            })
            .collect();

        // Release the herd only after every child is up, so they all hit
        // the fresh-database initialization window together.
        std::fs::write(&go_file, b"go").expect("write go file");

        // Bounded wait: a wedge (the 2026-07-22 failure mode) must FAIL the
        // test, never hang the suite.
        let deadline = Instant::now() + ROUND_DEADLINE;
        let mut failures: Vec<String> = Vec::new();
        let mut pending: Vec<(usize, Child)> = children.drain(..).enumerate().collect();
        while !pending.is_empty() {
            if Instant::now() >= deadline {
                for (i, child) in &mut pending {
                    let _ = child.kill();
                    failures.push(format!(
                        "round {round}: child {i} still running at deadline (wedged), killed"
                    ));
                }
                // Reap the killed children so no zombies outlive the test.
                for (_, child) in &mut pending {
                    let _ = child.wait();
                }
                break;
            }
            let mut still_pending = Vec::new();
            for (i, mut child) in pending {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if !status.success() {
                            let output = child
                                .wait_with_output()
                                .expect("collect output of finished child");
                            failures.push(format!(
                                "round {round}: child {i} failed ({status}):\n--- stdout ---\n{}\n--- stderr ---\n{}",
                                String::from_utf8_lossy(&output.stdout),
                                String::from_utf8_lossy(&output.stderr),
                            ));
                        }
                    }
                    Ok(None) => still_pending.push((i, child)),
                    Err(e) => failures.push(format!("round {round}: waiting on child {i}: {e}")),
                }
            }
            pending = still_pending;
            if !pending.is_empty() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        assert!(
            failures.is_empty(),
            "concurrent cross-process opens on a fresh cache dir must all succeed \
             within the deadline, but {} failed:\n{}",
            failures.len(),
            failures.join("\n"),
        );
    }
}
