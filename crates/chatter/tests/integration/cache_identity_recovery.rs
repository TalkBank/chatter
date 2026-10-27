//! Real CLI recovery of cache scopes learned from canonical CHAT sources.

use crate::cache_tests::cache_hits;
use crate::command_matrix_tests::{CacheClearAction, cache_stats_json, run_prefix_clear};
use crate::common::{CliHarness, assert_success, reference_fixture, stdout_string};
use std::fs;
use talkbank_parser_tests::test_error::TestError;

/// A preview of a deleted input subtree must remain read-only, and its actual
/// clear must leave the similarly named sibling usable as a warm cache hit.
#[test]
fn file_identity_recovery_deleted_directory_cli_preserves_preview_and_sibling()
-> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let directory = tempfile::tempdir()?;
    let removed = directory.path().join("corpus");
    let nested = removed.join("deeper");
    let sibling = directory.path().join("corpus-other");
    fs::create_dir_all(&nested)?;
    fs::create_dir(&sibling)?;
    let paths = [
        removed.join("basic-conversation.cha"),
        nested.join("basic-conversation.cha"),
        sibling.join("basic-conversation.cha"),
    ];
    let fixture = reference_fixture("corpus/reference/core/basic-conversation.cha");
    for path in &paths {
        fs::copy(&fixture, path)?;
        assert_success(&harness.run_validate(path, &[])?, "learn canonical file");
    }
    assert_eq!(
        cache_stats_json(&harness)?["total_entries"].as_u64(),
        Some(3)
    );
    for path in &paths[..2] {
        fs::remove_file(path)?;
    }
    fs::remove_dir(&nested)?;
    fs::remove_dir(&removed)?;
    for _ in 0..2 {
        let preview = run_prefix_clear(&harness, &removed, CacheClearAction::Preview)?;
        assert_success(&preview, "preview removed directory");
        assert!(stdout_string(&preview).contains("Would clear 2 cache entries matching prefix"));
        assert_eq!(
            cache_stats_json(&harness)?["total_entries"].as_u64(),
            Some(3)
        );
        assert!(!removed.exists(), "preview must not recreate inputs");
    }
    let clear = run_prefix_clear(&harness, &removed, CacheClearAction::Apply)?;
    assert_success(&clear, "clear removed subtree");
    assert!(stdout_string(&clear).contains("Cleared 2 cache entries matching prefix"));
    assert_eq!(
        cache_stats_json(&harness)?["total_entries"].as_u64(),
        Some(1)
    );
    let repeated = run_prefix_clear(&harness, &removed, CacheClearAction::Apply)?;
    assert_success(&repeated, "repeat clear");
    assert!(stdout_string(&repeated).contains("Cleared 0 cache entries matching prefix"));
    let warm = harness.run_validate(&sibling, &[])?;
    assert_success(&warm, "retained sibling");
    assert_eq!(cache_hits(&warm)?, 1);
    assert_eq!(fs::read(&paths[2])?, fs::read(&fixture)?);
    assert!(!removed.exists());
    Ok(())
}

/// A missing prefix behind an existing file is a path refusal. It must not
/// turn into a different scope or clear an otherwise valid cached transcript.
#[test]
fn file_identity_recovery_cli_file_ancestor_refusal_preserves_cache() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("basic-conversation.cha");
    let fixture = reference_fixture("corpus/reference/core/basic-conversation.cha");
    fs::copy(&fixture, &path)?;
    assert_success(&harness.run_validate(&path, &[])?, "learn canonical file");
    assert_eq!(
        cache_stats_json(&harness)?["total_entries"].as_u64(),
        Some(1)
    );
    for preview in [CacheClearAction::Preview, CacheClearAction::Apply] {
        let output = run_prefix_clear(&harness, &path.join("missing"), preview)?;
        assert!(!output.status.success(), "file ancestor must be refused");
        assert_eq!(
            cache_stats_json(&harness)?["total_entries"].as_u64(),
            Some(1)
        );
    }
    let warm = harness.run_validate(directory.path(), &[])?;
    assert_success(&warm, "refusal leaves the learned file warm");
    assert_eq!(cache_hits(&warm)?, 1);
    assert_eq!(fs::read(&path)?, fs::read(&fixture)?);
    Ok(())
}
