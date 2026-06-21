//! `chatter update` is the discoverable front door to the self-updater that
//! the official cargo-dist installers ship alongside the binary as
//! `chatter-update`. Clinicians should be able to type `chatter update` (a
//! command they can find in `chatter --help`) rather than having to know the
//! separate `chatter-update` program exists.
//!
//! Top-down integration test: it drives the real binary at its user seam, so
//! it cannot pass unless the subcommand is actually wired into clap and the
//! dispatch path locates (or gracefully fails to locate) the updater.
//!
//! The genuine update behavior (download + install a newer release) is
//! exercised by `chatter-update` itself and first end-to-end at the next
//! release; here we pin the front-door contract: the command exists, is
//! documented as experimental, and fails gracefully with an actionable
//! message when the updater is not installed.

use std::process::Command;

/// `chatter --help` must list `update` so a non-technical user can discover it.
#[test]
fn top_level_help_lists_update_command() {
    let out = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .arg("--help")
        .output()
        .expect("failed to run chatter --help");
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    assert!(
        text.lines().any(|l| l.trim_start().starts_with("update")),
        "`chatter --help` does not list an `update` subcommand:\n{text}"
    );
}

/// `chatter update --help` must explain it runs the bundled self-updater and
/// flag the facility as experimental (the cargo-dist updater is experimental
/// upstream).
#[test]
fn update_help_describes_self_updater_and_experimental() {
    let out = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args(["update", "--help"])
        .output()
        .expect("failed to run chatter update --help");
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    assert!(
        text.contains("chatter-update"),
        "`chatter update --help` does not mention the chatter-update program:\n{text}"
    );
    assert!(
        text.contains("experimental"),
        "`chatter update --help` does not mark the facility experimental:\n{text}"
    );
}

/// When the `chatter-update` program cannot be found (not next to the running
/// binary and not on PATH), `chatter update` must exit non-zero with an
/// actionable message that points the user at reinstalling, never panic and
/// never silently succeed. We run with an emptied PATH so the only place the
/// dispatcher can look is next to the test binary, where no `chatter-update`
/// exists.
#[test]
fn update_without_updater_fails_with_actionable_message() {
    // An empty directory to use as PATH so a developer machine that happens to
    // have a real `chatter-update` installed cannot mask the absent-updater
    // path this test is asserting.
    let empty_dir =
        std::env::temp_dir().join(format!("chatter-update-test-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty_dir).expect("create empty PATH dir");

    let out = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .arg("update")
        .env("PATH", &empty_dir)
        .output()
        .expect("failed to run chatter update");

    let _ = std::fs::remove_dir(&empty_dir);

    assert!(
        !out.status.success(),
        "`chatter update` unexpectedly succeeded with no updater installed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        stderr.contains("chatter-update"),
        "error message does not name the missing updater program:\n{stderr}"
    );
    assert!(
        stderr.contains("install") || stderr.contains("releases"),
        "error message is not actionable (no reinstall pointer):\n{stderr}"
    );
}
