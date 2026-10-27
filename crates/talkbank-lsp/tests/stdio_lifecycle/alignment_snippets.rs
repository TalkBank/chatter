//! Hover Markdown and sidecar plain text preserve the same CHAT annotations.
use super::{Editor, URI};
use serde_json::json;

const SOURCE: &str = include_str!("../../../../corpus/reference/annotation/groups-regular.cha");

#[test]
fn alignment_snippets_preserve_chat_in_hover_and_sidecar_after_edit() {
    let mut editor = Editor::start();
    editor.initialize();
    editor.open(SOURCE);
    editor.diagnostics();
    for (version, source, annotation) in [
        (1, SOURCE.to_owned(), "foo"),
        (2, SOURCE.replace("hey [= foo]", "hey [= zoo]"), "zoo"),
    ] {
        if version > 1 {
            editor.send(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":URI,"version":version},"contentChanges":[{"text":source}]}}),
            );
            editor.diagnostics();
        }
        let id = 10 * version as u64;
        editor.send(
            json!({"jsonrpc":"2.0","id":id,"method":"workspace/executeCommand",
            "params":{"command":"talkbank/getAlignmentSidecar","arguments":[URI]}}),
        );
        let sidecar = editor.response(id);
        assert!(sidecar["error"].is_null(), "{sidecar}");
        assert_eq!(
            sidecar["result"]["utterances"][0]["main_units"][0]["text"],
            format!("hey [= {annotation}]")
        );
        let (line, text) = source
            .lines()
            .enumerate()
            .find(|(_, text)| text.starts_with("*CHI:"))
            .unwrap();
        let offset = text.find("hey").unwrap();
        editor.send(json!({"jsonrpc":"2.0","id":id+1,"method":"textDocument/hover","params":{
            "textDocument":{"uri":URI},"position":{"line":line,"character":text[..offset].encode_utf16().count()+1}}}));
        let hover = editor.response(id + 1);
        assert!(hover["error"].is_null(), "{hover}");
        let markdown = hover["result"]["contents"]["value"].as_str().unwrap();
        assert!(
            markdown.contains(&format!(r"hey \[\= {annotation}\]")),
            "{markdown}"
        );
    }
    editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
    assert!(editor.response(99)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}

#[test]
fn main_and_mor_hover_after_clitic_report_the_words_own_gra_relation() {
    let source = include_str!("../../../../corpus/reference/edge-cases/clitics-and-compounds.cha");
    let mut editor = Editor::start();
    editor.initialize();
    editor.open(source);
    editor.diagnostics();
    for (id, prefix, token) in [
        (10, "*CHI:\tit's", "cookie"),
        (11, "%mor:\tpron|it~", "noun|cookie"),
    ] {
        let (line, text) = source
            .lines()
            .enumerate()
            .find(|(_, text)| text.starts_with(prefix))
            .unwrap();
        let offset = text.find(token).unwrap();
        editor.send(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/hover","params":{
            "textDocument":{"uri":URI},"position":{"line":line,"character":text[..offset].encode_utf16().count()+1}}}));
        let response = editor.response(id);
        assert!(response["error"].is_null(), "{response}");
        let markdown = response["result"]["contents"]["value"].as_str().unwrap();
        assert!(
            markdown.contains(r"| %gra | ROOT → ROOT \(root of sentence\) |"),
            "{markdown}"
        );
    }
    editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
    assert!(editor.response(99)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}
