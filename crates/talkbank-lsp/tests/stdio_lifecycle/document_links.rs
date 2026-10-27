//! Media links use the shared model of the current editor revision.
use super::{Editor, URI};
use serde_json::{Value, json};
use talkbank_lsp::backend::utils::position_to_offset;
use tower_lsp::lsp_types::DocumentLink;

const SOURCE: &str = include_str!("../../../../corpus/reference/core/headers-media-url.cha");

fn links(editor: &mut Editor, id: u64) -> Vec<DocumentLink> {
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/documentLink",
        "params":{"textDocument":{"uri":URI}}}),
    );
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    match response["result"].clone() {
        Value::Null => Vec::new(),
        result => serde_json::from_value(result).unwrap(),
    }
}

#[test]
fn document_links_preserve_quoted_urls_utf16_and_current_revision() {
    for newline in ["\n", "\r\n"] {
        let mut editor = Editor::start();
        editor.initialize();
        let original = "https://media.talkbank.org/example/sample.mp3";
        let url = "https://example.org/🦀,take.mp3?part=1,2";
        let source = SOURCE.replace(original, url).replace('\n', newline);
        editor.open(&source);
        editor.diagnostics();
        let first = links(&mut editor, 3);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].target, Some(url.parse().unwrap()));
        let range = first[0].range;
        assert_eq!(
            &source
                [position_to_offset(&source, range.start)..position_to_offset(&source, range.end)],
            format!("\"{url}\"")
        );
        assert_eq!(
            range.end.character - range.start.character,
            url.encode_utf16().count() as u32 + 2
        );

        let local = "Media🦀";
        let changed = source
            .replace(&format!("\"{url}\""), local)
            .replace("@Media:", &format!("@Comment:\t🦀{newline}@Media:"));
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":changed}]}}),
        );
        let current = links(&mut editor, 4);
        assert_eq!(current.len(), 1);
        assert!(current[0].target.is_none());
        let range = current[0].range;
        assert_eq!(
            &changed[position_to_offset(&changed, range.start)
                ..position_to_offset(&changed, range.end)],
            local
        );
        assert_eq!(
            range.end.character - range.start.character,
            local.encode_utf16().count() as u32
        );
        assert_eq!(range.start.line, first[0].range.start.line + 1);

        let malformed = changed.replace(local, "");
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":3},"contentChanges":[{"text":malformed}]}}),
        );
        assert!(links(&mut editor, 5).is_empty());

        let without = changed
            .lines()
            .filter(|line| !line.starts_with("@Media:"))
            .collect::<Vec<_>>()
            .join(newline);
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":4},"contentChanges":[{"text":without}]}}),
        );
        assert!(links(&mut editor, 6).is_empty());
        editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
        assert!(editor.response(99)["error"].is_null());
        editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
        editor.expect_exit(0);
    }
}
