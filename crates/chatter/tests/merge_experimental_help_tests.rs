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

//! The merge / reconciliation commands must be labeled EXPERIMENTAL in their
//! help text. They are early-stage (see the cli-reference "Merge and
//! Reconciliation Commands (experimental)" section): the marker keeps the
//! public v0.1.0 surface honest about that, at the two places a user looks,
//! the top-level `chatter --help` subcommand listing and each command's own
//! `chatter <cmd> --help`.
//!
//! Top-down integration test: drives the real binary's help output, the user
//! seam, so it cannot pass unless the labeling is actually wired into clap.

use std::process::Command;

/// The experimental merge surface. `merge-preflight` is an internal module,
/// not a CLI command, so it is intentionally absent.
const MERGE_COMMANDS: &[&str] = &[
    "merge",
    "speaker-id",
    "adjudicate",
    "pipeline",
    "batch",
    "sanity-scan",
];

/// Each merge command's own `--help` must flag it as experimental.
#[test]
fn merge_command_help_marks_experimental() {
    for &cmd in MERGE_COMMANDS {
        let out = Command::new(env!("CARGO_BIN_EXE_chatter"))
            .args([cmd, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to run chatter {cmd} --help: {e}"));
        let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
        assert!(
            text.contains("experimental"),
            "`chatter {cmd} --help` does not mark the command experimental:\n{text}"
        );
    }
}

/// The top-level `chatter --help` listing must flag each merge command as
/// experimental on its own line.
#[test]
fn top_level_help_marks_merge_commands_experimental() {
    let out = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .arg("--help")
        .output()
        .expect("failed to run chatter --help");
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    for &cmd in MERGE_COMMANDS {
        let line = text
            .lines()
            .find(|l| l.trim_start().starts_with(cmd))
            .unwrap_or_else(|| panic!("no `{cmd}` entry in chatter --help:\n{text}"));
        assert!(
            line.contains("experimental"),
            "`{cmd}` line in `chatter --help` is not marked experimental: {line:?}"
        );
    }
}
