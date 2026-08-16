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
             its body only sets a flag and cannot block",
        ),
    ),
    (
        "install_cli",
        Coverage::Excused(
            "takes AppHandle, and its effect is writing an executable into the \
             user's PATH, which a test must not do",
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
            "[]".to_string(),
            chatter_desktop_lib::protocol::commands::ExportFormat::Json,
            out.to_string_lossy().into_owned(),
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
