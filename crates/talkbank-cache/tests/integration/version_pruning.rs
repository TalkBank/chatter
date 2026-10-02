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
    let cache = CachePool::with_directory(
        cache_dir.to_path_buf(),
        talkbank_cache::CacheIdentity::new(version.clone(), talkbank_model::ParserKind::TreeSitter),
    )
    .expect("open cache to seed a version");
    crate::shim::set_validation(
        &cache,
        file,
        talkbank_model::validation::AlignmentValidation::Structure,
        talkbank_cache::CacheOutcome::Valid,
    )
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
    let cache = CachePool::with_directory(
        cache_dir.path().to_path_buf(),
        talkbank_cache::CacheIdentity::new(current, talkbank_model::ParserKind::TreeSitter),
    )
    .expect("open cache under the current version");

    match cache.version_prune() {
        VersionPruneOutcome::NothingUnreachable | VersionPruneOutcome::FreshDatabase => {
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
                SpaceReclaimed::VacuumedSizeUnknown => {
                    panic!("an ordinary cache file should be measurable around a VACUUM")
                }
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
        let cache = CachePool::with_directory(
            cache_dir.path().to_path_buf(),
            talkbank_cache::CacheIdentity::new(
                version.clone(),
                talkbank_model::ParserKind::TreeSitter,
            ),
        )
        .expect("reopen cache");
        crate::shim::get_validation(
            &cache,
            &file_path,
            talkbank_model::validation::AlignmentValidation::Structure,
        )
    };
    assert_eq!(
        reachable(&previous),
        Some(talkbank_cache::CacheOutcome::Valid),
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
    let cache = CachePool::with_directory(
        cache_dir.path().to_path_buf(),
        talkbank_cache::CacheIdentity::new(current, talkbank_model::ParserKind::TreeSitter),
    )
    .expect("reopen under the current version");
    match cache.version_prune() {
        VersionPruneOutcome::NothingUnreachable => {}
        VersionPruneOutcome::FreshDatabase => panic!("a directory cache ran its prune"),
        VersionPruneOutcome::Pruned(report) => panic!(
            "nothing was unreachable, yet {} row(s) were deleted",
            report.rows_deleted()
        ),
    }
    assert_eq!(
        crate::shim::get_validation(
            &cache,
            &file_path,
            talkbank_model::validation::AlignmentValidation::Structure
        ),
        Some(talkbank_cache::CacheOutcome::Valid),
        "the current version's own rows must survive its prune"
    );
}

/// Parser namespaces must coexist inside the two retained semantic versions.
/// Administrative opens must not introduce a third, fabricated generation.
#[test]
fn parser_rotation_and_maintenance_preserve_both_live_rule_generations() {
    use talkbank_cache::{CacheIdentity, CacheOnDisk};
    use talkbank_model::ParserKind;

    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path().join("cache");
    let file = write_temp_cha(dir.path(), "identity.cha", "@UTF8\n@Begin\n@End\n");
    let versions = [
        RulesVersion::for_testing("default-rules"),
        RulesVersion::for_testing("strict-rules"),
    ];
    for version in &versions {
        for parser in [ParserKind::TreeSitter, ParserKind::Re2c] {
            let cache = CachePool::with_directory(
                cache_dir.clone(),
                CacheIdentity::new(version.clone(), parser),
            )
            .unwrap();
            let (verdict, roundtrip) = verdicts_for(parser);
            crate::shim::set_validation(&cache, &file, Structure, verdict).unwrap();
            crate::shim::set_roundtrip(&cache, &file, Structure, roundtrip).unwrap();
        }
    }
    for _ in 0..3 {
        let CacheOnDisk::Current(inspection) =
            CacheOnDisk::inspect_directory(cache_dir.clone()).unwrap()
        else {
            panic!("the cache the loop wrote is current");
        };
        let maintenance = inspection.into_maintenance().unwrap();
        assert_eq!(maintenance.stats().unwrap().total_entries, 8);
        drop(maintenance);
        for version in &versions {
            for parser in [ParserKind::TreeSitter, ParserKind::Re2c] {
                let cache = CachePool::with_directory(
                    cache_dir.clone(),
                    CacheIdentity::new(version.clone(), parser),
                )
                .unwrap();
                let (verdict, roundtrip) = verdicts_for(parser);
                assert_eq!(
                    crate::shim::get_validation(&cache, &file, Structure),
                    Some(verdict)
                );
                assert_eq!(
                    crate::shim::get_roundtrip(&cache, &file, Structure),
                    Some(roundtrip)
                );
                assert!(matches!(
                    cache.version_prune(),
                    VersionPruneOutcome::NothingUnreachable
                ));
            }
        }
    }
}

/// Contradictory verdicts per parser, so a row served to the wrong parser
/// shows: tree-sitter's validation passed and its roundtrip failed, re2c's
/// the other way round.
fn verdicts_for(
    parser: talkbank_model::ParserKind,
) -> (
    talkbank_cache::CacheOutcome,
    talkbank_cache::RoundtripOutcome,
) {
    use talkbank_cache::CacheOutcome::{Invalid, Valid};
    use talkbank_cache::RoundtripOutcome::{Failed, Passed};
    match parser {
        talkbank_model::ParserKind::TreeSitter => (Valid, Failed),
        talkbank_model::ParserKind::Re2c => (Invalid, Passed),
    }
}

use talkbank_model::validation::AlignmentValidation::Structure;

/// An in-memory cache is created empty by its open, so it reports that no
/// prune ran rather than a prune that found nothing.
#[test]
fn an_in_memory_cache_reports_that_no_prune_ran() {
    let cache = talkbank_cache::CachePool::in_memory(talkbank_cache::CacheIdentity::new(
        talkbank_cache::RulesVersion::for_testing("in-memory"),
        talkbank_model::ParserKind::TreeSitter,
    ))
    .expect("in-memory cache opens");
    assert_eq!(cache.version_prune(), &VersionPruneOutcome::FreshDatabase);
}
