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

//! Regression tests for command-tree stack usage.
//!
//! Every `chatter` invocation crashed with a main-thread stack overflow
//! on Windows (1 MiB default main-thread stack) starting 2026-06-05: the
//! clap-derived command-tree construction in debug builds outgrew that
//! limit as the CLAN flag surface expanded, aborting before argument
//! parsing even began (observed as exit code 0xC00000FD in the
//! `adjudication_tests` subprocess tests on windows-latest).
//!
//! These tests replicate the Windows-sized stack on Unix, so the limit
//! is enforced by every platform's CI rather than only by the
//! windows-latest job. Windows itself needs no shim: its native runs
//! exercise the real 1 MiB main stack.

#![cfg(unix)]

use std::process::Command;

/// The Windows default main-thread stack size in KiB (`ulimit -s` units).
const WINDOWS_MAIN_STACK_KIB: u32 = 1024;

/// `chatter --help` must succeed with a Windows-sized (1 MiB) stack.
///
/// `--help` forces full clap command-tree construction, the exact code
/// path that overflowed. The limit is applied through `sh` so it
/// constrains only the spawned `chatter` process, not the test runner.
#[test]
fn help_runs_within_windows_sized_main_stack() {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "ulimit -s {WINDOWS_MAIN_STACK_KIB} && exec \"$0\" --help"
        ))
        .arg(env!("CARGO_BIN_EXE_chatter"))
        .output()
        .expect("failed to spawn sh wrapper");

    assert!(
        output.status.success(),
        "chatter --help crashed under a 1 MiB stack (Windows default): \
         status={:?}\nstderr=\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}
