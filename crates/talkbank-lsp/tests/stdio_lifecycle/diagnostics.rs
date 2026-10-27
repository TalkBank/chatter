//! Editor notifications must produce the same diagnostics as fresh-open text.
//! Reuse one real server and the lifecycle owner's bounded I/O and cleanup.

use super::Editor;
use serde_json::{Value, json};
use tower_lsp::lsp_types::{Position, Range, SelectionRange};

const SOURCE: &str = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
const URI: &str = "file:///diagnostic-edits.cha";

#[path = "document_links.rs"]
mod document_links;

#[path = "inlay_hints.rs"]
mod inlay_hints;

#[path = "references.rs"]
mod references;

#[path = "rename.rs"]
mod rename;

#[path = "code_actions.rs"]
mod code_actions;

#[path = "recovery.rs"]
mod recovery;

#[test]
fn reversed_clamped_edit_does_not_commit_any_part_of_a_revision() {
    let mut editor = Editor::start();
    editor.initialize();
    editor.open(SOURCE);
    assert_eq!(editor.diagnostics(), json!([]));
    editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":2},"contentChanges":[
            {"range":{"start":{"line":0,"character":0},"end":{"line":1,"character":0}},"text":""},
            {"range":{"start":{"line":1001,"character":0},"end":{"line":1000,"character":0}},"text":"oops"}
        ]}}));
    editor.send(json!({"jsonrpc":"2.0","id":10,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}));
    assert_eq!(editor.response(10)["error"]["code"], -32801);
    let mut edit_response_failures = Vec::new();
    for (id, method, params) in [
        (
            30,
            "textDocument/formatting",
            json!({"textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}),
        ),
        (
            32,
            "textDocument/completion",
            json!({"textDocument":{"uri":URI},"position":{"line":8,"character":1}}),
        ),
        (
            33,
            "textDocument/onTypeFormatting",
            json!({"textDocument":{"uri":URI},"position":{"line":8,"character":5},"ch":":","options":{"tabSize":4,"insertSpaces":false}}),
        ),
        (
            34,
            "textDocument/linkedEditingRange",
            json!({"textDocument":{"uri":URI},"position":{"line":8,"character":1}}),
        ),
    ] {
        editor.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let response = editor.response(id);
        if response["error"]["code"] != -32801 || response.get("result").is_some() {
            edit_response_failures.push(format!("{method}: {response}"));
        }
    }
    let cleared = editor.message_matching(|message| {
        message["method"] == "textDocument/publishDiagnostics" && message["params"]["version"] == 2
    });
    assert_eq!(cleared["params"]["diagnostics"], json!([]));
    let warning = editor.message_matching(|message| {
        message["method"] == "window/logMessage" && message["params"]["type"] == 2
    });
    assert!(
        warning["params"]["message"]
            .as_str()
            .unwrap()
            .contains("reopen")
    );

    // Even a valid ranged edit cannot use the discarded source. A stale full
    // replacement must not bypass the highest observed version either.
    for (id, version, changes) in [
        (
            11,
            3,
            json!([{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"text":"@UTF8\n"}]),
        ),
        (12, 3, json!([{"text":SOURCE}])),
    ] {
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":version},"contentChanges":changes}}),
        );
        editor.send(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}));
        assert_eq!(editor.response(id)["error"]["code"], -32801);
    }
    let cleared = editor.message_matching(|message| {
        message["method"] == "textDocument/publishDiagnostics" && message["params"]["version"] == 3
    });
    assert_eq!(cleared["params"]["diagnostics"], json!([]));
    editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didSave","params":{"textDocument":{"uri":URI}}}));
    for (id, method, params) in [
        (
            13,
            "textDocument/documentSymbol",
            json!({"textDocument":{"uri":URI}}),
        ),
        (14, "workspace/symbol", json!({"query":""})),
    ] {
        editor.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let response = editor.response(id);
        assert!(response["error"].is_null(), "{response}");
        assert_eq!(response["result"], Value::Null);
    }
    // A newer full replacement, followed by a ranged edit in the same batch,
    // establishes its own base and restores normal analysis.
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":4},"contentChanges":[
            {"text":SOURCE},
            {"range":{"start":{"line":0,"character":0},"end":{"line":1,"character":0}},"text":""}
        ]}}),
    );
    editor.send(json!({"jsonrpc":"2.0","id":15,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}));
    let changed = editor.response(15);
    assert!(changed["error"].is_null(), "{changed}");
    assert_eq!(changed["result"]["resultId"], "4");
    assert!(
        changed["result"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "E503")
    );
    let published = editor.message_matching(|message| {
        message["method"] == "textDocument/publishDiagnostics" && message["params"]["version"] == 4
    });
    assert_eq!(
        published["params"]["diagnostics"],
        changed["result"]["items"]
    );
    // Reopen also recovers a lost session when its version starts over.
    editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":5},"contentChanges":[
            {"range":{"start":{"line":0,"character":1001},"end":{"line":0,"character":1000}},"text":"oops"}
        ]}}));
    let cleared = editor.message_matching(|message| {
        message["method"] == "textDocument/publishDiagnostics" && message["params"]["version"] == 5
    });
    assert_eq!(cleared["params"]["diagnostics"], json!([]));
    editor.close();
    editor.open(SOURCE);
    assert_eq!(editor.diagnostics(), json!([]));
    editor.send(
        json!({"jsonrpc":"2.0","id":31,"method":"textDocument/formatting",
        "params":{"textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}}),
    );
    let formatting = editor.response(31);
    assert!(formatting["error"].is_null(), "{formatting}");
    assert!(formatting["result"].is_null());
    editor.send(
        json!({"jsonrpc":"2.0","id":35,"method":"textDocument/linkedEditingRange",
        "params":{"textDocument":{"uri":URI},"position":{"line":8,"character":1}}}),
    );
    let linked = editor.response(35);
    assert!(linked["error"].is_null(), "{linked}");
    let ranges: tower_lsp::lsp_types::LinkedEditingRanges =
        serde_json::from_value(linked["result"].clone()).unwrap();
    assert!(ranges.ranges.len() >= 2);
    let cursor = tower_lsp::lsp_types::Position::new(8, 1);
    assert!(
        ranges
            .ranges
            .iter()
            .any(|range| range.start <= cursor && cursor < range.end)
    );
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert!(editor.response(2)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
    assert!(
        edit_response_failures.is_empty(),
        "{}",
        edit_response_failures.join("\n")
    );
}

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
fn spec_diagnostics_do_not_offer_guessed_participant_or_language_facts() {
    let mut editor = Editor::start();
    editor.initialize();
    for (source, code) in [
        (
            include_str!(
                "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E308_2.cha"
            ),
            "E308",
        ),
        (
            include_str!(
                "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E504_1.cha"
            ),
            "E504",
        ),
        (
            include_str!(
                "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E504_2.cha"
            ),
            "E504",
        ),
        (
            include_str!(
                "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E507_2.cha"
            ),
            // The editor reports primary-parser missing-token recovery here;
            // do not invent E507 to imitate a later validation stage.
            "E342",
        ),
    ] {
        editor.open(source);
        let diagnostics = editor.diagnostics();
        let diagnostic = diagnostics
            .as_array()
            .unwrap()
            .iter()
            .find(|diagnostic| diagnostic["code"] == code)
            .unwrap_or_else(|| panic!("{code} not published: {diagnostics}"));
        editor.send(
            json!({"jsonrpc":"2.0","id":3,"method":"textDocument/codeAction","params":{
            "textDocument":{"uri":URI},"range":diagnostic["range"],
            "context":{"diagnostics":[diagnostic]}}}),
        );
        let response = editor.response(3);
        assert!(response["error"].is_null(), "{code}: {response}");
        assert_eq!(response["result"], Value::Null, "{code}: no guessed edit");
        editor.close();
    }

    // Positive control: refusal is selective, not a disabled code-action endpoint.
    editor.open(include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E503_1.cha"
    ));
    let diagnostics = editor.diagnostics();
    let diagnostic = diagnostics
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| diagnostic["code"] == "E503")
        .unwrap();
    editor.send(
        json!({"jsonrpc":"2.0","id":4,"method":"textDocument/codeAction","params":{
        "textDocument":{"uri":URI},"range":diagnostic["range"],
        "context":{"diagnostics":[diagnostic]}}}),
    );
    let response = editor.response(4);
    assert!(response["error"].is_null(), "{response}");
    let actions = response["result"].as_array().unwrap();
    assert_eq!(actions.len(), 1);
    assert!(actions[0]["edit"]["changes"].is_null());
    let change = &actions[0]["edit"]["documentChanges"][0];
    assert_eq!(change["textDocument"], json!({"uri":URI,"version":1}));
    assert_eq!(change["edits"][0]["newText"], "@UTF8\n");
    editor.close();
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert!(editor.response(2)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
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
    editor.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize",
        "params":{"processId":null,"rootUri":null,"capabilities":{}}}));
    let initialized = editor.response(1);
    assert!(initialized["error"].is_null(), "{initialized}");
    assert_eq!(
        initialized["result"]["capabilities"]["selectionRangeProvider"],
        true
    );
    editor.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
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
    assert_selection_chains(&mut editor, 12, 10, 14);
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
    // Move the content word by three UTF-16 units, then request immediately:
    // selection must follow this revision rather than cached parse geometry.
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":4},"contentChanges":[{
            "range":{"start":{"line":2,"character":10},"end":{"line":2,"character":10}},
            "text":"😀 "}]}}),
    );
    assert_selection_chains(&mut editor, 13, 13, 17);
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert!(editor.response(2)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}

fn assert_selection_chains(editor: &mut Editor, id: u64, word_start: u32, word_end: u32) {
    let positions = [Position::new(2, 2), Position::new(2, word_end - 1)];
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/selectionRange",
        "params":{"textDocument":{"uri":URI},"positions":positions}}),
    );
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    let selections: Vec<SelectionRange> =
        serde_json::from_value(response["result"].clone()).expect("typed selection response");
    assert_eq!(selections.len(), positions.len());
    assert_eq!(
        selections[0].range,
        Range::new(Position::new(2, 0), Position::new(2, 9))
    );
    assert_eq!(
        selections[1].range,
        Range::new(Position::new(2, word_start), Position::new(2, word_end))
    );
    let contains = |range: Range, point: Position| {
        (range.start.line, range.start.character) <= (point.line, point.character)
            && (point.line, point.character) <= (range.end.line, range.end.character)
    };
    for (selection, position) in selections.iter().zip(positions) {
        assert!(contains(selection.range, position));
        let mut child = selection;
        while let Some(parent) = child.parent.as_deref() {
            assert!(
                contains(parent.range, child.range.start)
                    && contains(parent.range, child.range.end),
                "{selection:?}"
            );
            child = parent;
        }
    }
}

#[path = "alignment_snippets.rs"]
mod alignment_snippets;
