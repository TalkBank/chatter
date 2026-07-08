// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! Integration tests for the desktop app's validation pipeline and event bridge.
//!
//! These tests exercise the same code paths as the Tauri commands but without
//! the Tauri runtime; they call `validate_target_streaming()` directly and
//! verify the `FrontendEvent` serialization matches what the React frontend
//! expects.
//!
//! Run with: `cargo nextest run -p chatter-desktop --test validation_bridge`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chatter_desktop_lib::commands::resolve_open_in_clan;
use chatter_desktop_lib::events::FrontendEvent;
use chatter_desktop_lib::protocol;
use chatter_desktop_lib::protocol::commands::{
    ExportFormat, ExportResultsRequest, OpenInClanRequest, ValidateRequest,
};
use chatter_desktop_lib::validation::{initialize_cache, validate_target_streaming_with_config};
use crossbeam_channel::{Receiver, Sender};
use talkbank_transform::validation_runner::ValidationConfig;

/// Test-only convenience wrapper: production always threads an explicit
/// config and an app-lifetime cache (see `ValidationState` in `commands.rs`),
/// but most tests here don't care about either, so this opens a fresh cache
/// (isolated per-process by `cargo nextest`) and uses `ValidationConfig::default()`.
fn validate_target_streaming(
    target: PathBuf,
) -> Result<(Receiver<FrontendEvent>, Sender<()>), String> {
    validate_target_streaming_with_config(target, ValidationConfig::default(), initialize_cache())
}

/// Find the workspace root by walking up from the manifest dir.
fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // apps/chatter-desktop/src-tauri → apps/chatter-desktop → apps → repo root
    dir.pop();
    dir.pop();
    dir.pop();
    dir
}

/// Reference corpus path. Every `.cha` file under it must be valid CHAT.
fn reference_corpus() -> PathBuf {
    workspace_root().join("corpus/reference")
}

/// Count the `.cha` files the validator must discover, derived from disk.
///
/// The reference corpus is the sacred set and is expected to grow as new
/// fixtures are added (see `corpus/reference/`). Hardcoding the count as a
/// literal couples this desktop test to an unrelated number that breaks on
/// every legitimate fixture addition. Instead we discover the expected count by
/// globbing the same `**/*.cha` set the canonical `roundtrip_reference_corpus`
/// test runs against (an independent runtime walk here, vs that test's
/// compile-time rstest glob, so they can drift; they converge on the same set
/// today), and assert the validation run discovered and completed exactly that
/// many. This catches both a path/discovery regression (count drops to zero or
/// undercounts) and silent file-skipping, without breaking when the corpus
/// grows.
fn reference_corpus_cha_count() -> usize {
    walkdir::WalkDir::new(reference_corpus())
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("cha"))
        .count()
}

/// Collect all frontend events from a validation run.
fn collect_events(target: &Path) -> Vec<FrontendEvent> {
    let (rx, _cancel_tx) =
        validate_target_streaming(target.to_path_buf()).expect("desktop validation should start");

    let mut events = Vec::new();
    while let Ok(event) = rx.recv() {
        events.push(event);
    }
    events
}

/// Extract file-level stats from collected events.
struct RunSummary {
    total_files: usize,
    valid_files: usize,
    invalid_files: usize,
    /// file path → error count (errors + warnings)
    errors_by_file: BTreeMap<String, usize>,
    /// file path → count of Severity::Error only
    hard_errors_by_file: BTreeMap<String, usize>,
    finished: bool,
}

fn summarize(events: &[FrontendEvent]) -> RunSummary {
    let mut summary = RunSummary {
        total_files: 0,
        valid_files: 0,
        invalid_files: 0,
        errors_by_file: BTreeMap::new(),
        hard_errors_by_file: BTreeMap::new(),
        finished: false,
    };

    for event in events {
        match event {
            FrontendEvent::Started { total_files } => {
                summary.total_files = *total_files;
            }
            FrontendEvent::Errors {
                file, diagnostics, ..
            } => {
                *summary.errors_by_file.entry(file.clone()).or_default() += diagnostics.len();
                let hard = diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        let json = serde_json::to_value(&diagnostic.error).unwrap();
                        json["severity"].as_str() == Some("Error")
                    })
                    .count();
                if hard > 0 {
                    *summary.hard_errors_by_file.entry(file.clone()).or_default() += hard;
                }
            }
            FrontendEvent::FileComplete { status, .. } => {
                let json = serde_json::to_value(status).unwrap();
                match json["type"].as_str() {
                    Some("valid") => summary.valid_files += 1,
                    Some("invalid") => summary.invalid_files += 1,
                    _ => {}
                }
            }
            FrontendEvent::Finished { .. } => {
                summary.finished = true;
            }
            _ => {}
        }
    }
    summary
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn reference_corpus_no_hard_errors() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!(
            "Skipping: reference corpus not present at {}",
            corpus.display()
        );
        return;
    }

    let events = collect_events(&corpus);
    let summary = summarize(&events);
    let expected_files = reference_corpus_cha_count();

    assert!(summary.finished, "validation run did not finish");
    assert!(
        expected_files > 0,
        "no .cha files discovered under {}",
        corpus.display()
    );
    assert_eq!(
        summary.total_files, expected_files,
        "validation should discover all {expected_files} reference corpus .cha files"
    );

    // Reference corpus may have warnings but must have zero hard errors
    assert!(
        summary.hard_errors_by_file.is_empty(),
        "reference corpus should produce zero errors (Severity::Error), but got errors in: {:?}",
        summary.hard_errors_by_file
    );

    // All files should complete (valid or invalid-with-warnings-only)
    assert_eq!(
        summary.valid_files + summary.invalid_files,
        expected_files,
        "all {expected_files} files should complete: {} valid + {} invalid = {}",
        summary.valid_files,
        summary.invalid_files,
        summary.valid_files + summary.invalid_files,
    );
}

#[test]
fn event_lifecycle_has_correct_sequence() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    // First event should be Discovering
    assert!(
        matches!(events.first(), Some(FrontendEvent::Discovering)),
        "first event should be Discovering"
    );

    // Second event should be Started
    assert!(
        matches!(events.get(1), Some(FrontendEvent::Started { .. })),
        "second event should be Started"
    );

    // Last event should be Finished
    assert!(
        matches!(events.last(), Some(FrontendEvent::Finished { .. })),
        "last event should be Finished"
    );

    // Count FileComplete events, should equal total_files
    let file_completes = events
        .iter()
        .filter(|e| matches!(e, FrontendEvent::FileComplete { .. }))
        .count();

    if let Some(FrontendEvent::Started { total_files }) = events.get(1) {
        assert_eq!(
            file_completes, *total_files,
            "number of FileComplete events should match total_files"
        );
    }
}

#[test]
fn frontend_events_serialize_to_expected_json_shape() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    for event in &events {
        let json = serde_json::to_value(event).unwrap();

        // Every event must have a "type" field (from #[serde(tag = "type")])
        assert!(
            json.get("type").is_some(),
            "event missing 'type' field: {json}"
        );

        let ty = json["type"].as_str().unwrap();
        match ty {
            "discovering" => {}
            "started" => {
                assert!(
                    json.get("totalFiles").is_some(),
                    "started missing totalFiles"
                );
            }
            "errors" => {
                assert!(json.get("file").is_some(), "errors missing file");
                assert!(
                    json.get("diagnostics").is_some(),
                    "errors missing diagnostics array"
                );
                assert!(json.get("source").is_some(), "errors missing source");
                if let Some(first) = json["diagnostics"]
                    .as_array()
                    .and_then(|items| items.first())
                {
                    assert!(first.get("error").is_some(), "diagnostic missing error");
                    assert!(
                        first.get("renderedHtml").is_some(),
                        "diagnostic missing renderedHtml"
                    );
                    assert!(
                        first.get("renderedText").is_some(),
                        "diagnostic missing renderedText"
                    );
                }
            }
            "fileComplete" => {
                assert!(json.get("file").is_some(), "fileComplete missing file");
                assert!(json.get("status").is_some(), "fileComplete missing status");
                let status = &json["status"];
                assert!(
                    status.get("type").is_some(),
                    "fileComplete status missing type"
                );
            }
            "finished" => {
                let stats = &json["stats"];
                assert!(
                    stats.get("totalFiles").is_some(),
                    "finished missing totalFiles"
                );
                assert!(
                    stats.get("validFiles").is_some(),
                    "finished missing validFiles"
                );
                assert!(
                    stats.get("invalidFiles").is_some(),
                    "finished missing invalidFiles"
                );
            }
            other => panic!("unexpected event type: {other}"),
        }
    }
}

#[test]
fn protocol_contracts_serialize_to_expected_json_shape() {
    assert_eq!(protocol::events::VALIDATION, "validation-event");
    assert_eq!(protocol::commands::VALIDATE, "validate");
    assert_eq!(protocol::commands::CANCEL_VALIDATION, "cancel_validation");
    assert_eq!(
        protocol::commands::CHECK_CLAN_AVAILABLE,
        "check_clan_available"
    );
    assert_eq!(protocol::commands::OPEN_IN_CLAN, "open_in_clan");
    assert_eq!(protocol::commands::EXPORT_RESULTS, "export_results");
    assert_eq!(
        protocol::commands::REVEAL_IN_FILE_MANAGER,
        "reveal_in_file_manager"
    );

    let validate = serde_json::to_value(ValidateRequest {
        path: "/tmp/reference".into(),
        ..ValidateRequest::default()
    })
    .unwrap();
    assert_eq!(validate["path"], "/tmp/reference");
    assert_eq!(validate["roundtrip"], false);
    assert_eq!(validate["parserKind"], "tree-sitter");
    assert_eq!(validate["strictLinkers"], false);
    assert!(validate["jobs"].is_null());

    let open_in_clan = serde_json::to_value(OpenInClanRequest {
        file: "/tmp/reference.cha".into(),
        line: 12,
        col: 4,
        byte_offset: 33,
        msg: "E001: bad".into(),
    })
    .unwrap();
    assert_eq!(open_in_clan["file"], "/tmp/reference.cha");
    assert_eq!(open_in_clan["line"], 12);
    assert_eq!(open_in_clan["col"], 4);
    assert_eq!(open_in_clan["byteOffset"], 33);
    assert_eq!(open_in_clan["msg"], "E001: bad");

    let export_request = serde_json::to_value(ExportResultsRequest {
        results: "[]".into(),
        format: ExportFormat::Json,
        path: "/tmp/results.json".into(),
    })
    .unwrap();
    assert_eq!(export_request["results"], "[]");
    assert_eq!(export_request["format"], "json");
    assert_eq!(export_request["path"], "/tmp/results.json");
}

#[test]
fn single_file_validation() {
    // Use a known file from the reference corpus
    let file = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    if !file.exists() {
        println!("Skipping: {} not present", file.display());
        return;
    }

    // Desktop single-file validation should validate exactly the selected file.
    let events = collect_events(&file);
    let summary = summarize(&events);

    assert!(
        summary.finished,
        "run did not finish, got {} events",
        events.len()
    );
    assert_eq!(
        summary.total_files, 1,
        "single-file runs should report one file"
    );
    assert!(
        summary.hard_errors_by_file.is_empty(),
        "core/ files should have no hard errors"
    );

    let completed_files: Vec<_> = events
        .iter()
        .filter_map(|event| {
            if let FrontendEvent::FileComplete { file, .. } = event {
                Some(file.clone())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(
        completed_files.len(),
        1,
        "single-file runs should complete one file"
    );
    assert_eq!(
        completed_files[0],
        file.to_string_lossy(),
        "single-file runs should only complete the selected file"
    );
}

#[test]
fn finished_stats_match_file_events() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    let file_completes = events
        .iter()
        .filter(|e| matches!(e, FrontendEvent::FileComplete { .. }))
        .count();

    if let Some(FrontendEvent::Finished { stats }) = events.last() {
        let stats_json = serde_json::to_value(stats).unwrap();
        let total = stats_json["totalFiles"].as_u64().unwrap() as usize;
        let valid = stats_json["validFiles"].as_u64().unwrap() as usize;
        let invalid = stats_json["invalidFiles"].as_u64().unwrap() as usize;

        assert_eq!(
            file_completes, total,
            "FileComplete count should match stats.totalFiles"
        );
        assert_eq!(
            valid + invalid + stats_json["parseErrors"].as_u64().unwrap() as usize,
            total,
            "valid + invalid + parseErrors should equal total"
        );
    } else {
        panic!("last event should be Finished");
    }
}

/// Test with a corpus that has known subdirectories to verify tree structure.
/// Uses the reference corpus which has core/, tiers/, etc. subdirectories.
#[test]
fn nested_directory_produces_relative_paths_with_subdirs() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    // Collect all file paths from Errors and FileComplete events
    let mut all_files: Vec<String> = Vec::new();
    for event in &events {
        match event {
            FrontendEvent::Errors { file, .. } => {
                all_files.push(file.clone());
            }
            FrontendEvent::FileComplete { file, .. } if !all_files.contains(file) => {
                all_files.push(file.clone());
            }
            _ => {}
        }
    }

    // All paths should be absolute (the event bridge sends absolute paths)
    for file in &all_files {
        assert!(
            file.starts_with('/') || file.contains(":\\"),
            "file path should be absolute: {file}"
        );
    }

    // There should be files from multiple subdirectories
    let unique_dirs: std::collections::BTreeSet<_> = all_files
        .iter()
        .filter_map(|f| {
            let p = std::path::Path::new(f);
            p.parent().map(|d| d.to_string_lossy().into_owned())
        })
        .collect();

    assert!(
        unique_dirs.len() > 1,
        "reference corpus should have files in multiple subdirectories, but found dirs: {:?}",
        unique_dirs
    );
    println!("Found {} unique directories in events:", unique_dirs.len());
    for dir in &unique_dirs {
        println!("  {dir}");
    }
}

/// Verify that files with errors (including warnings) appear in Errors events.
/// This is what the FileTree filters on.
#[test]
fn files_with_any_errors_appear_in_error_events() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    let mut error_files: Vec<String> = Vec::new();
    for event in &events {
        if let FrontendEvent::Errors {
            file, diagnostics, ..
        } = event
        {
            println!("Errors event: {} ({} diagnostics)", file, diagnostics.len());
            for diagnostic in diagnostics {
                let json = serde_json::to_value(&diagnostic.error).unwrap();
                println!(
                    "  {} [{}] {}",
                    json["code"], json["severity"], json["message"]
                );
            }
            error_files.push(file.clone());
        }
    }

    println!("\nTotal files with errors: {}", error_files.len());
    println!("These are the files the FileTree would show.");
}

/// Test against ~/testchat/bad/ which has both root-level and nested/ files.
/// Skips gracefully if the directory doesn't exist (or if `HOME` is unset,
/// which is the normal Windows-CI case; there's no `HOME` env var on
/// Windows, and `env!("HOME")` is compile-time so it can't be used there).
#[test]
fn testchat_bad_nested_directory() {
    let Some(home) = std::env::var_os("HOME") else {
        println!("Skipping: HOME not set");
        return;
    };
    let testchat = std::path::PathBuf::from(home).join("testchat/bad");
    if !testchat.exists() {
        println!("Skipping: ~/testchat/bad/ not present");
        return;
    }

    let events = collect_events(&testchat);
    let summary = summarize(&events);

    assert!(summary.finished);
    println!("Total files: {}", summary.total_files);
    println!("Files with errors: {}", summary.errors_by_file.len());

    // Check that nested/ files appear
    let nested_count = summary
        .errors_by_file
        .keys()
        .filter(|k| k.contains("/nested/"))
        .count();
    println!("Files in nested/: {nested_count}");

    // Print tree structure that the frontend would build
    let root_str = testchat.to_string_lossy();
    println!("\nTree structure (files with errors only):");
    let mut sorted_paths: Vec<_> = summary.errors_by_file.keys().collect();
    sorted_paths.sort();
    for path in &sorted_paths {
        let rel = if path.starts_with(&*root_str) {
            let r = &path[root_str.len()..];
            r.strip_prefix('/').unwrap_or(r)
        } else {
            path.as_str()
        };
        let depth = rel.matches('/').count();
        let indent = "  ".repeat(depth);
        let name = rel.rsplit('/').next().unwrap_or(rel);
        let errors = summary.errors_by_file[*path];
        println!("  {indent}✗ {name} ({errors})");
    }

    assert!(summary.total_files > 0, "should find files");
    assert!(
        !summary.errors_by_file.is_empty(),
        "should have some files with errors"
    );
    assert!(
        nested_count > 0,
        "nested/ directory files should have errors too"
    );
}

/// Verify that error events carry paired rendered miette HTML per diagnostic.
#[test]
fn rendered_html_present_for_errors() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    for event in &events {
        if let FrontendEvent::Errors { diagnostics, .. } = event {
            for diagnostic in diagnostics {
                assert!(
                    !diagnostic.rendered_html.is_empty(),
                    "rendered HTML must not be empty"
                );
                // Should contain miette box-drawing characters
                assert!(
                    diagnostic.rendered_html.contains("│")
                        || diagnostic.rendered_html.contains("╭")
                        || diagnostic.rendered_html.contains("warning")
                        || diagnostic.rendered_html.contains("error"),
                    "rendered HTML should contain miette-style content, got: {}",
                    &diagnostic.rendered_html[..diagnostic.rendered_html.len().min(200)]
                );
                // ANSI colors should be converted to HTML style attributes
                assert!(
                    diagnostic.rendered_html.contains("style="),
                    "rendered HTML should contain ANSI-to-HTML color styles, got: {}",
                    &diagnostic.rendered_html[..diagnostic.rendered_html.len().min(200)]
                );
            }
        }
    }
}

#[test]
fn non_chat_files_are_rejected_by_path_contract() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let file = std::env::temp_dir().join(format!(
        "chatter-desktop-path-contract-{}-{}.txt",
        std::process::id(),
        now
    ));
    std::fs::write(&file, "not a .cha file").unwrap();

    let result = validate_target_streaming(file.clone());

    std::fs::remove_file(&file).ok();

    let message = result.expect_err("non-.cha file should be rejected");
    assert!(
        message.contains("one .cha file or one folder at a time"),
        "unexpected rejection message: {message}"
    );
}

/// REGRESSION GUARD (highest-level desktop boundary test for the wrong-line bug):
/// the desktop bridge must render an error's caret/context at the error's TRUE
/// source line, exactly like the CLI. On 2026-06-05 the GUI was opened for the
/// first time and an error whose message said line 7 rendered its caret at line 3,
/// while the CLI rendered the same error correctly. Root cause: the ANSI/colored
/// render path resolved the error's window-relative label span against the FULL
/// file instead of the error's own windowed `"input"` source (see
/// `talkbank-transform::render_error_with_miette_with_source_colored`).
///
/// This pins the boundary against the committed spec fixture E601, whose invalid
/// `%mor:\t|||` tier sits on line 8 (after the @UTF8/@Begin/@Languages/
/// @Participants/@ID/@Comment header lines). miette renders the location header as
/// `[input:8:1]`, so both the plain `rendered_text` (the CLI reference, already
/// correct) and the HTML `rendered_html` (the GUI path) must contain `:8:`.
#[test]
fn desktop_bridge_renders_error_at_true_line() {
    // E601's `%mor:\t|||` is on line 8; the miette header is `[input:8:1]`.
    const TRUE_LINE_MARKER: &str = ":8:";

    let fixture = workspace_root()
        .join("tests/error_corpus/validation_errors/E601_invalid_dependent_tier.cha");
    assert!(
        fixture.exists(),
        "committed fixture missing: {}",
        fixture.display()
    );

    let events = collect_events(&fixture);
    let mut checked = false;
    for event in &events {
        if let FrontendEvent::Errors { diagnostics, .. } = event {
            for d in diagnostics {
                checked = true;
                // The CLI (plain) path is the reference and already renders line 8.
                assert!(
                    d.rendered_text.contains(TRUE_LINE_MARKER),
                    "rendered_text should point at line 8 (the CLI reference):\n{}",
                    d.rendered_text
                );
                // The GUI (HTML) path must match the CLI line, not diverge to another.
                assert!(
                    d.rendered_html.contains(TRUE_LINE_MARKER),
                    "rendered_html must point at line 8 like the CLI, but did not:\n{}",
                    &d.rendered_html[..d.rendered_html.len().min(800)]
                );
            }
        }
    }
    assert!(
        checked,
        "expected at least one error diagnostic from the E601 fixture"
    );
}

/// REGRESSION GUARD: the desktop's single-file validation path must hit the
/// same on-disk validation cache the CLI and the desktop's own directory path
/// use, not silently skip caching. Isolates the cache to a per-process temp
/// directory via `TALKBANK_CHAT_CACHE_DIR` (see `chatter/CLAUDE.md` "Cache
/// Policy"); safe because `cargo nextest` runs each test in its own process.
#[test]
fn single_file_validation_hits_cache_on_second_run() {
    let cache_dir = std::env::temp_dir().join(format!(
        "chatter-desktop-cache-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&cache_dir).unwrap();
    // SAFETY: this test process runs alone under `cargo nextest` (one process
    // per test), so no other thread in this process reads/writes this env var.
    unsafe {
        std::env::set_var("TALKBANK_CHAT_CACHE_DIR", &cache_dir);
    }

    let file = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    assert!(file.exists(), "fixture missing: {}", file.display());

    let cache_hit_flag = |events: &[FrontendEvent]| -> bool {
        events
            .iter()
            .find_map(|event| match event {
                FrontendEvent::FileComplete { status, .. } => {
                    let json = serde_json::to_value(status).unwrap();
                    json["cacheHit"].as_bool()
                }
                _ => None,
            })
            .expect("single-file run should produce one FileComplete with a cacheHit field")
    };

    let first_run = collect_events(&file);
    assert!(
        !cache_hit_flag(&first_run),
        "first validation of a fresh cache must be a cache miss"
    );

    let second_run = collect_events(&file);
    assert!(
        cache_hit_flag(&second_run),
        "second validation of the same unchanged file must hit the cache, \
         like the CLI and the desktop's directory-validation path already do"
    );

    std::fs::remove_dir_all(&cache_dir).ok();
}

/// REGRESSION GUARD: validating a single `.cha` file directly must run the
/// same `@Media`-filename check (E531) the desktop's directory-validation
/// path and the CLI already run. Before this fix, the single-file path
/// bypassed the shared worker loop (and its file-stem-derived checks)
/// entirely, so the identical file validated via "select this file" vs.
/// "select its parent folder" produced different rule sets.
#[test]
fn single_file_validation_runs_media_filename_check() {
    let fixture = workspace_root()
        .join("tests/error_corpus/validation_errors/E531_media_filename_mismatch.cha");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let events = collect_events(&fixture);

    let found_e531 = events.iter().any(|event| {
        if let FrontendEvent::Errors { diagnostics, .. } = event {
            diagnostics.iter().any(|d| {
                let json = serde_json::to_value(&d.error).unwrap();
                json["code"].as_str() == Some("E531")
            })
        } else {
            false
        }
    });

    assert!(
        found_e531,
        "single-file validation of a file with a mismatched @Media basename \
         must report E531, the same as directory validation of the same file"
    );
}

/// REGRESSION GUARD: the single-file validation gate must use the exact same
/// `.cha`-file predicate as directory validation and the CLI
/// (`talkbank_transform::validation_runner::is_chat_transcript_path`), not a
/// desktop-local reimplementation. That shared predicate is case-sensitive on
/// the `.cha` extension and excludes macOS AppleDouble sidecar files
/// (`._name.cha`); a local case-insensitive/sidecar-inclusive copy would
/// accept files the directory walk (and CLAN) never would.
#[test]
fn single_file_gate_matches_shared_chat_transcript_predicate() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let uppercase_ext = std::env::temp_dir().join(format!(
        "chatter-desktop-predicate-test-{}-{}.CHA",
        std::process::id(),
        now
    ));
    std::fs::write(&uppercase_ext, "@UTF8\n@Begin\n@End\n").unwrap();
    let uppercase_result = validate_target_streaming(uppercase_ext.clone());
    std::fs::remove_file(&uppercase_ext).ok();
    assert!(
        uppercase_result.is_err(),
        "a `.CHA` (uppercase) file must be rejected by the single-file gate, \
         matching the case-sensitive shared predicate used by directory validation"
    );

    let sidecar = std::env::temp_dir().join(format!(
        "._chatter-desktop-predicate-test-{}-{}.cha",
        std::process::id(),
        now
    ));
    std::fs::write(&sidecar, "@UTF8\n@Begin\n@End\n").unwrap();
    let sidecar_result = validate_target_streaming(sidecar.clone());
    std::fs::remove_file(&sidecar).ok();
    assert!(
        sidecar_result.is_err(),
        "an AppleDouble sidecar file (`._name.cha`) must be rejected by the \
         single-file gate, matching the shared predicate directory validation uses"
    );
}

/// Open-in-CLAN parity (FFI-free portion): the desktop must resolve an error to
/// the SAME CLAN-adjusted coordinates and highlight message the CLI/TUI uses.
/// The actual Apple-Event send (`send2clan::send_to_clan`) cannot run in CI, so
/// this pins the testable seam: read file + `resolve_clan_location` + pick the
/// message. E601's error is on source line 8; CLAN hides the `@UTF8` header, so
/// the CLAN line is 7. The highlight message must be the bare `error.message`,
/// NOT a `"{code}: {message}"` reconstruction (the old desktop divergence).
#[test]
fn open_in_clan_resolves_clan_adjusted_line_and_bare_message() {
    let fixture = workspace_root()
        .join("tests/error_corpus/validation_errors/E601_invalid_dependent_tier.cha");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    // Use the REAL enhanced error the bridge produces, and build the request the
    // way the frontend does (line/col/byte from the error, message = error.message).
    let events = collect_events(&fixture);
    let error = events
        .iter()
        .find_map(|e| match e {
            FrontendEvent::Errors { diagnostics, .. } => {
                diagnostics.first().map(|d| d.error.clone())
            }
            _ => None,
        })
        .expect("E601 fixture should yield one error diagnostic");

    let request = OpenInClanRequest {
        file: fixture.to_string_lossy().into_owned(),
        line: error.location.line.map(|l| l as i32).unwrap_or(0),
        col: error.location.column.map(|c| c as i32).unwrap_or(0),
        byte_offset: error.location.span.start,
        msg: error.message.clone(),
    };

    let resolved = resolve_open_in_clan(&request).expect("resolve should succeed for E601");

    assert_eq!(
        resolved.line, 7,
        "CLAN line should be source line 8 minus the hidden @UTF8 header (= 7)"
    );
    assert_eq!(resolved.column, 1, "CLAN column should be 1");
    assert_eq!(
        resolved.message, error.message,
        "highlight message must be the bare error.message, not '{{code}}: {{message}}'"
    );
}
