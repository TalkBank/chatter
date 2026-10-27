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

//! Every Tauri command, under a real async runtime or excused in writing.
//!
//! # The defect this exists to make unrepresentable
//!
//! Tauri runs an `async fn` command ON its async runtime. `UnifiedCache::open`
//! owned a runtime and called `block_on`; nesting runtimes panics; the panic
//! unwound out of the command, so the IPC promise never settled and the UI
//! waited forever. Chatter Desktop could not validate a single file for four
//! weeks, across two releases, with the entire suite green.
//!
//! It stayed green because of a ratio rather than an oversight. The desktop
//! suite has 19 tests and, before this file, exactly ONE of them entered a
//! runtime. Every other test called the same functions on a plain thread,
//! which is the one context where the bug does not reproduce. The old header
//! of `validation_bridge.rs` said as much in its own words: it existed to
//! exercise the Tauri code paths "without the Tauri runtime".
//!
//! # Why a list-checking test rather than more tests
//!
//! Adding runtime tests fixes today's commands. The gate below fixes
//! tomorrow's: a new `#[tauri::command]` cannot be registered without either
//! gaining a runtime test or being named here with a reason. The command list
//! is READ FROM `lib.rs`, never mirrored, so it cannot drift from what the
//! application actually registers.
//!
//! Both directions are checked, the same way `UNPUBLISHED_TOP_LEVEL` does it
//! for the CLI: an unaccounted command fails, and so does an entry naming a
//! command that no longer exists.

mod common;

use chatter_desktop_lib::protocol::commands::ExportFormat;
use common::workspace_root;

/// The commands the application actually registers, read from the source of
/// truth rather than restated here.
///
/// Parsing `generate_handler!` out of the file is not elegant, and the
/// alternative is worse: a second list in this test, which is the exact defect
/// shape the gate exists to prevent. A macro-derived registry would be better
/// still, and is not worth a proc-macro for eight names.
fn registered_commands() -> Vec<String> {
    let lib = workspace_root().join("apps/chatter-desktop/src-tauri/src/lib.rs");
    let source = std::fs::read_to_string(&lib)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", lib.display()));

    let start = source
        .find("generate_handler![")
        .expect("lib.rs must register commands through tauri::generate_handler!");
    let rest = &source[start..];
    let end = rest
        .find(']')
        .expect("the generate_handler! invocation must be closed");

    let names: Vec<String> = rest[..end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("commands::"))
        .map(|name| name.trim_end_matches(',').trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();

    assert!(
        !names.is_empty(),
        "parsed zero commands out of generate_handler!; the parse is broken, and an \
         empty list would make every check below pass vacuously"
    );
    names
}

/// What runtime coverage one registered command has.
///
/// One value per command rather than two lists, because two lists admit a
/// command being in BOTH, which is not a state that means anything. The first
/// cut had that shape and needed an `^` plus a special-cased "in BOTH lists"
/// message to report a condition this enum cannot express.
enum Coverage {
    /// Driven through `tauri::async_runtime::block_on` by a test.
    RuntimeTested,
    /// No runtime test, and the reason, which is required: "we did not get to
    /// it" and "this cannot be tested here" are different states and only the
    /// second is a decision.
    Excused(&'static str),
}

/// Every registered command, and what covers it.
///
/// `validate` is covered by `validation_bridge.rs`'s
/// `a_run_starts_when_the_caller_is_driving_an_async_runtime`; the other two
/// tested ones are in this file.
const COMMAND_COVERAGE: &[(&str, Coverage)] = &[
    ("validate", Coverage::RuntimeTested),
    ("check_clan_available", Coverage::RuntimeTested),
    ("export_results", Coverage::RuntimeTested),
    (
        "cancel_validation",
        Coverage::Excused(
            "takes tauri::State, which has no constructor outside a running app; \
             delegates to runtime-entered validation_session tests covering \
             pending/running ownership and stale cancellation",
        ),
    ),
    (
        "open_in_clan",
        Coverage::Excused(
            "launches the CLAN application; a passing test would open a GUI on \
             the machine running it",
        ),
    ),
    (
        "reveal_in_file_manager",
        Coverage::Excused("opens a Finder or Explorer window as its entire effect"),
    ),
    (
        "open_external",
        Coverage::Excused(
            "opens the user's browser; also the one command that is not async, \
             so it cannot nest a runtime",
        ),
    ),
];

#[test]
fn every_registered_command_is_runtime_tested_or_excused() {
    let registered = registered_commands();

    for command in &registered {
        assert!(
            COMMAND_COVERAGE.iter().any(|(name, _)| name == command),
            "`{command}` is registered with Tauri but appears in COMMAND_COVERAGE \
             not at all. Every command runs ON Tauri's async runtime, so a command \
             with no runtime test is one whose nested-runtime behaviour nobody has \
             observed. Add a test to this file and mark it RuntimeTested, or mark \
             it Excused with the reason."
        );
    }

    // The table must not rot either: an entry naming a command that no longer
    // exists is coverage claimed over nothing.
    for (name, coverage) in COMMAND_COVERAGE {
        assert!(
            registered.iter().any(|c| c == name),
            "COMMAND_COVERAGE names `{name}`{}, which lib.rs no longer registers",
            match coverage {
                Coverage::RuntimeTested => String::new(),
                Coverage::Excused(reason) => format!(" ({reason})"),
            }
        );
    }
}

#[test]
fn check_clan_available_answers_inside_a_runtime() {
    // The assertion is that it RETURNS, not what it returns: whether CLAN is
    // installed is a property of the machine, and pinning it would make this a
    // test of the test runner's host. What is being tested is that calling it
    // the way Tauri calls it does not panic.
    let answered = tauri::async_runtime::block_on(async {
        let _: bool = chatter_desktop_lib::commands::check_clan_available().await;
        true
    });
    assert!(
        answered,
        "check_clan_available did not return under a runtime"
    );
}

#[test]
fn export_results_writes_inside_a_runtime() {
    // `std::env::temp_dir()` rather than a `tempfile` dev-dependency, which is
    // the idiom `validation_bridge.rs` already uses. Named per test so two
    // tests cannot race on one path.
    let dir = std::env::temp_dir().join("chatter-desktop-export-results-runtime");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let out = dir.join("results.json");
    let _ = std::fs::remove_file(&out);

    let outcome = tauri::async_runtime::block_on(async {
        chatter_desktop_lib::commands::export_results(
            chatter_desktop_lib::protocol::commands::ExportResultsRequest {
                results: "[]".to_string(),
                format: chatter_desktop_lib::protocol::commands::ExportFormat::Json,
                path: out.to_string_lossy().into_owned(),
            },
        )
        .await
    });

    outcome.expect("export_results should succeed writing to a temp path");
    assert!(
        out.exists(),
        "export_results reported success and wrote no file"
    );
    let _ = std::fs::remove_file(&out);
}

/// The text export writes each file's label as the app showed it, failures
/// without diagnostics included: the label has one owner, the frontend's
/// `fileStatusLabel`.
#[test]
fn text_export_writes_each_files_label_as_shown() {
    let directory = tempfile::tempdir().expect("export test directory");
    let path = directory.path().join("failures.txt");
    let read = directory.path().join("read.cha");
    let pending = directory.path().join("pending.cha");
    for source in [&read, &pending] {
        std::fs::write(
            source,
            include_bytes!("../../../../corpus/reference/core/basic-conversation.cha"),
        )
        .expect("reference source");
    }
    let results = serde_json::json!([
        {"path":read, "errors":[], "status":{"type":"readError", "message":"permission denied"},
         "statusLabel":"Read error: permission denied"},
        {"path":pending, "errors":[], "status":null, "statusLabel":"Validation pending"}
    ]);
    tauri::async_runtime::block_on(chatter_desktop_lib::commands::export_results(text_request(
        results.to_string(),
        &path,
    )))
    .expect("text export");
    let text = std::fs::read_to_string(path).expect("exported results");
    for expected in [
        "read.cha\nRead error: permission denied",
        "pending.cha\nValidation pending",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in {text:?}");
    }
}

/// Report admission protects valid inputs in both export formats.
#[test]
fn export_refuses_source_aliases_in_both_formats() {
    for format in [ExportFormat::Json, ExportFormat::Text] {
        let directory = tempfile::tempdir().expect("export directory");
        let input = directory.path().join("source.cha");
        let original = include_bytes!("../../../../corpus/reference/core/basic-conversation.cha");
        std::fs::write(&input, original).expect("reference source");
        let hard_link = directory.path().join("report.json");
        std::fs::hard_link(&input, &hard_link).expect("hard link");
        let mut aliases = vec![input.clone(), hard_link];
        #[cfg(unix)]
        {
            let symlink = directory.path().join("report.txt");
            std::os::unix::fs::symlink(&input, &symlink).expect("symlink");
            aliases.push(symlink);
        }
        let other = directory.path().join("other.cha");
        std::fs::write(&other, original).expect("other reference source");
        // Valid files are hidden from the tree but retained in export results.
        let results = serde_json::json!([{
            "path": other, "errors": [], "status": {"type":"valid", "cacheHit":false},
            "statusLabel":"Valid"
        }, {
            "path": input, "errors": [], "status": {"type":"valid", "cacheHit":false},
            "statusLabel":"Valid"
        }])
        .to_string();
        for path in aliases {
            let result =
                tauri::async_runtime::block_on(chatter_desktop_lib::commands::export_results(
                    chatter_desktop_lib::protocol::commands::ExportResultsRequest {
                        results: results.clone(),
                        format,
                        path: path.to_string_lossy().into_owned(),
                    },
                ));
            assert!(
                matches!(
                    result,
                    Err(chatter_desktop_lib::errors::ExportError::Destination(
                        talkbank_transform::report_destination::ReportDestinationError::SourceAlias(
                            _
                        )
                    ))
                ),
                "source alias must be refused: {}",
                path.display()
            );
            assert_eq!(std::fs::read(&input).expect("preserved source"), original);
        }
    }
}

/// A text export request for `results`, written to `path`.
fn text_request(
    results: String,
    path: &std::path::Path,
) -> chatter_desktop_lib::protocol::commands::ExportResultsRequest {
    chatter_desktop_lib::protocol::commands::ExportResultsRequest {
        results,
        format: ExportFormat::Text,
        path: path.to_string_lossy().into_owned(),
    }
}

#[test]
fn text_export_preserves_rendering_and_refuses_malformed_input_before_writing() {
    let directory = tempfile::tempdir().expect("export test directory");
    let path = directory.path().join("results.txt");
    let source = directory.path().join("sample.cha");
    std::fs::write(
        &source,
        include_bytes!("../../../../corpus/reference/core/basic-conversation.cha"),
    )
    .expect("reference source");
    let rendered = "W109: normalize name\n  │ media snippet\n  ╰─ advice\n";
    let results = serde_json::json!([{
        "path":source, "errors":[{"renderedText":rendered}],
        "status":{"type":"roundtripFailed", "cacheHit":false, "reason":"model changed"},
        "statusLabel":"Roundtrip failed: model changed"
    }]);
    let export = |results: String| {
        tauri::async_runtime::block_on(chatter_desktop_lib::commands::export_results(text_request(
            results, &path,
        )))
    };
    export(results.to_string()).expect("export rendered diagnostic and status");
    let original = std::fs::read_to_string(&path).expect("text export");
    assert!(
        original.contains(rendered),
        "rendered text must survive verbatim"
    );
    assert!(original.contains("Roundtrip failed: model changed"));
    for malformed in [
        serde_json::json!([{"errors":[], "status":null, "statusLabel":"Valid"}]),
        serde_json::json!([{"path":"bad.cha", "errors":[{}], "status":null, "statusLabel":"Valid"}]),
        serde_json::json!([{"path":"bad.cha", "errors":[], "status":null}]),
    ] {
        assert!(export(malformed.to_string()).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).expect("existing export"),
            original,
            "malformed input must not overwrite a prior export"
        );
    }
}

/// Existing separate reports remain replaceable. Malformed payloads or lost
/// source identity leave the previous report intact in either format.
#[test]
fn export_replaces_reports_only_with_admitted_sources() {
    use chatter_desktop_lib::errors::ExportError;
    use talkbank_transform::report_destination::ReportDestinationError;
    for format in [ExportFormat::Json, ExportFormat::Text] {
        let directory = tempfile::tempdir().expect("export directory");
        let input = directory.path().join("source.cha");
        let original = include_bytes!("../../../../corpus/reference/core/basic-conversation.cha");
        std::fs::write(&input, original).expect("reference source");
        let path = directory.path().join("report.txt");
        std::fs::write(
            &path,
            "old report contents that must be replaced, including their tail".repeat(100),
        )
        .expect("prior report");
        let results = serde_json::json!([{
            "path": input, "errors": [], "status": {"type":"valid", "cacheHit":false},
            "statusLabel":"Valid", "retainedExtension":{"message":"preserved"}
        }]);
        let export = |results: String| {
            tauri::async_runtime::block_on(chatter_desktop_lib::commands::export_results(
                chatter_desktop_lib::protocol::commands::ExportResultsRequest {
                    results,
                    format,
                    path: path.to_string_lossy().into_owned(),
                },
            ))
        };
        export(results.to_string()).expect("replace distinct report");
        let expected = match format {
            ExportFormat::Json => serde_json::to_string_pretty(&results).expect("expected JSON"),
            ExportFormat::Text => format!("{}\nValid", input.display()),
        };
        assert_eq!(std::fs::read_to_string(&path).expect("report"), expected);
        assert_eq!(std::fs::read(&input).expect("source"), original);
        for malformed in ["null", "{}", "[{\"errors\":[]}]", "not JSON"] {
            assert!(matches!(
                export(malformed.to_string()),
                Err(ExportError::MalformedResults { .. })
            ));
            assert_eq!(std::fs::read_to_string(&path).expect("report"), expected);
        }
        std::fs::remove_file(&input).expect("source disappears after validation");
        assert!(matches!(
            export(results.to_string()),
            Err(ExportError::Destination(
                ReportDestinationError::SourceIdentity { .. }
            ))
        ));
        assert_eq!(
            std::fs::read_to_string(&path).expect("preserved report"),
            expected
        );
    }
}

/// Exercise actual filesystem write failures in a child so the size limit never
/// affects the test runner or another test. The child still enters Tauri's runtime.
#[cfg(unix)]
#[test]
fn export_write_failure_preserves_reports() {
    const CHILD_DIRECTORY: &str = "CHATTER_EXPORT_LIMIT_TEST_DIRECTORY";
    match std::env::var_os(CHILD_DIRECTORY) {
        Some(directory) => {
            let directory = std::path::PathBuf::from(directory);
            let results = std::fs::read_to_string(directory.join("request.json")).unwrap();
            for (format, name) in [(ExportFormat::Json, "json"), (ExportFormat::Text, "text")] {
                for state in ["existing", "absent"] {
                    let path = directory.join(format!("{name}-{state}.txt"));
                    let result = tauri::async_runtime::block_on(
                        chatter_desktop_lib::commands::export_results(
                            chatter_desktop_lib::protocol::commands::ExportResultsRequest {
                                results: results.clone(),
                                format,
                                path: path.to_string_lossy().into_owned(),
                            },
                        ),
                    );
                    assert!(
                        matches!(
                            result,
                            Err(chatter_desktop_lib::errors::ExportError::Write { .. })
                        ),
                        "{result:?}"
                    );
                }
            }
        }
        None => {
            let directory = tempfile::tempdir().unwrap();
            let input = directory.path().join("source.cha");
            let original =
                include_bytes!("../../../../corpus/reference/core/basic-conversation.cha");
            std::fs::write(&input, original).unwrap();
            let results = serde_json::json!([{
                "path": input, "errors": [], "statusLabel": "Valid"
            }]);
            std::fs::write(directory.path().join("request.json"), results.to_string()).unwrap();
            for name in ["json", "text"] {
                std::fs::write(
                    directory.path().join(format!("{name}-existing.txt")),
                    "previous report",
                )
                .unwrap();
            }
            let output = std::process::Command::new("/bin/sh")
                .args([
                    "-c",
                    "trap '' XFSZ; ulimit -f 0 || exit 90; exec \"$@\"",
                    "export-limit",
                ])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "export_write_failure_preserves_reports",
                    "--nocapture",
                ])
                .env(CHILD_DIRECTORY, directory.path())
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed"),
                "the limited child must execute its runtime test: {output:?}"
            );
            for name in ["json", "text"] {
                assert_eq!(
                    std::fs::read_to_string(directory.path().join(format!("{name}-existing.txt")))
                        .unwrap(),
                    "previous report"
                );
                assert!(!directory.path().join(format!("{name}-absent.txt")).exists());
            }
            assert_eq!(std::fs::read(input).unwrap(), original);
            assert_eq!(
                std::fs::read_dir(directory.path()).unwrap().count(),
                4,
                "staged files must be cleaned up"
            );
        }
    }
}

/// Publication follows an existing symlink, preserves permissions and replaces
/// one directory entry without modifying sibling hard links to the old report.
#[cfg(unix)]
#[test]
fn export_publication_preserves_links_and_permissions() {
    use std::os::unix::fs::PermissionsExt;
    for format in [ExportFormat::Json, ExportFormat::Text] {
        let directory = tempfile::tempdir().unwrap();
        let report = directory.path().join("report.txt");
        let link = directory.path().join("link.txt");
        let sibling = directory.path().join("sibling.txt");
        std::fs::write(&report, "previous report").unwrap();
        std::fs::set_permissions(&report, std::fs::Permissions::from_mode(0o640)).unwrap();
        std::fs::hard_link(&report, &sibling).unwrap();
        std::os::unix::fs::symlink(&report, &link).unwrap();
        tauri::async_runtime::block_on(chatter_desktop_lib::commands::export_results(
            chatter_desktop_lib::protocol::commands::ExportResultsRequest {
                results: "[]".into(),
                format,
                path: link.to_string_lossy().into_owned(),
            },
        ))
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&report).unwrap(),
            match format {
                ExportFormat::Json => "[]",
                ExportFormat::Text => "",
            }
        );
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(
            std::fs::metadata(&report).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(
            std::fs::read_to_string(&sibling).unwrap(),
            "previous report"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 3);
    }
}
