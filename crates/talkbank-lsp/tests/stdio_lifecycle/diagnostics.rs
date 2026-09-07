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

#[test]
fn utf16_incremental_edit_preserves_neighboring_text() {
    let mut editor = Editor::start();
    editor.initialize();
    // CRLF input ensures formatting returns the current document's text.
    let source = SOURCE
        .replace("@Begin\n", "@Begin\n@Comment:\ta😀b\n")
        .replace('\n', "\r\n");
    editor.open(&source);
    assert_eq!(editor.diagnostics(), json!([]));
    // @Comment:\t occupies ten units; a + emoji occupy three UTF-16 units.
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":2},"contentChanges":[{
            "range":{"start":{"line":2,"character":13},"end":{"line":2,"character":14}},
            "text":"X"}]}}),
    );
    editor.send(
        json!({"jsonrpc":"2.0","id":8,"method":"textDocument/formatting","params":{
        "textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}}),
    );
    let response = editor.response(8);
    assert!(response["error"].is_null(), "{response}");
    assert_eq!(
        response["result"][0]["range"]["start"],
        json!({"line":0,"character":0})
    );
    assert_eq!(
        response["result"][0]["range"]["end"],
        json!({"line":source.lines().count(),"character":0})
    );
    let text = response["result"][0]["newText"].as_str().unwrap();
    assert!(text.contains("@Comment:\ta😀X\n"), "{text}");
    assert!(!text.contains("a😀bX"), "{text}");
    let formatted = text.to_owned();
    // The actual formatter, not a copied implementation, owns idempotence.
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":3},"contentChanges":[{"text":formatted}]}}),
    );
    editor.send(
        json!({"jsonrpc":"2.0","id":9,"method":"textDocument/formatting","params":{
        "textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}}),
    );
    let unchanged = editor.response(9);
    assert!(unchanged["error"].is_null(), "{unchanged}");
    assert!(unchanged["result"].is_null(), "{unchanged}");
    editor.send(
        json!({"jsonrpc":"2.0","id":12,"method":"textDocument/selectionRange","params":{
        "textDocument":{"uri":URI},"positions":[{"line":2,"character":13}]}}),
    );
    let selection = editor.response(12);
    assert!(selection["error"].is_null(), "{selection}");
    assert_eq!(
        selection["result"][0]["range"],
        json!({
        "start":{"line":2,"character":10},"end":{"line":2,"character":14}})
    );
    for (id, method, params) in [
        (
            10,
            "textDocument/semanticTokens/full",
            json!({"textDocument":{"uri":URI}}),
        ),
        (
            11,
            "textDocument/semanticTokens/range",
            json!({"textDocument":{"uri":URI},
            "range":{"start":{"line":0,"character":0},"end":{"line":source.lines().count(),"character":0}}}),
        ),
    ] {
        editor.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let response = editor.response(id);
        assert!(response["error"].is_null(), "{response}");
        let data = response["result"]["data"].as_array().unwrap();
        assert!(!data.is_empty());
        let mut line = 0;
        let mut column = 0;
        let (tokens, remainder) = data.as_chunks::<5>();
        assert!(remainder.is_empty());
        for token in tokens {
            let delta = token[0].as_u64().unwrap();
            line += delta;
            column = if delta == 0 { column } else { 0 } + token[1].as_u64().unwrap();
            let length = token[2].as_u64().unwrap();
            let width = formatted
                .lines()
                .nth(line as usize)
                .unwrap()
                .encode_utf16()
                .count() as u64;
            assert!(
                column + length <= width,
                "{method}: token {token:?} exceeds UTF-16 line {line} width {width}"
            );
        }
    }
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert!(editor.response(2)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}
