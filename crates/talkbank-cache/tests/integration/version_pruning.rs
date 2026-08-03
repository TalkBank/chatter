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

//! Regression test for unbounded growth of the on-disk cache.
//!
//! # The defect this pins
//!
//! Every read binds the opening pool's [`RulesVersion`] into the SQL `WHERE`
//! clause, so a row written under any other version is UNREACHABLE BY
//! CONSTRUCTION: no query this binary can issue will ever match it again.
//! Nothing deleted those rows. The only cleanup was a 30-day age cutoff, which
//! answers a different question, so every release stranded a complete copy of
//! the corpus in the database. A real user cache measured 464,773 rows across
//! 88 distinct versions for a corpus of ~106,000 files, roughly 190 MB of the
//! 243 MB file being rows no reader could ever bind.
//!
//! # What "fixed" means
//!
//! Opening a cache prunes rows whose version no reader will bind, keeping the
//! opening version and ONE generation of grace (see
//! `version_prune::RetainedVersions` for why the predecessor is kept).
//! Reachable rows are never touched.

use std::io::Write as _;

use talkbank_cache::{CachePool, RulesVersion, SpaceReclaimed, VersionPruneOutcome};

/// A minimal but well-formed CHAT file. Content is irrelevant here (only cache
/// bookkeeping is under test), but the cache hashes the file from disk, so it
/// must exist.
const CHAT_CONTENT: &str = "@UTF8\n@Begin\n@End\n";

/// Write `content` to `dir/name` and return the path.
fn write_temp_cha(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).expect("create temp cha file");
    file.write_all(content.as_bytes())
        .expect("write temp cha content");
    path
}

/// Open a cache under `version`, write one validation row, and close it.
fn seed_version(cache_dir: &std::path::Path, file: &std::path::Path, version: &RulesVersion) {
    let cache =
        CachePool::with_directory_and_rules_version(cache_dir.to_path_buf(), version.clone())
            .expect("open cache to seed a version");
    cache
        .set_validation(file, false, true)
        .expect("write a validation row");
}

#[test]
fn opening_a_cache_drops_rows_no_reader_can_ever_bind() {
    let cache_dir = tempfile::tempdir().expect("create temp cache dir");
    let file_dir = tempfile::tempdir().expect("create temp file dir");
    let file_path = write_temp_cha(file_dir.path(), "sample.cha", CHAT_CONTENT);

    // Four generations of the same corpus, written oldest to newest. Each seed
    // opens the cache and therefore prunes, exactly as four successive chatter
    // releases would, so by the time the last one runs the older generations
    // have already been walking off the end of the two-version window.
    let ancient = RulesVersion::for_testing("0.4.0+rules.aaaa");
    let old = RulesVersion::for_testing("0.5.0+rules.bbbb");
    let previous = RulesVersion::for_testing("0.6.0+rules.cccc");
    let current = RulesVersion::for_testing("0.7.0+rules.dddd");

    for version in [&ancient, &old, &previous] {
        seed_version(cache_dir.path(), &file_path, version);
    }

    // Opening under the current version prunes what has fallen outside the
    // window: `old` is now two generations back.
    let cache =
        CachePool::with_directory_and_rules_version(cache_dir.path().to_path_buf(), current)
            .expect("open cache under the current version");

    match cache.version_prune() {
        VersionPruneOutcome::NothingUnreachable => {
            panic!("a stranded version was on disk and should have been pruned")
        }
        VersionPruneOutcome::Pruned(report) => {
            assert_eq!(
                report.versions_deleted(),
                1,
                "exactly the generation that fell outside the window goes"
            );
            assert_eq!(report.rows_deleted(), 1, "one row was seeded per version");
            match report.reclaimed() {
                SpaceReclaimed::Vacuumed { .. } => {}
                SpaceReclaimed::NotReclaimed(reason) => {
                    panic!("a file-backed cache with no competing process should vacuum: {reason}")
                }
            }
        }
    }

    // The retention window, stated as reachability rather than as counts: the
    // one grace generation is still warm, and everything older is gone for
    // good rather than merely invisible.
    let reachable = |version: &RulesVersion| {
        CachePool::with_directory_and_rules_version(cache_dir.path().to_path_buf(), version.clone())
            .expect("reopen cache")
            .get_validation(&file_path, false)
    };
    assert_eq!(
        reachable(&previous),
        Some(true),
        "one generation of grace is kept so a downgrade is not cold"
    );
    assert_eq!(
        reachable(&ancient),
        None,
        "a pruned version's rows must be gone, not merely invisible"
    );
    assert_eq!(
        reachable(&old),
        None,
        "the generation that fell outside the window must be gone too"
    );
}

#[test]
fn opening_a_cache_that_holds_only_reachable_rows_deletes_nothing() {
    let cache_dir = tempfile::tempdir().expect("create temp cache dir");
    let file_dir = tempfile::tempdir().expect("create temp file dir");
    let file_path = write_temp_cha(file_dir.path(), "sample.cha", CHAT_CONTENT);

    let previous = RulesVersion::for_testing("0.6.0+rules.cccc");
    let current = RulesVersion::for_testing("0.7.0+rules.dddd");
    seed_version(cache_dir.path(), &file_path, &previous);
    seed_version(cache_dir.path(), &file_path, &current);

    // Reopening finds exactly the current version plus its one grace
    // generation, so there is nothing to delete and no VACUUM to pay for.
    let cache =
        CachePool::with_directory_and_rules_version(cache_dir.path().to_path_buf(), current)
            .expect("reopen under the current version");
    match cache.version_prune() {
        VersionPruneOutcome::NothingUnreachable => {}
        VersionPruneOutcome::Pruned(report) => panic!(
            "nothing was unreachable, yet {} row(s) were deleted",
            report.rows_deleted()
        ),
    }
    assert_eq!(
        cache.get_validation(&file_path, false),
        Some(true),
        "the current version's own rows must survive its prune"
    );
}
