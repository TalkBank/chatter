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

//! Regression test for the validation-rules-version cache key.
//!
//! # The bug this pins
//!
//! The validation-result cache is keyed on file content, path, and alignment
//! mode, but historically the only "version" dimension was the
//! `talkbank-cache` crate's package version (`CARGO_PKG_VERSION`). That string
//! does NOT change when the validation *rules* change (a new rule like E370,
//! "retrace marker must be followed by material", is added in `talkbank-model`,
//! not `talkbank-cache`). So a "Valid" result cached before E370 existed kept
//! being served after E370 was added, even though a fresh validation now
//! rejects the file. `chatter validate` (the designated authority on CHAT
//! validity) therefore returned a stale "Valid" verdict.
//!
//! # What "fixed" means
//!
//! The cache key must incorporate a *validation-rules version*: any change to
//! the active rule set must invalidate prior entries. The test models a rules
//! change by opening two caches over the *same* SQLite directory under two
//! different [`RulesVersion`] values, and asserts that a result written under
//! the first rules version is invisible to a lookup under the second.

use std::io::Write as _;

use talkbank_cache::{CachePool, RulesVersion};

/// A minimal but well-formed CHAT file. Content is irrelevant to this test
/// (we only care about cache-key behavior), but the cache reads the file from
/// disk to compute its content hash, so the file must exist.
const CHAT_CONTENT: &str = "@UTF8\n@Begin\n@End\n";

/// Write `content` to `dir/name` and return the path. The caller keeps `dir`
/// alive so the file stays on disk for the duration of the test.
fn write_temp_cha(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).expect("create temp cha file");
    file.write_all(content.as_bytes())
        .expect("write temp cha content");
    path
}

#[test]
fn validation_result_is_not_served_across_a_rules_version_change() {
    // A persistent, shared cache directory: both cache handles below open the
    // SAME on-disk SQLite database, exactly as two `chatter validate` runs
    // would on the same machine across a rule-set change.
    let cache_dir = tempfile::tempdir().expect("create temp cache dir");

    // A separate directory for the CHAT fixture, kept stable across both runs
    // so the content hash is identical (isolating the rules-version dimension).
    let file_dir = tempfile::tempdir().expect("create temp file dir");
    let file_path = write_temp_cha(file_dir.path(), "sample.cha", CHAT_CONTENT);

    // Two distinct rules versions, standing in for "before rule E370" and
    // "after rule E370". Real callers obtain this from the active rule set;
    // here we inject it directly to drive the cache-key behavior.
    let rules_before = RulesVersion::for_testing("rules-without-e370");
    let rules_after = RulesVersion::for_testing("rules-with-e370");

    // --- Run 1: validate under the OLD rule set, cache "Valid" ---
    {
        let cache = CachePool::with_directory_and_rules_version(
            cache_dir.path().to_path_buf(),
            rules_before.clone(),
        )
        .expect("open cache under old rules version");
        cache
            .set_validation(&file_path, false, true)
            .expect("cache a valid result under old rules");

        // Sanity check: the same rules version reads its own entry back.
        assert_eq!(
            cache.get_validation(&file_path, false),
            Some(true),
            "a result must be readable under the rules version that wrote it"
        );
    }

    // --- Run 2: the rule set has changed; the OLD "Valid" must NOT be served ---
    {
        let cache = CachePool::with_directory_and_rules_version(
            cache_dir.path().to_path_buf(),
            rules_after,
        )
        .expect("open cache under new rules version");

        assert_eq!(
            cache.get_validation(&file_path, false),
            None,
            "a result cached under a different validation-rules version must be a \
             cache MISS, forcing fresh re-validation; serving the stale 'Valid' here \
             is the E370 silent-staleness bug"
        );
    }
}
