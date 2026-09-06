//! Editor notifications must produce the same diagnostics as fresh-open text.
//! Reuse one real server and the lifecycle owner's bounded I/O and cleanup.

use super::Editor;
use serde_json::{Value, json};

const SOURCE: &str = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
const URI: &str = "file:///diagnostic-edits.cha";

impl Editor {
    fn diagnostics(&self) -> Value {
        self.message_matching(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == URI
        })["params"]["diagnostics"]
            .clone()
    }

    fn close(&mut self) {
        self.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":URI}}}));
        assert_eq!(self.diagnostics(), json!([]));
    }

    fn open(&mut self, source: &str) {
        self.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":URI,"languageId":"chat","version":1,"text":source}}}),
        );
    }
}

#[test]
fn edited_diagnostics_match_fresh_open() {
    let mut editor = Editor::start();
    editor.initialize();
    for (name, source, change, code) in [
        (
            "deleted UTF8",
            SOURCE.strip_prefix("@UTF8\n").unwrap().to_owned(),
            json!({"range":{"start":{"line":0,"character":0},"end":{"line":1,"character":0}},"text":""}),
            "E503",
        ),
        (
            "recovery suffix",
            format!("{SOURCE}oops"),
            json!({"text":format!("{SOURCE}oops")}),
            "E316",
        ),
    ] {
        editor.open(SOURCE);
        assert_eq!(editor.diagnostics(), json!([]), "{name}: reference input");
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":2},"contentChanges":[change]}}),
        );
        let edited = editor.diagnostics();
        assert!(
            edited.as_array().unwrap().iter().any(|d| d["code"] == code),
            "{name}: {edited}"
        );
        editor.close();
        editor.open(&source);
        assert_eq!(edited, editor.diagnostics(), "{name}: fresh-open parity");
        editor.close();
    }
    editor.open(SOURCE);
    assert_eq!(editor.diagnostics(), json!([]));
    let final_source = SOURCE
        .strip_prefix("@UTF8\n")
        .unwrap()
        .replace("cookies", "café😀");
    for (version, source) in [
        (2, SOURCE.replace("want", "need")),
        (3, final_source.clone()),
    ] {
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":version},"contentChanges":[{"text":source}]}}),
        );
    }
    // Request before debounced validation: geometry must use current source.
    editor.send(json!({"jsonrpc":"2.0","id":3,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":URI}}}));
    let during_debounce = editor.response(3);
    assert!(during_debounce["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","id":5,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}));
    let pulled = editor.response(5);
    assert!(pulled["error"].is_null());
    assert_eq!(pulled["result"]["resultId"], "3");
    let edited = editor.diagnostics();
    assert_eq!(pulled["result"]["items"], edited, "pull during debounce");
    editor.close();
    editor.open(&final_source);
    assert_eq!(edited, editor.diagnostics(), "skipped revision");
    editor.send(json!({"jsonrpc":"2.0","id":4,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":URI}}}));
    assert_eq!(
        during_debounce["result"],
        editor.response(4)["result"],
        "request during debounce"
    );
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert!(editor.response(2)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}
