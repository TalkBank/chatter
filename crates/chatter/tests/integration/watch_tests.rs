//! Real editor-style replacement saves must not detach a file watch.
#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

struct RunningWatch {
    child: Child,
    lines: Receiver<String>,
}

impl Drop for RunningWatch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl RunningWatch {
    fn wait_for(&self, needle: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut seen = Vec::new();
        loop {
            let line = self
                .lines
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|error| panic!("waiting for {needle}: {error}; {seen:?}"));
            if line.contains(needle) {
                return line;
            }
            seen.push(line);
        }
    }
}

#[test]
fn watch_survives_repeated_replacement_saves_and_ctrl_c() {
    replacement_save_journey(WatchChoice::Default);
}

#[test]
fn re2c_watch_survives_repeated_replacement_saves_and_ctrl_c() {
    replacement_save_journey(WatchChoice::Re2c);
}

enum WatchChoice {
    Default,
    Re2c,
}

fn replacement_save_journey(parser: WatchChoice) {
    let harness = crate::common::CliHarness::new().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("selected.cha");
    let valid = std::fs::read_to_string(crate::common::reference_fixture(
        "corpus/reference/core/basic-conversation.cha",
    ))
    .unwrap();
    std::fs::write(&path, &valid).unwrap();
    // A prior verdict from the other backend must not satisfy this watch.
    let (arguments, other): (&[&str], &str) = match parser {
        WatchChoice::Default => (&[], "re2c"),
        WatchChoice::Re2c => (&["--parser", "re2c"], "tree-sitter"),
    };
    harness
        .chatter_cmd()
        .arg("validate")
        .arg(&path)
        .args(["--parser", other, "--quiet"])
        .assert()
        .success();
    let mut command = harness.streaming_command();
    command.arg("watch").arg(&path);
    command.args(arguments);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, lines) = channel();
    for reader in [
        Box::new(stdout) as Box<dyn std::io::Read + Send>,
        Box::new(stderr),
    ] {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                if tx.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);
    let mut watch = RunningWatch { child, lines };
    let initial = watch.wait_for("is valid");
    assert!(
        !initial.contains("(cached)"),
        "wrong-backend cache hit: {initial}"
    );
    let replacement = scratch.path().join("replacement.tmp");
    std::fs::write(&replacement, &valid).unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    let unchanged = watch.wait_for("is valid");
    assert!(
        unchanged.contains("(cached)"),
        "same-backend cache was not reused: {unchanged}"
    );
    std::fs::write(&replacement, valid.replace("cookies .", "cookies")).unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    watch.wait_for("E305");
    std::fs::write(&replacement, &valid).unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    watch.wait_for("is valid");
    assert!(
        Command::new("kill")
            .args(["-INT", &watch.child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    watch.wait_for("Stopping watch mode");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = watch.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "watch did not exit");
        std::thread::sleep(Duration::from_millis(20));
    }
}
