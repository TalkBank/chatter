//! Workspace navigation follows parsed structure in the current buffer.

use super::Editor;
use serde_json::json;
use tower_lsp::lsp_types::SymbolInformation;

const SOURCE: &str = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
const URI: &str = "file:///symbol-reference.cha";

fn symbols(editor: &mut Editor, id: u64, query: &str) -> Vec<SymbolInformation> {
    editor.send(json!({"jsonrpc":"2.0","id":id,"method":"workspace/symbol",
        "params":{"query":query}}));
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    serde_json::from_value::<Option<Vec<SymbolInformation>>>(response["result"].clone())
        .expect("optional symbol list")
        .unwrap_or_default()
}

#[test]
fn workspace_symbols_exclude_unparsed_drafts_and_restore_current_symbols() {
    let (line, original) = SOURCE
        .lines()
        .enumerate()
        .find(|(_, text)| text.starts_with("*CHI:\t"))
        .expect("reference utterance");
    for ending in ["\n", "\r\n", "\r"] {
        let mut editor = Editor::start();
        editor.initialize();
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":URI,"languageId":"chat","version":1,
                "text":SOURCE.replace('\n', ending)}}}),
        );
        let before = symbols(&mut editor, 2, "");
        assert!(before.iter().any(|symbol| symbol.name == "*CHI:"));
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":2},"contentChanges":[{
                "range":{"start":{"line":line,"character":0},
                    "end":{"line":line,"character":original.encode_utf16().count()}},
                "text":"*not-a-tier"}]}}),
        );
        let rejected = symbols(&mut editor, 3, "not-a-tier");
        assert!(
            rejected.is_empty(),
            "unparsed draft became a symbol: {rejected:?}"
        );
        assert!(
            !symbols(&mut editor, 4, "MOT").is_empty(),
            "healthy region remains navigable"
        );
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":3},"contentChanges":[{
                "range":{"start":{"line":line,"character":0},
                    "end":{"line":line,"character":11}},"text":original}]}}),
        );
        assert_eq!(symbols(&mut editor, 5, ""), before);
        editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose",
            "params":{"textDocument":{"uri":URI}}}));
        assert!(symbols(&mut editor, 6, "").is_empty());
        editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
        assert!(editor.response(99)["error"].is_null());
        editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
        editor.expect_exit(0);
    }
}
