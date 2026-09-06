//! Exercise the actual executable with the editor's stdin pipe kept open.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

mod diagnostics;

const DEADLINE: Duration = Duration::from_secs(10);

struct Editor {
    process: Child,
    input: Option<ChildStdin>,
    messages: Receiver<Value>,
    pending: RefCell<VecDeque<Value>>,
}

impl Editor {
    fn start() -> Self {
        let mut process = Command::new(env!("CARGO_BIN_EXE_talkbank-lsp"))
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
        self.send(json!({"jsonrpc":"2.0", "id":1, "method":"initialize",
            "params":{"processId":null,"rootUri":null,"capabilities":{}}}));
        assert!(self.response(1)["result"]["capabilities"].is_object());
        self.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
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
