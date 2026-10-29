//! Real editor damage/repair cycles for shared phonology recovery states.

use super::{Editor, SOURCE, URI};
use serde_json::{Value, json};
use tower_lsp::lsp_types::TextEdit;

const PHO: &str = include_str!("../../../../corpus/reference/tiers/pho-groupings.cha");
const SIN: &str = include_str!("../../../../corpus/reference/annotation/groups-sign.cha");

fn published(editor: &Editor, version: i32) -> Value {
    let notification = editor.message_matching(|message| {
        message["method"] == "textDocument/publishDiagnostics"
            && message["params"]["uri"] == URI
            && message["params"]["version"] == version
    });
    notification["params"]["diagnostics"].clone()
}

fn change(editor: &mut Editor, version: i32, source: &str) {
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":version},"contentChanges":[{"text":source}]}}),
    );
}

fn pull(editor: &mut Editor, id: u64, version: i32) -> Value {
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/diagnostic",
        "params":{"textDocument":{"uri":URI}}}),
    );
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    assert_eq!(response["result"]["resultId"], version.to_string());
    response["result"]["items"].clone()
}

#[test]
fn phonology_recovery_edits_preserve_source_and_clear_on_repair() {
    let mut editor = Editor::start();
    editor.initialize();
    for (control, group, damaged) in [
        (PHO.to_owned(), "‹a b›", "‹›"),
        (PHO.replacen("%pho:", "%mod:", 1), "‹a b›", "‹a b"),
        (SIN.to_owned(), "〔c d〕", "〔〕"),
    ] {
        assert_eq!(control.matches(group).count(), 1);
        for ending in ["\n", "\r\n"] {
            let control = control.replace('\n', ending);
            let broken = control.replacen(group, damaged, 1);
            editor.open(&control);
            let baseline = published(&editor, 1);
            assert!(
                baseline
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|d| d["code"] != "E316")
            );

            change(&mut editor, 2, &broken);
            // Request without waiting for the debounced push: both responses
            // must describe the current damaged buffer.
            let rejected = pull(&mut editor, 10, 2);
            assert_ne!(rejected, baseline, "damage must change diagnostics");
            assert!(
                rejected
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["severity"] == 1),
                "{rejected}"
            );
            editor.send(json!({"jsonrpc":"2.0","id":11,"method":"textDocument/formatting",
                "params":{"textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}}));
            let formatting = editor.response(11);
            assert!(formatting["error"].is_null(), "{formatting}");
            let edits: Option<Vec<TextEdit>> =
                serde_json::from_value(formatting["result"].clone()).unwrap();
            if let Some(edits) = edits {
                assert_eq!(edits.len(), 1);
                let output = &edits[0].new_text;
                for tier in broken.lines().filter(|line| line.starts_with('%')) {
                    assert!(
                        output.lines().any(|line| line == tier),
                        "lost recovered tier: {formatting}"
                    );
                }
            }
            assert_eq!(published(&editor, 2), rejected);

            change(&mut editor, 3, &control);
            assert_eq!(
                pull(&mut editor, 12, 3),
                baseline,
                "repair must clear recovery"
            );
            assert_eq!(published(&editor, 3), baseline);
            editor.close();
            editor.open(&control);
            assert_eq!(published(&editor, 1), baseline, "fresh-open control");
            editor.close();
        }
    }
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert!(editor.response(2)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}

#[test]
fn formatting_refuses_unrepresented_source_after_edit() {
    let mut editor = Editor::start();
    editor.initialize();
    editor.open(SOURCE);
    assert_eq!(published(&editor, 1), json!([]));
    // Same authored-source suffix mutation as the existing diagnostic control.
    let broken = format!("{SOURCE}oops");
    change(&mut editor, 2, &broken);
    editor.send(
        json!({"jsonrpc":"2.0","id":20,"method":"textDocument/formatting",
        "params":{"textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}}),
    );
    let response = editor.response(20);
    assert!(
        response["error"].is_object(),
        "formatting must refuse source loss: {response}"
    );
    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("oops"),
        "{response}"
    );
    assert!(
        response.get("result").is_none(),
        "refusal must not offer an edit"
    );
    assert!(
        published(&editor, 2)
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E316")
    );
    change(&mut editor, 3, SOURCE);
    assert_eq!(published(&editor, 3), json!([]));
    editor.send(
        json!({"jsonrpc":"2.0","id":21,"method":"textDocument/formatting",
        "params":{"textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}}),
    );
    let repaired = editor.response(21);
    assert!(repaired["error"].is_null(), "{repaired}");
    assert!(
        repaired["result"].is_null(),
        "canonical source needs no edit"
    );
    editor.close();
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert!(editor.response(2)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}

#[test]
fn on_type_formatting_uses_exact_current_cursor_and_prefix() {
    let mut editor = Editor::start();
    editor.initialize();
    for ending in ["\n", "\r\n"] {
        editor.open(&SOURCE.replace('\n', ending));
        // Derive unfinished tier lines from the reference transcript, and exercise
        // immediate requests after didChange without waiting for diagnostics.
        let prefix = SOURCE
            .lines()
            .find(|line| line.starts_with('*'))
            .unwrap()
            .split_once(':')
            .unwrap()
            .0;
        let mut version = 2;
        for line in [format!("{prefix}:"), "%mor:".to_owned(), "*😀:".to_owned()] {
            let source = format!("@Begin{ending}{line}{ending}@End{ending}");
            change(&mut editor, version, &source);
            version += 1;
            let end = line.encode_utf16().count() as u32;
            for column in 0..=end + 2 {
                editor.send(
                    json!({"jsonrpc":"2.0","id":50,"method":"textDocument/onTypeFormatting",
                    "params":{"textDocument":{"uri":URI},"position":{"line":1,"character":column},
                    "ch":":","options":{"tabSize":4,"insertSpaces":false}}}),
                );
                let response = editor.response(50);
                assert!(response["error"].is_null(), "{response}");
                if column == end {
                    assert_eq!(
                        response["result"],
                        json!([{"range":{"start":{"line":1,"character":end},
                        "end":{"line":1,"character":end}},"newText":"\t"}])
                    );
                } else {
                    assert!(
                        response["result"].is_null(),
                        "unexpected edit at {column}: {response}"
                    );
                }
            }
        }
        for line in [
            format!("{prefix}:\t"),
            format!("{prefix}:\tcontent:"),
            "*:".to_owned(),
            "%:".to_owned(),
            "@Languages:".to_owned(),
            "*CHI invalid:".to_owned(),
        ] {
            change(
                &mut editor,
                version,
                &format!("@Begin{ending}{line}{ending}@End{ending}"),
            );
            version += 1;
            editor.send(json!({"jsonrpc":"2.0","id":51,"method":"textDocument/onTypeFormatting",
                "params":{"textDocument":{"uri":URI},"position":{"line":1,"character":line.encode_utf16().count()},
                "ch":":","options":{"tabSize":4,"insertSpaces":false}}}));
            assert!(editor.response(51)["result"].is_null());
        }
        editor.close();
    }
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert!(editor.response(2)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}

#[test]
fn invalid_change_coordinates_require_a_full_replacement() {
    let source = SOURCE.replace("@End", "@Comment:\t😀\n@End");
    let comment_line = source
        .lines()
        .position(|line| line == "@Comment:\t😀")
        .unwrap() as u32;
    let middle = "@Comment:\t".encode_utf16().count() as u32 + 1;
    let missing_line = source.lines().count() as u32 + 3;
    let mut editor = Editor::start();
    editor.initialize();
    for range in [
        tower_lsp::lsp_types::Range::new(
            tower_lsp::lsp_types::Position::new(comment_line, middle),
            tower_lsp::lsp_types::Position::new(comment_line, middle + 1),
        ),
        tower_lsp::lsp_types::Range::new(
            tower_lsp::lsp_types::Position::new(comment_line, middle - 1),
            tower_lsp::lsp_types::Position::new(comment_line, middle),
        ),
        tower_lsp::lsp_types::Range::new(
            tower_lsp::lsp_types::Position::new(missing_line, 0),
            tower_lsp::lsp_types::Position::new(missing_line, 0),
        ),
    ] {
        editor.open(&source);
        assert_eq!(editor.diagnostics(), json!([]));
        editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":2},"contentChanges":[
                {"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"text":"prefix"},
                {"range":range,"text":"replacement"}
            ]}}));
        editor.send(
            json!({"jsonrpc":"2.0","id":40,"method":"textDocument/diagnostic",
            "params":{"textDocument":{"uri":URI}}}),
        );
        let lost = editor.response(40);
        assert_eq!(
            lost["error"]["code"], -32801,
            "invalid edit must lose synchronization: {lost}"
        );
        let cleared = editor.message_matching(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["version"] == 2
        });
        assert_eq!(cleared["params"]["diagnostics"], json!([]));
        // A newer full replacement recovers, with no prefix or replacement
        // text from the refused batch leaking into the server's document.
        change(&mut editor, 3, &source);
        assert_eq!(pull(&mut editor, 41, 3), json!([]));
        editor.send(
            json!({"jsonrpc":"2.0","id":42,"method":"textDocument/formatting",
            "params":{"textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}}),
        );
        let formatted = editor.response(42);
        assert!(formatted["error"].is_null(), "{formatted}");
        assert!(
            formatted["result"].is_null(),
            "the restored canonical text needs no edit: {formatted}"
        );
        editor.close();
    }
    editor.send(json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
    assert!(editor.response(2)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}
