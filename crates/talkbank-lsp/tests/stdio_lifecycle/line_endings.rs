//! Wire positions and ranged changes use protocol lines, including lone CR.

use super::Editor;
use serde_json::json;
use tower_lsp::lsp_types::SymbolInformation;

const SOURCE: &str = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
const URI: &str = "file:///protocol-lines.cha";

fn symbols(editor: &mut Editor, id: u64) -> Vec<SymbolInformation> {
    editor.send(json!({"jsonrpc":"2.0","id":id,"method":"workspace/symbol",
        "params":{"query":"CHI"}}));
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    serde_json::from_value(response["result"].clone()).expect("speaker symbols")
}

#[test]
fn protocol_line_endings_support_symbol_navigation_and_ranged_edits() {
    let mut expected_tokens = None;
    let (line_number, line) = SOURCE
        .lines()
        .enumerate()
        .find(|(_, line)| line.starts_with("*CHI:\t"))
        .expect("reference speaker");
    for ending in ["\n", "\r\n", "\r"] {
        let mut editor = Editor::start();
        editor.initialize();
        let source = SOURCE.replace('\n', ending);
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":URI,"languageId":"chat","version":1,"text":source}}}),
        );
        let before = symbols(&mut editor, 2);
        let first = before
            .iter()
            .find(|symbol| symbol.location.range.start.line == line_number as u32)
            .expect("first speaker symbol");
        assert_eq!(
            first.location.range.end.character,
            line.encode_utf16().count() as u32
        );
        // Insert a non-BMP character at the start of the existing body. The
        // following request observes the changed source without replacing it.
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":2},"contentChanges":[{
                "range":{"start":{"line":line_number,"character":6},
                         "end":{"line":line_number,"character":6}},"text":"😀"}]}}),
        );
        let after = symbols(&mut editor, 3);
        assert_eq!(before.len(), after.len());
        for (before, after) in before.iter().zip(&after) {
            assert_eq!(before.name, after.name);
            assert_eq!(before.location.range.start, after.location.range.start);
            let extra = if before.location.range.start.line == line_number as u32 {
                2
            } else {
                0
            };
            assert_eq!(
                after.location.range.end.character,
                before.location.range.end.character + extra
            );
        }
        editor.send(
            json!({"jsonrpc":"2.0","id":4,"method":"textDocument/semanticTokens/full",
            "params":{"textDocument":{"uri":URI}}}),
        );
        let response = editor.response(4);
        assert!(response["error"].is_null(), "{response}");
        let data = response["result"]["data"]
            .as_array()
            .expect("semantic tokens");
        assert!(!data.is_empty());
        match &expected_tokens {
            None => expected_tokens = Some(data.clone()),
            Some(expected) => assert_eq!(data, expected, "token geometry for {ending:?}"),
        }
        editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
        assert!(editor.response(99)["error"].is_null());
        editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
        editor.expect_exit(0);
    }
}
