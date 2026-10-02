//! A consumer that closes standard output (`chatter ... | head`) ends the
//! command with exit status 1, the status `validate --format json` gives,
//! never a panic (exit 101) and never a success: a failed run must not exit
//! 0 because its summary had nowhere to go. Every text writer shares one
//! route, so each surface here is one sample of it.

use std::process::{Command, Output, Stdio};

/// Run `chatter args` with standard output closed by its reader before the
/// command writes anything, and collect how it ended.
fn with_stdout_closed(args: &[&str]) -> Output {
    let cache = tempfile::tempdir().expect("cache directory");
    let mut child = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args(args)
        .current_dir(crate::common::reference_fixture(""))
        .env("TALKBANK_CHAT_CACHE_DIR", cache.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("chatter starts");
    // The reader stops at once, as `head` does once it has its lines.
    drop(child.stdout.take());
    child.wait_with_output().expect("chatter ends")
}

#[test]
fn a_closed_stdout_ends_every_text_writer_with_status_one() {
    for args in [
        &["schema"][..],
        &["normalize", "corpus/reference/core/basic-conversation.cha"],
        &["validate", "corpus/reference/core", "--tui-mode", "disable"],
        &[
            "show-alignment",
            "corpus/reference/core/basic-conversation.cha",
        ],
        &["validate", "corpus/reference/core", "--format", "json"],
    ] {
        let output = with_stdout_closed(args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");
        assert!(!stderr.contains("cannot write"), "{args:?}: {stderr}");
    }
}
