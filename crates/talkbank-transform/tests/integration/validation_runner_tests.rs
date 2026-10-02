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

//! Integration tests for the streaming validation runner, config and stats.

use std::io::Write as _;
use talkbank_model::validation::AlignmentValidation;
use talkbank_transform::{
    ParserKind, RoundtripCheck, RunEnding, ValidationConfig, ValidationEvent,
    validate_directory_streaming,
};

/// Minimal valid CHAT file content for temp directory tests.
const VALID_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n%mor:\tn|hello n|world .\n@End\n";

/// Invalid CHAT content (missing @End).
const INVALID_CHAT: &str =
    "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n*CHI:\thello .\n";

// ===== Config (3 tests) =====

#[test]
fn default_config_values() {
    let config = ValidationConfig::default();
    assert_eq!(
        config.alignment,
        AlignmentValidation::IncludeTierAlignment,
        "Default should enable alignment checking"
    );
    assert_eq!(
        config.roundtrip,
        RoundtripCheck::Skip,
        "Default should disable roundtrip"
    );
}

#[test]
fn config_parser_kind_default_is_treesitter() {
    let config = ValidationConfig::default();
    assert_eq!(
        config.parser_kind,
        ParserKind::TreeSitter,
        "Default parser kind should be TreeSitter"
    );
}

// ===== Streaming validation (3 tests) =====

/// Helper: write a .cha file into a directory.
fn write_cha_file(dir: &std::path::Path, name: &str, content: &str) {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).ok();
    if let Some(ref mut f) = f {
        f.write_all(content.as_bytes()).ok();
    }
}

#[test]
fn validate_directory_with_valid_files() {
    let dir = tempfile::tempdir().ok();
    let dir = match dir {
        Some(ref d) => d.path(),
        None => return,
    };

    write_cha_file(dir, "valid1.cha", VALID_CHAT);
    write_cha_file(dir, "valid2.cha", VALID_CHAT);

    let config = ValidationConfig {
        alignment: AlignmentValidation::Structure,
        jobs: Some(std::num::NonZeroUsize::MIN),
        roundtrip: RoundtripCheck::Skip,
        error_limit: talkbank_transform::ErrorLimit::Unlimited,
        parser_kind: ParserKind::TreeSitter,
        rules: talkbank_model::RuleSelection::new(),
        presentation: talkbank_transform::PresentationPolicy::new(),
    };

    let (events, _cancel) = validate_directory_streaming(
        dir,
        &talkbank_transform::ValidationRun::uncached(config.clone()),
    );

    let mut saw_started = false;
    let mut saw_finished = false;
    for event in events {
        match event {
            ValidationEvent::Started { total_files } => {
                assert_eq!(total_files, 2);
                saw_started = true;
            }
            ValidationEvent::Finished(RunEnding::Complete(snap)) => {
                let snap = snap.snapshot();
                saw_finished = true;
                assert_eq!(snap.total_files().get(), 2);
            }
            _ => {}
        }
    }
    assert!(saw_started, "Should see Started event");
    assert!(saw_finished, "Should see Complete event");
}

#[test]
fn validate_directory_with_invalid_file() {
    let dir = tempfile::tempdir().ok();
    let dir = match dir {
        Some(ref d) => d.path(),
        None => return,
    };

    write_cha_file(dir, "bad.cha", INVALID_CHAT);

    let config = ValidationConfig {
        alignment: AlignmentValidation::Structure,
        jobs: Some(std::num::NonZeroUsize::MIN),
        roundtrip: RoundtripCheck::Skip,
        error_limit: talkbank_transform::ErrorLimit::Unlimited,
        parser_kind: ParserKind::TreeSitter,
        rules: talkbank_model::RuleSelection::new(),
        presentation: talkbank_transform::PresentationPolicy::new(),
    };

    let (events, _cancel) = validate_directory_streaming(
        dir,
        &talkbank_transform::ValidationRun::uncached(config.clone()),
    );

    let mut saw_errors = false;
    let mut saw_finished = false;
    for event in events {
        match event {
            ValidationEvent::FileComplete(complete) => {
                saw_errors |= complete.status.shown().is_some();
            }
            ValidationEvent::Finished(RunEnding::Complete(snap)) => {
                let snap = snap.snapshot();
                saw_finished = true;
                // A file the parser could not make sense of is invalid.
                assert!(
                    snap.invalid_files() > 0,
                    "Invalid file should be counted: invalid={}",
                    snap.invalid_files()
                );
            }
            _ => {}
        }
    }
    assert!(saw_finished, "Should see Complete event");
    // The invalid file's result carries its diagnostics.
    assert!(
        saw_errors,
        "the invalid file's result should carry its diagnostics"
    );
}

#[test]
fn validate_directory_empty() {
    let dir = tempfile::tempdir().ok();
    let dir = match dir {
        Some(ref d) => d.path(),
        None => return,
    };

    let config = ValidationConfig {
        alignment: AlignmentValidation::Structure,
        jobs: Some(std::num::NonZeroUsize::MIN),
        roundtrip: RoundtripCheck::Skip,
        error_limit: talkbank_transform::ErrorLimit::Unlimited,
        parser_kind: ParserKind::TreeSitter,
        rules: talkbank_model::RuleSelection::new(),
        presentation: talkbank_transform::PresentationPolicy::new(),
    };

    let (events, _cancel) = validate_directory_streaming(
        dir,
        &talkbank_transform::ValidationRun::uncached(config.clone()),
    );

    let mut saw_started = false;
    let mut saw_finished = false;
    for event in events {
        match event {
            ValidationEvent::Started { total_files } => {
                assert_eq!(total_files, 0);
                saw_started = true;
            }
            ValidationEvent::Finished(ending) => {
                assert_eq!(ending, RunEnding::NothingFound);
                saw_finished = true;
            }
            _ => {}
        }
    }
    assert!(saw_started, "Empty dir should still emit Started{{0}}");
    assert!(saw_finished, "Empty dir should end NothingFound");
}
