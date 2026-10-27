//! A consumer that closes standard output (`chatter ... | head`) ends the
//! command with exit status 1, the status `validate --format json` gives,
//! never a panic (exit 101) and never a success: a failed run must not exit
//! 0 because its summary had nowhere to go. Every text writer shares one
//! route, so each surface here is one sample of it.

use std::process::{Output, Stdio};

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
        let harness = crate::common::CliHarness::new().expect("isolated CLI");
        harness
            .streaming_command()
            .args(args)
            .current_dir(crate::common::reference_fixture(""))
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
        &["--help"],
        &["--version"],
        &["commands", "list", "--format", "json"],
        &["checks", "list"],
        &["cache", "stats"],
        &["new"],
        &["format", "corpus/reference/core/basic-conversation.cha"],
        &["validate", "corpus/reference/core", "--tui-mode", "disable"],
        &[
            "inspect",
            "alignment",
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

/// A broken summary stream must drop the audit's staged writer, preserving an
/// existing destination and removing the newly allocated temporary file.
#[test]
fn closed_stdout_discards_the_unpublished_audit_stage() {
    let scratch = tempfile::tempdir().unwrap();
    let report = scratch.path().join("audit.jsonl");
    std::fs::write(&report, "retained report\n").unwrap();
    let output = ClosedStdout::new().run(&[
        "validate",
        "corpus/reference/core/basic-conversation.cha",
        "--audit",
        report.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        std::fs::read_to_string(&report).unwrap(),
        "retained report\n"
    );
    let entries: Vec<_> = std::fs::read_dir(scratch.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(entries, vec![report], "no abandoned audit staging file");
}

/// Publishing the transcript on stdout must complete before a sidecar can be
/// written. An existing summary remains byte-for-byte intact on a closed pipe.
#[test]
fn closed_stdout_prevents_later_rediarization_sidecar_writes() {
    let scratch = tempfile::tempdir().unwrap();
    let turns = scratch.path().join("turns.json");
    let summary = scratch.path().join("summary.json");
    std::fs::write(
        &turns,
        r#"{"turns":[{"track":"PAR0","start_ms":0,"end_ms":6000000}]}"#,
    )
    .unwrap();
    std::fs::write(&summary, "retained summary\n").unwrap();
    let output = ClosedStdout::new().run(&[
        "experimental",
        "speakers",
        "rediarize",
        "corpus/reference/content/media-bullets.cha",
        "--turns",
        turns.to_str().unwrap(),
        "--summary-json",
        summary.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        std::fs::read_to_string(summary).unwrap(),
        "retained summary\n"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("Error"),
        "{output:?}"
    );
}
