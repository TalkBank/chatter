//! A consumer that closes standard output (`chatter ... | head`) ends the
//! command with exit status 1, the status `validate --format json` gives,
//! never a panic (exit 101) and never a success: a failed run must not exit
//! 0 because its summary had nowhere to go. Every text writer shares one
//! route, so each surface here is one sample of it.

use std::process::{Command, Output, Stdio};

/// A write endpoint whose sole reader was closed before any child can start.
struct ClosedStdout(std::io::PipeWriter);

impl ClosedStdout {
    fn new() -> Self {
        let (reader, writer) = std::io::pipe().expect("stdout pipe");
        drop(reader);
        Self(writer)
    }

    /// Consume the closed-reader endpoint when launching the command.
    fn run(self, args: &[&str]) -> Output {
        let cache = tempfile::tempdir().expect("cache directory");
        Command::new(env!("CARGO_BIN_EXE_chatter"))
            .args(args)
            .current_dir(crate::common::reference_fixture(""))
            .env("TALKBANK_CHAT_CACHE_DIR", cache.path())
            .stdin(Stdio::null())
            .stdout(self.0)
            .stderr(Stdio::piped())
            .output()
            .expect("chatter ends")
    }
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
        let output = ClosedStdout::new().run(args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");
        assert!(!stderr.contains("cannot write"), "{args:?}: {stderr}");
    }
}
