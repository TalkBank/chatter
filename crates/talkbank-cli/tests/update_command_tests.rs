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

/// Regression for the 2026-06-22 install/update mismatch (operator report):
/// cargo-dist names the bundled self-updater after the *package*
/// (`talkbank-cli-update`), but `chatter update` only looks for
/// `chatter-update`. So the updater the official installer actually drops next
/// to `chatter` is never found, and `chatter update` reports it "not installed"
/// even on a correct, complete install.
///
/// Top-down: drives the real `chatter update` against an install layout whose
/// updater is named the way cargo-dist names it (`<package>-update`), staged as
/// a sibling of a copied `chatter`. The invariant is rename-agnostic: whatever
/// the package is called, `chatter update` must launch `<package>-update`.
#[cfg(unix)]
#[test]
fn update_runs_the_updater_named_the_way_cargo_dist_installs_it() {
    use std::os::unix::fs::PermissionsExt;

    // cargo-dist derives the standalone updater's name from the package name and
    // appends `-update` (+ platform exe suffix); this is what the real installer
    // places beside `chatter`.
    let updater_stem = concat!(env!("CARGO_PKG_NAME"), "-update");
    let updater_file = format!("{updater_stem}{}", std::env::consts::EXE_SUFFIX);

    let dir = std::env::temp_dir().join(format!("chatter-update-cargodist-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp install dir");

    // Run a COPY of chatter from the temp dir so current_exe().parent() is the
    // directory we control, the location the launcher checks first.
    let chatter_copy = dir.join(format!("chatter{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(env!("CARGO_BIN_EXE_chatter"), &chatter_copy).expect("copy chatter");
    std::fs::set_permissions(&chatter_copy, std::fs::Permissions::from_mode(0o755))
        .expect("chmod chatter copy");

    // Fake updater under cargo-dist's real name: prints a marker and exits 0.
    let updater_path = dir.join(&updater_file);
    std::fs::write(&updater_path, b"#!/bin/sh\necho CHATTER_UPDATER_RAN\n")
        .expect("write fake updater");
    std::fs::set_permissions(&updater_path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake updater");

    let out = Command::new(&chatter_copy)
        .arg("update")
        .env("PATH", &dir) // only the staged sibling is discoverable
        .output()
        .expect("failed to run chatter update");

    let _ = std::fs::remove_dir_all(&dir);

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stdout.contains("CHATTER_UPDATER_RAN"),
        "`chatter update` did not launch the updater cargo-dist installs \
         (`{updater_file}`); the launcher only looks for `chatter-update`.\n\
         status: {:?}\nstdout: {stdout}\nstderr: {stderr}",
        out.status.code()
    );
}
