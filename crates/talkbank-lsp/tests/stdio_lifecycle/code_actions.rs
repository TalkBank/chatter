//! Quick fixes must be admitted for current server evidence and editor revisions.

use super::{Editor, SOURCE, URI};
use serde_json::{Value, json};
use talkbank_lsp::backend::utils::position_to_offset;
use tower_lsp::lsp_types::{CodeActionOrCommand, DocumentChanges, OneOf, TextEdit};

fn actions(editor: &mut Editor, id: u64, diagnostic: &Value) -> Value {
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/codeAction","params":{
        "textDocument":{"uri":URI},"range":diagnostic["range"],
        "context":{"diagnostics":[diagnostic]}}}),
    );
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    response["result"].clone()
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

fn diagnostic(items: &Value, code: &str) -> Value {
    items
        .as_array()
        .expect("diagnostic list")
        .iter()
        .find(|item| item["code"] == code)
        .unwrap_or_else(|| panic!("missing {code}: {items}"))
        .clone()
}

fn edit(action: &Value, version: i32) -> TextEdit {
    let CodeActionOrCommand::CodeAction(action) =
        serde_json::from_value(action.clone()).expect("typed code action")
    else {
        panic!("expected an edit, not a command")
    };
    let workspace = action.edit.expect("workspace edit");
    assert!(workspace.changes.is_none(), "no unversioned fallback");
    let Some(DocumentChanges::Edits(changes)) = workspace.document_changes else {
        panic!("versioned document edit")
    };
    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert_eq!(change.text_document.uri.as_str(), URI);
    assert_eq!(change.text_document.version, Some(version));
    assert_eq!(change.edits.len(), 1);
    let OneOf::Left(edit) = &change.edits[0] else {
        panic!("plain text edit")
    };
    edit.clone()
}

fn shutdown(editor: &mut Editor) {
    editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
    assert!(editor.response(99)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}

#[test]
fn stale_and_foreign_diagnostics_do_not_authorize_edits() {
    let mut editor = Editor::start();
    editor.initialize();
    let missing_utf8 = SOURCE.strip_prefix("@UTF8\n").unwrap();
    editor.open(missing_utf8);
    let stale = diagnostic(&editor.diagnostics(), "E503");
    let positive = actions(&mut editor, 3, &stale);
    assert_eq!(positive.as_array().unwrap().len(), 1);
    assert_eq!(edit(&positive[0], 1).new_text, "@UTF8\n");

    let mut foreign = stale.clone();
    foreign["message"] = json!("Not the server's diagnostic");
    // A forged message cannot replace current server evidence. The range
    // still selects the real E503, so the offered action is unchanged.
    assert_eq!(actions(&mut editor, 4, &foreign), positive);
    foreign["code"] = json!("E306");
    foreign["range"] = json!({"start":{"line":u32::MAX,"character":0},
        "end":{"line":u32::MAX,"character":0}});
    assert_eq!(actions(&mut editor, 5, &foreign), Value::Null);

    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":SOURCE}]}}),
    );
    // Request before waiting for debounced publication; the edit request must
    // analyze the current revision rather than consume its cached predecessor.
    assert_eq!(actions(&mut editor, 6, &stale), Value::Null);
    assert_eq!(pull(&mut editor, 7, 2), json!([]));

    editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose",
        "params":{"textDocument":{"uri":URI}}}));
    assert_eq!(actions(&mut editor, 8, &stale), Value::Null);
    editor.open(SOURCE); // Reopened version 1 is a different document instance.
    assert_eq!(pull(&mut editor, 9, 1), json!([]));
    assert_eq!(actions(&mut editor, 10, &stale), Value::Null);
    shutdown(&mut editor);
}

#[test]
fn unsupported_clients_receive_no_quick_fix_edits() {
    for capabilities in [
        json!({}),
        json!({"workspace":{"workspaceEdit":{"documentChanges":false}}}),
    ] {
        let mut editor = Editor::start();
        let initialized = editor.initialize_with_capabilities(capabilities);
        assert_eq!(
            initialized["result"]["capabilities"]["codeActionProvider"],
            false
        );
        editor.open(SOURCE.strip_prefix("@UTF8\n").unwrap());
        let current = diagnostic(&editor.diagnostics(), "E503");
        assert_eq!(actions(&mut editor, 3, &current), Value::Null);
        shutdown(&mut editor);
    }
}

#[test]
fn versioned_quick_fix_preserves_utf16_crlf_and_repairs_current_text() {
    let mut editor = Editor::start();
    let initialized = editor.initialize_with_capabilities(json!({
        "workspace":{"workspaceEdit":{"documentChanges":true}}
    }));
    assert_eq!(
        initialized["result"]["capabilities"]["codeActionProvider"],
        true
    );
    let source = include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E305_1.cha"
    )
    .replace("hello world", "hello [= 😀] world")
    .replace('\n', "\r\n");
    editor.open(&source);
    let current = diagnostic(&editor.diagnostics(), "E305");
    let middle = "*CHI:\thello [= ".encode_utf16().count() + 1;
    editor.send(
        json!({"jsonrpc":"2.0","id":20,"method":"textDocument/codeAction",
        "params":{"textDocument":{"uri":URI},"range":{
            "start":{"line":current["range"]["start"]["line"],"character":middle},
            "end":{"line":current["range"]["start"]["line"],"character":middle}},
            "context":{"diagnostics":[current]}}}),
    );
    let rejected = editor.response(20);
    assert!(rejected["error"].is_null(), "{rejected}");
    assert_eq!(
        rejected["result"],
        Value::Null,
        "UTF-16 half-surrogate range"
    );
    let suggestions = actions(&mut editor, 3, &current);
    assert_eq!(suggestions.as_array().unwrap().len(), 3);
    let selected = suggestions
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["title"] == "Add '.' (declarative/default)")
        .unwrap();
    let proposed = edit(selected, 1);
    assert_eq!(proposed.range.start, proposed.range.end);
    let end = source.find("world").unwrap() + "world".len();
    assert_eq!(position_to_offset(&source, proposed.range.start), end);
    assert_eq!(
        proposed.range.start.character,
        "*CHI:\thello [= 😀] world".encode_utf16().count() as u32
    );

    let mut expected = source.clone();
    expected.insert_str(end, &proposed.new_text);
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":2},"contentChanges":[{
            "range":proposed.range,"text":proposed.new_text}]}}),
    );
    assert_eq!(pull(&mut editor, 4, 2), json!([]));
    assert_eq!(actions(&mut editor, 5, &current), Value::Null);

    editor.send(
        json!({"jsonrpc":"2.0","id":6,"method":"textDocument/formatting","params":{
        "textDocument":{"uri":URI},"options":{"tabSize":4,"insertSpaces":false}}}),
    );
    let formatted = editor.response(6);
    assert!(formatted["error"].is_null(), "{formatted}");
    let formatted: Vec<TextEdit> = serde_json::from_value(formatted["result"].clone()).unwrap();
    assert_eq!(formatted.len(), 1);
    assert_eq!(formatted[0].new_text, expected.replace("\r\n", "\n"));
    shutdown(&mut editor);
}

#[test]
fn existing_fix_policies_emit_versioned_edits_from_real_diagnostics() {
    for (source, code) in [
        (
            include_str!(
                "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E502_1.cha"
            ),
            "E502",
        ),
        (
            include_str!(
                "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E306_separator_only_2.cha"
            ),
            "E306",
        ),
        (
            include_str!(
                "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E259_1.cha"
            ),
            "E259",
        ),
    ] {
        let mut editor = Editor::start();
        editor.initialize();
        editor.open(source);
        let current = diagnostic(&editor.diagnostics(), code);
        let suggestions = actions(&mut editor, 3, &current);
        let matching: Vec<_> = suggestions
            .as_array()
            .unwrap()
            .iter()
            .filter(|action| {
                action["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["code"] == code)
            })
            .collect();
        assert_eq!(matching.len(), 1, "{code}: {suggestions}");
        let proposed = edit(matching[0], 1);
        let start = position_to_offset(source, proposed.range.start);
        let end = position_to_offset(source, proposed.range.end);
        match code {
            "E502" => {
                assert_eq!(start, source.len());
                assert_eq!(end, start);
                assert_eq!(proposed.new_text, "@End\n");
            }
            "E306" => {
                assert_eq!(&source[start..end], "*CHI:\t, .\n");
                assert!(proposed.new_text.is_empty());
            }
            "E259" => {
                assert_eq!(&source[start..end], ",");
                assert!(proposed.new_text.is_empty());
            }
            _ => unreachable!(),
        }
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":2},"contentChanges":[{
                "range":proposed.range,"text":proposed.new_text}]}}),
        );
        let after = pull(&mut editor, 4, 2);
        assert!(
            !after
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == code),
            "{code}: {after}"
        );
        shutdown(&mut editor);
    }
}

#[test]
fn empty_turn_with_dependent_tier_is_not_deleted() {
    let mut editor = Editor::start();
    editor.initialize();
    let source = include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E306_separator_only_3.cha"
    );
    editor.open(source);
    let current = diagnostic(&editor.diagnostics(), "E306");
    let suggestions = actions(&mut editor, 3, &current);
    // The overlapping comma fix is allowed; deleting the tier is not.
    let offered = suggestions.as_array().unwrap();
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0]["diagnostics"][0]["code"], "E259");
    let proposed = edit(&offered[0], 1);
    let start = position_to_offset(source, proposed.range.start);
    let end = position_to_offset(source, proposed.range.end);
    assert_eq!(&source[start..end], ",");
    assert!(proposed.new_text.is_empty());
    shutdown(&mut editor);
}

#[test]
fn quick_fixes_use_current_range_and_requested_kind_without_echoed_diagnostics() {
    let mut editor = Editor::start();
    editor.initialize();
    editor.open(SOURCE.strip_prefix("@UTF8\n").unwrap());
    let current = diagnostic(&editor.diagnostics(), "E503");
    enum Expected {
        Fix,
        None,
    }
    let cases = [
        (Value::Null, Expected::Fix),
        (json!(["quickfix"]), Expected::Fix),
        (json!([""]), Expected::Fix),
        (json!(["source.fixAll", "quickfix"]), Expected::Fix),
        (json!(["source.fixAll"]), Expected::None),
        (json!(["refactor"]), Expected::None),
        (json!(["quickfix.custom"]), Expected::None),
        (json!(["quickfixOther"]), Expected::None),
        (json!([]), Expected::None),
    ];
    for (index, (only, offered)) in cases.into_iter().enumerate() {
        let id = 10 + index as u64;
        editor.send(
            json!({"jsonrpc":"2.0","id":id,"method":"textDocument/codeAction",
            "params":{"textDocument":{"uri":URI},"range":current["range"],
            "context":{"diagnostics":[],"only":only}}}),
        );
        let response = editor.response(id);
        assert!(response["error"].is_null(), "{response}");
        if matches!(offered, Expected::Fix) {
            assert_eq!(response["result"].as_array().unwrap().len(), 1);
            assert_eq!(edit(&response["result"][0], 1).new_text, "@UTF8\n");
        } else {
            assert_eq!(response["result"], Value::Null);
        }
    }
    // A genuine diagnostic sent outside the requested range cannot select a fix.
    for (index, range) in [
        json!({"start":{"line":1,"character":0},"end":{"line":1,"character":0}}),
        json!({"start":{"line":1,"character":0},"end":{"line":0,"character":0}}),
        json!({"start":{"line":0,"character":999},"end":{"line":0,"character":999}}),
    ]
    .into_iter()
    .enumerate()
    {
        let id = 30 + index as u64;
        editor.send(
            json!({"jsonrpc":"2.0","id":id,"method":"textDocument/codeAction",
            "params":{"textDocument":{"uri":URI},"range":range,
            "context":{"diagnostics":[current]}}}),
        );
        let response = editor.response(id);
        assert!(response["error"].is_null(), "{response}");
        assert_eq!(response["result"], Value::Null);
    }
    shutdown(&mut editor);
}

#[test]
fn header_quick_fixes_preserve_protocol_line_endings() {
    #[derive(Clone, Copy)]
    enum MissingHeader {
        Utf8,
        End,
    }
    impl MissingHeader {
        fn code(self) -> &'static str {
            match self {
                Self::Utf8 => "E503",
                Self::End => "E502",
            }
        }
        fn offset(self, source: &str) -> usize {
            match self {
                Self::Utf8 => 0,
                Self::End => source.len(),
            }
        }
    }
    struct Case {
        header: MissingHeader,
        source: String,
        insertion: String,
    }
    let mut failures = Vec::new();
    for ending in ["\n", "\r\n", "\r"] {
        let without_end = SOURCE.strip_suffix("@End\n").expect("reference end header");
        for case in [
            Case {
                header: MissingHeader::Utf8,
                source: SOURCE
                    .strip_prefix("@UTF8\n")
                    .unwrap()
                    .replace('\n', ending),
                insertion: format!("@UTF8{ending}"),
            },
            Case {
                header: MissingHeader::End,
                source: without_end.replace('\n', ending),
                insertion: format!("@End{ending}"),
            },
            Case {
                header: MissingHeader::End,
                source: without_end
                    .strip_suffix('\n')
                    .unwrap()
                    .replace('\n', ending),
                insertion: format!("{ending}@End{ending}"),
            },
        ] {
            let mut editor = Editor::start();
            editor.initialize();
            editor.open(&case.source);
            let current = diagnostic(&editor.diagnostics(), case.header.code());
            let suggestions = actions(&mut editor, 3, &current);
            let matching: Vec<_> = suggestions
                .as_array()
                .unwrap()
                .iter()
                .filter(|action| {
                    action["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|item| item["code"] == case.header.code())
                })
                .collect();
            assert_eq!(matching.len(), 1, "{}: {suggestions}", case.header.code());
            let proposed = edit(matching[0], 1);
            assert_eq!(proposed.range.start, proposed.range.end);
            let offset = position_to_offset(&case.source, proposed.range.start);
            let expected_offset = case.header.offset(&case.source);
            assert_eq!(offset, expected_offset);
            if proposed.new_text != case.insertion {
                failures.push(format!(
                    "{} transport {ending:?}: expected {:?}, got {:?}",
                    case.header.code(),
                    case.insertion,
                    proposed.new_text
                ));
            }
            let mut repaired = case.source.clone();
            repaired.insert_str(offset, &proposed.new_text);
            editor.send(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":URI,"version":2},"contentChanges":[{
                    "range":proposed.range,"text":proposed.new_text}]}}),
            );
            assert_eq!(pull(&mut editor, 4, 2), json!([]), "{repaired:?}");
            assert_eq!(actions(&mut editor, 5, &current), Value::Null);
            shutdown(&mut editor);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
