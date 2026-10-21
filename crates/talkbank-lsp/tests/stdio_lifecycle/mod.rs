//! Exercise the actual executable with the editor's stdin pipe kept open.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

mod completion;
mod diagnostics;

const DEADLINE: Duration = Duration::from_secs(10);

/// A native program path admitted before process construction. The operating
/// system still decides whether the observed file can actually be executed.
struct TestProgram(PathBuf);

impl TestProgram {
    fn at(path: PathBuf) -> std::io::Result<Self> {
        if !path.is_absolute() || !std::fs::metadata(&path)?.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the test program must be an absolute regular-file path",
            ));
        }
        Ok(Self(path))
    }

    fn selected() -> std::io::Result<Self> {
        let path = match std::env::var_os("TALKBANK_LSP_TEST_BINARY") {
            Some(path) => {
                let path = PathBuf::from(path);
                eprintln!("LSP test program: {}", path.display());
                path
            }
            None => PathBuf::from(env!("CARGO_BIN_EXE_talkbank-lsp")),
        };
        Self::at(path)
    }

    fn command(self) -> Command {
        Command::new(self.0)
    }
}

#[test]
fn native_program_selection_refuses_relative_directory_and_missing_paths() {
    assert!(matches!(
        TestProgram::at(PathBuf::from("talkbank-lsp")),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput
    ));
    let directory = std::env::temp_dir();
    assert!(matches!(
        TestProgram::at(directory),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput
    ));
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_talkbank-lsp"));
    let missing = executable.with_file_name(format!("missing-lsp-program-{}", std::process::id()));
    assert!(
        !missing.exists(),
        "missing-path control must actually be absent"
    );
    assert!(matches!(
        TestProgram::at(missing),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(TestProgram::at(executable).is_ok());
}

#[test]
fn native_program_selection_missing_override_never_falls_back() {
    let executable = std::env::current_exe().expect("test executable");
    let missing = executable.with_file_name(format!("missing-lsp-program-{}", std::process::id()));
    assert!(
        !missing.exists(),
        "missing-path control must actually be absent"
    );
    let output = Command::new(executable)
        .args([
            "stdio_lifecycle::shutdown_then_exit_does_not_require_stdin_eof",
            "--exact",
            "--nocapture",
        ])
        .env("TALKBANK_LSP_TEST_BINARY", missing)
        .output()
        .expect("actual test subprocess");
    assert!(
        !output.status.success(),
        "missing override must not fall back"
    );
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.contains("admitted LSP test program"), "{output}");
}

struct Editor {
    process: Child,
    input: Option<ChildStdin>,
    messages: Receiver<Value>,
    pending: RefCell<VecDeque<Value>>,
}

impl Editor {
    fn start() -> Self {
        let mut process = TestProgram::selected()
            .expect("admitted LSP test program")
            .command()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = process.stdin.take();
        let mut output = BufReader::new(process.stdout.take().unwrap());
        let (sender, messages) = mpsc::channel();
        std::thread::spawn(move || {
            loop {
                let mut length = None;
                loop {
                    let mut header = String::new();
                    if output.read_line(&mut header).unwrap() == 0 {
                        return;
                    }
                    if header == "\r\n" {
                        break;
                    }
                    if let Some(value) = header.strip_prefix("Content-Length:") {
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                }
                let mut payload = vec![0; length.unwrap()];
                output.read_exact(&mut payload).unwrap();
                if sender
                    .send(serde_json::from_slice(&payload).unwrap())
                    .is_err()
                {
                    return;
                }
            }
        });
        Self {
            process,
            input,
            messages,
            pending: RefCell::new(VecDeque::new()),
        }
    }

    fn send(&mut self, message: Value) {
        let payload = serde_json::to_vec(&message).unwrap();
        let input = self.input.as_mut().unwrap();
        write!(input, "Content-Length: {}\r\n\r\n", payload.len()).unwrap();
        input.write_all(&payload).unwrap();
        input.flush().unwrap();
    }

    // Preserve interleaved notifications while awaiting a response, and vice
    // versa. Transport scheduling must not make the test silently lose events.
    fn message_matching(&self, matches: impl Fn(&Value) -> bool) -> Value {
        let mut pending = self.pending.borrow_mut();
        if let Some(index) = pending.iter().position(&matches) {
            return pending.remove(index).unwrap();
        }
        let deadline = Instant::now() + DEADLINE;
        loop {
            let message = self
                .messages
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if matches(&message) {
                return message;
            }
            pending.push_back(message);
        }
    }

    fn response(&self, id: u64) -> Value {
        self.message_matching(|message| message.get("id") == Some(&json!(id)))
    }

    fn initialize(&mut self) {
        self.initialize_with_capabilities(json!({
            "workspace":{"workspaceEdit":{"documentChanges":true}}
        }));
    }

    fn initialize_with_capabilities(&mut self, capabilities: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0", "id":1, "method":"initialize",
            "params":{"processId":null,"rootUri":null,"capabilities":capabilities}}));
        let response = self.response(1);
        assert!(response["result"]["capabilities"].is_object());
        self.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
        response
    }

    fn expect_exit(&mut self, code: i32) {
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Some(status) = self.process.try_wait().unwrap() {
                assert_eq!(status.code(), Some(code));
                return;
            }
            assert!(
                Instant::now() < deadline,
                "LSP failed to exit while editor retained stdin"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[test]
fn shutdown_then_exit_does_not_require_stdin_eof() {
    let mut editor = Editor::start();
    editor.initialize();
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert_eq!(
        editor.response(2),
        json!({"jsonrpc":"2.0","id":2,"result":null})
    );
    // Shutdown alone must leave the process alive awaiting the exit notification.
    assert!(editor.process.try_wait().unwrap().is_none());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}

#[test]
fn exit_without_shutdown_is_unsuccessful() {
    let mut editor = Editor::start();
    editor.initialize();
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(1);
}

#[test]
fn exit_before_initialize_is_unsuccessful() {
    let mut editor = Editor::start();
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(1);
}

#[test]
fn rejected_shutdown_does_not_authorize_successful_exit() {
    let mut editor = Editor::start();
    editor.initialize();
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}));
    assert!(editor.response(2)["error"].is_object());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(1);
}

#[test]
fn stdin_eof_still_stops_the_server() {
    let mut editor = Editor::start();
    editor.initialize();
    drop(editor.input.take());
    editor.expect_exit(0);
}
