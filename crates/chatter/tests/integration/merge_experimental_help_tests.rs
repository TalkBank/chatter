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

/// Experimental workflows are discoverable under one explicit namespace.
#[test]
fn experimental_command_help_is_scoped() {
    for path in [
        vec!["experimental"],
        vec!["experimental", "speakers"],
        vec!["experimental", "review"],
        vec!["experimental", "transform"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
            .args(&path)
            .arg("--help")
            .output()
            .expect("help subprocess");
        assert!(output.status.success());
        let help = String::from_utf8_lossy(&output.stdout);
        assert!(help.contains("chatter experimental"), "{help}");
    }
    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .arg("--help")
        .output()
        .expect("root help");
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.lines()
            .any(|line| line.trim_start().starts_with("experimental")
                && line.contains("Experimental"))
    );
}
