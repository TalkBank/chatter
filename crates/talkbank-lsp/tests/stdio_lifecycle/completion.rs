//! Completion edits must survive transport and bind to the current editor buffer.

use super::Editor;
use serde_json::json;
use talkbank_lsp::backend::utils::{offset_to_position, position_to_offset};
use tower_lsp::lsp_types::{CompletionResponse, CompletionTextEdit, TextEdit};

const SOURCE: &str =
    include_str!("../../../../corpus/reference/content/postcodes-and-freecodes.cha");
const URI: &str = "file:///completion-edits.cha";

fn postcode_edit(editor: &mut Editor, id: u64, source: &str) -> TextEdit {
    let start = source.find("[+ foo]").expect("reference postcode");
    let position = offset_to_position(source, start + 4);
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/completion",
        "params":{"textDocument":{"uri":URI},"position":position}}),
    );
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    let CompletionResponse::Array(items) =
        serde_json::from_value(response["result"].clone()).expect("typed completion response")
    else {
        panic!("expected completion array");
    };
    assert_eq!(items.len(), 4);
    let item = items
        .into_iter()
        .find(|item| item.label == "[+ exc]")
        .expect("postcode suggestion");
    let Some(CompletionTextEdit::Edit(edit)) = item.text_edit else {
        panic!("explicit completion edit");
    };
    assert_eq!(position_to_offset(source, edit.range.start), start);
    assert_eq!(
        position_to_offset(source, edit.range.end),
        start + "[+ foo]".len()
    );
    assert_eq!(edit.new_text, "[+ exc]");
    edit
}

#[test]
fn completion_edits_follow_current_buffer_over_stdio() {
    let mut editor = Editor::start();
    editor.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize",
        "params":{"processId":null,"rootUri":null,"capabilities":{}}}));
    let initialized = editor.response(1);
    assert!(initialized["error"].is_null(), "{initialized}");
    let triggers = initialized["result"]["capabilities"]["completionProvider"]["triggerCharacters"]
        .as_array()
        .expect("advertised completion triggers");
    for trigger in ["@", "[", "*", "%", "+"] {
        assert!(
            triggers.contains(&json!(trigger)),
            "missing trigger {trigger}"
        );
    }
    editor.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));

    let source = SOURCE.replace('\n', "\r\n");
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":URI,"languageId":"chat","version":1,"text":source}}}),
    );
    let original = postcode_edit(&mut editor, 2, &source);

    // Request immediately after a revision, without waiting for debounced
    // diagnostics. A non-BMP annotation changes byte and UTF-16 geometry.
    let updated = source.replace("hello .", "hello [= 😀] .");
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":updated}]}}),
    );
    let edit = postcode_edit(&mut editor, 3, &updated);
    assert_eq!(edit.range.start.line, original.range.start.line);
    assert_eq!(
        edit.range.start.character - original.range.start.character,
        " [= 😀]".encode_utf16().count() as u32
    );

    // Apply the wire edit locally and send that same range as an incremental
    // editor notification. Check the server's resulting document, not a copy of
    // the completion helper's candidate construction.
    let mut expected = updated.clone();
    expected.replace_range(
        position_to_offset(&updated, edit.range.start)
            ..position_to_offset(&updated, edit.range.end),
        &edit.new_text,
    );
    assert_eq!(expected, updated.replacen("[+ foo]", "[+ exc]", 1));
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":3},"contentChanges":[{
            "range":edit.range,"text":edit.new_text}]}}),
    );
    editor.send(
        json!({"jsonrpc":"2.0","id":4,"method":"textDocument/diagnostic",
        "params":{"textDocument":{"uri":URI}}}),
    );
    let diagnostics = editor.response(4);
    assert!(diagnostics["error"].is_null(), "{diagnostics}");
    assert_eq!(diagnostics["result"]["resultId"], "3");
    assert_eq!(diagnostics["result"]["items"], json!([]));

    editor.send(
        json!({"jsonrpc":"2.0","id":5,"method":"textDocument/formatting","params":{
        "textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}}),
    );
    let formatted = editor.response(5);
    assert!(formatted["error"].is_null(), "{formatted}");
    let edits: Vec<TextEdit> =
        serde_json::from_value(formatted["result"].clone()).expect("CRLF normalization edit");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, expected.replace("\r\n", "\n"));

    editor.send(json!({"jsonrpc":"2.0","id":6,"method":"shutdown"}));
    assert!(editor.response(6)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}
