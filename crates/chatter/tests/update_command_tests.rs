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

//! `chatter update` self-updates IN-PROCESS via the axoupdater library (the
//! cargo-dist self-updater used as a library, not a separate `chatter-update`
//! binary). This matches how mainstream CLIs self-update and removes the
//! standalone-binary name-coupling that previously made `chatter update` report
//! "not installed" on a correct install (operator report, 2026-06-22): cargo-dist
//! named the bundled updater after the package (`talkbank-cli-update`) while the
//! launcher only looked for `chatter-update`.
//!
//! Top-down integration tests drive the real binary at its user seam. The genuine
//! download+install path needs a cargo-dist install receipt and network access,
//! so it is exercised first end-to-end at the next release; here we pin the
//! front-door contract: the command exists, is documented experimental, does NOT
//! shell out to a separate updater program, and fails gracefully when there is no
//! install receipt to update from.

use std::process::Command;

/// An isolated home/config root so a test never reads a real cargo-dist install
/// receipt from the developer's machine. axoupdater reads the receipt from
/// `$XDG_CONFIG_HOME/<app>/` or `~/.config/<app>/`, so pointing both `HOME` and
/// `XDG_CONFIG_HOME` at an empty dir guarantees the no-receipt path.
fn isolated_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("chatter-update-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join(".config")).expect("create isolated home dir");
    dir
}

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

/// `chatter update --help` must describe in-process self-update and flag the
/// facility experimental (the cargo-dist updater is experimental upstream). It
/// must NOT instruct the user about a separate `chatter-update` program, which no
/// longer exists.
#[test]
fn update_help_describes_in_process_self_update() {
    let out = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args(["update", "--help"])
        .output()
        .expect("failed to run chatter update --help");
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    assert!(
        text.contains("experimental"),
        "`chatter update --help` does not mark the facility experimental:\n{text}"
    );
    assert!(
        !text.contains("chatter-update"),
        "`chatter update --help` still references a separate `chatter-update` program; \
         self-update is now in-process:\n{text}"
    );
}

/// With no cargo-dist install receipt (build-from-source, package-manager, or any
/// non-installer layout), `chatter update` cannot determine the installed version,
/// so it must exit non-zero with an actionable message pointing at reinstalling,
/// never panic and never silently succeed.
#[test]
fn update_without_install_receipt_fails_gracefully() {
    let home = isolated_home("noreceipt");
    let out = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .arg("update")
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .output()
        .expect("failed to run chatter update");
    let _ = std::fs::remove_dir_all(&home);

    assert!(
        !out.status.success(),
        "`chatter update` unexpectedly succeeded with no install receipt"
    );
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .to_lowercase();
    assert!(
        text.contains("install") || text.contains("releases") || text.contains("receipt"),
        "error message is not actionable (no reinstall pointer):\n{text}"
    );
}

/// `chatter update` self-updates IN-PROCESS and must NOT delegate to a separate
/// `<package>-update` sibling binary, even when one is present beside it and on
/// PATH. This is the inverse of the old launcher contract; a fake sibling that
/// writes a sentinel when run lets us assert it is never invoked. (Regression for
/// the 2026-06-22 name-coupling bug, now fixed by going in-process rather than by
/// matching the sibling's name.)
#[cfg(unix)]
#[test]
fn update_does_not_delegate_to_a_sibling_updater_binary() {
    use std::os::unix::fs::PermissionsExt;

    let dir = isolated_home("nodelegate");

    // Run a COPY of chatter from the temp dir so current_exe().parent() is the
    // directory we control (where the old launcher checked first).
    let chatter_copy = dir.join("chatter");
    std::fs::copy(env!("CARGO_BIN_EXE_chatter"), &chatter_copy).expect("copy chatter");
    std::fs::set_permissions(&chatter_copy, std::fs::Permissions::from_mode(0o755))
        .expect("chmod chatter copy");

    // Fake updater siblings that record their own invocation via a sentinel
    // file. Stage BOTH the cargo-dist standalone-updater name (`<package>-update`)
    // AND the name the old launcher hardcoded (`chatter-update`), so the test
    // fails if `chatter update` delegates under either name. The old launcher
    // shells out to `chatter-update`, so it trips this; the in-process version
    // never looks for a sibling at all.
    let sentinel = dir.join("sibling-ran.sentinel");
    for name in [concat!(env!("CARGO_PKG_NAME"), "-update"), "chatter-update"] {
        let updater = dir.join(name);
        // `: > file` uses only shell builtins (redirection), so it records the
        // invocation even though the test restricts PATH to `dir` (no external
        // `touch` would be resolvable there).
        std::fs::write(
            &updater,
            format!("#!/bin/sh\n: > '{}'\n", sentinel.display()),
        )
        .expect("write fake sibling updater");
        std::fs::set_permissions(&updater, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake sibling updater");
    }

    let _ = Command::new(&chatter_copy)
        .arg("update")
        .env("PATH", &dir) // the only place a sibling could be discovered
        .env("HOME", &dir) // no install receipt -> in-process update no-ops gracefully
        .env("XDG_CONFIG_HOME", dir.join(".config"))
        .output()
        .expect("failed to run chatter update");

    let delegated = sentinel.exists();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        !delegated,
        "`chatter update` delegated to the sibling `{}-update` instead of self-updating \
         in-process",
        env!("CARGO_PKG_NAME")
    );
}
