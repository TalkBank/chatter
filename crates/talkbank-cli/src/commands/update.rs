//! `chatter update`: the discoverable front door to the bundled self-updater.
//!
//! The official cargo-dist installers (shell + PowerShell) install a standalone
//! program named `chatter-update` next to the `chatter` binary. This subcommand
//! locates that program (preferring the directory the running `chatter` lives
//! in, since cargo-dist installs the pair side by side, then falling back to
//! `PATH`), runs it, and propagates its exit code. When the updater is not
//! present (for example a build-from-source or package-manager install), it
//! explains how to obtain it rather than failing opaquely.
//!
//! The self-updater itself is experimental (the cargo-dist updater is marked
//! experimental upstream). The genuine cross-version update logic lives in
//! `chatter-update`; this command is only the launcher.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::exit_codes::EXIT_INPUT_ERROR;

/// Base name (without executable suffix) of the standalone updater program that
/// cargo-dist installs alongside `chatter`.
const UPDATER_PROGRAM: &str = "chatter-update";

/// Where a user reinstalls to obtain the updater when it is missing.
const RELEASES_URL: &str = "https://github.com/TalkBank/chatter/releases/latest";

/// Platform-correct file name of the updater (`chatter-update` on Unix,
/// `chatter-update.exe` on Windows).
fn updater_file_name() -> String {
    format!("{UPDATER_PROGRAM}{}", std::env::consts::EXE_SUFFIX)
}

/// Return the updater path sitting beside the running executable, if it exists.
///
/// cargo-dist installs `chatter` and `chatter-update` into the same directory,
/// so the sibling of the current executable is the most reliable location and
/// is checked before falling back to a `PATH` lookup. `exe_dir` is the
/// directory of the running `chatter` binary.
fn sibling_updater(exe_dir: &Path) -> Option<PathBuf> {
    let candidate = exe_dir.join(updater_file_name());
    candidate.is_file().then_some(candidate)
}

/// Resolve the command to invoke for the updater: the sibling next to the
/// running `chatter` if present, otherwise the bare platform file name so the
/// OS resolves it on `PATH`.
fn resolve_updater_command() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .and_then(|dir| sibling_updater(&dir))
        .unwrap_or_else(|| PathBuf::from(updater_file_name()))
}

/// Run `chatter update`: launch the bundled `chatter-update` and exit with its
/// status, or print an actionable message and exit non-zero when it cannot be
/// found or run. This function always terminates the process; it never returns.
pub fn run_update() {
    let program = resolve_updater_command();
    match Command::new(&program).status() {
        Ok(status) => match status.code() {
            Some(code) => std::process::exit(code),
            None => {
                // Terminated by a signal (Unix): no numeric code to propagate.
                eprintln!("chatter update: `{UPDATER_PROGRAM}` was terminated by a signal");
                std::process::exit(EXIT_INPUT_ERROR);
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("chatter update: the self-updater `{UPDATER_PROGRAM}` is not installed.");
            eprintln!("It ships with the official chatter installer. To enable `chatter update`,");
            eprintln!("reinstall from the latest release:");
            eprintln!("  {RELEASES_URL}");
            eprintln!(
                "If you installed via a package manager or from source, update the same way."
            );
            std::process::exit(EXIT_INPUT_ERROR);
        }
        Err(err) => {
            eprintln!(
                "chatter update: failed to run `{}`: {err}",
                program.display()
            );
            std::process::exit(EXIT_INPUT_ERROR);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `sibling_updater` finds a `chatter-update` placed next to the executable
    /// and returns `None` when the directory has no such file. This pins the
    /// "prefer the install directory" half of the resolver, which the
    /// subprocess integration tests cannot exercise portably.
    #[test]
    fn sibling_updater_detects_presence_and_absence() {
        let dir =
            std::env::temp_dir().join(format!("chatter-update-sibling-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");

        // Absent: nothing in the directory.
        assert!(sibling_updater(&dir).is_none());

        // Present: create the platform-named updater file.
        let updater = dir.join(updater_file_name());
        std::fs::write(&updater, b"#!/bin/sh\nexit 0\n").expect("write fake updater");
        assert_eq!(sibling_updater(&dir).as_deref(), Some(updater.as_path()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
