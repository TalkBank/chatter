//! Terminal state and exit status must agree at the real process boundary.
#![cfg(unix)]

use crate::common::terminal::{Pty, RunningTerminal, TerminalOutput, terminal_flags};
use std::io::Write;
use std::os::fd::AsRawFd;

fn launch_tui(harness: &crate::common::CliHarness, output: TerminalOutput) -> RunningTerminal {
    let mut command = harness.streaming_command();
    command
        .args(["validate", "--tui-mode", "force"])
        .arg(crate::common::reference_fixture(
            "corpus/reference/core/basic-conversation.cha",
        ))
        .env("TERM", "xterm-256color");
    Pty::open().launch(command, output)
}

#[test]
fn force_quit_returns_130_after_restoring_the_terminal() {
    let harness = crate::common::CliHarness::new().unwrap();
    let mut running = launch_tui(&harness, TerminalOutput::Terminal);
    running.wait_for(b"\x1b[?1049h");
    running.pty.master.write_all(b"\x03\x03").unwrap();
    running.wait_for(b"\x1b[?1049l");
    assert_eq!(running.wait().code(), Some(130));
    running.assert_restored();
}

#[test]
fn tui_output_failure_restores_raw_mode_before_returning() {
    let harness = crate::common::CliHarness::new().unwrap();
    let mut running = launch_tui(&harness, TerminalOutput::Closed);
    assert_eq!(running.wait().code(), Some(1));
    running.assert_restored();
}

#[test]
fn terminal_observation_detects_unrestored_raw_mode() {
    let pty = Pty::open();
    let mut attributes = std::mem::MaybeUninit::uninit();
    // SAFETY: both descriptors are live and termios is initialized on success.
    assert_eq!(
        unsafe { libc::tcgetattr(pty.slave.as_raw_fd(), attributes.as_mut_ptr()) },
        0
    );
    let mut raw = unsafe { attributes.assume_init() };
    unsafe { libc::cfmakeraw(&mut raw) };
    assert_eq!(
        unsafe { libc::tcsetattr(pty.slave.as_raw_fd(), libc::TCSANOW, &raw) },
        0
    );
    assert_ne!(
        terminal_flags(&pty.master),
        pty.original_flags,
        "the observer must detect the raw state the CLI must restore"
    );
}

#[test]
fn non_unicode_arguments_are_usage_errors_without_a_startup_panic() {
    use std::os::unix::ffi::OsStringExt;
    let harness = crate::common::CliHarness::new().unwrap();
    harness
        .chatter_cmd()
        .args(["commands", "show"])
        .arg(std::ffi::OsString::from_vec(b"invalid-\xff".to_vec()))
        .assert()
        .code(2);
}
