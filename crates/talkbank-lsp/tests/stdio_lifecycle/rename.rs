//! Rename edits are tied to the current open document and negotiated capability.
use super::{Editor, SOURCE, URI};
use serde_json::{Value, json};
use talkbank_lsp::backend::utils::{offset_to_position, position_to_offset};
use tower_lsp::lsp_types::{DocumentChanges, OneOf, Position, TextEdit, WorkspaceEdit};

fn rename(editor: &mut Editor, id: u64, position: Position) -> Value {
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/rename","params":{
        "textDocument":{"uri":URI},"position":position,"newName":"KID"}}),
    );
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    response["result"].clone()
}

fn edits(result: Value, version: i32) -> Vec<TextEdit> {
    let workspace: WorkspaceEdit = serde_json::from_value(result).unwrap();
    assert!(workspace.changes.is_none(), "no unversioned fallback");
    let Some(DocumentChanges::Edits(mut changes)) = workspace.document_changes else {
        panic!("versioned edits required")
    };
    assert_eq!(changes.len(), 1);
    let change = changes.remove(0);
    assert_eq!(change.text_document.uri.as_str(), URI);
    assert_eq!(change.text_document.version, Some(version));
    change
        .edits
        .into_iter()
        .map(|edit| match edit {
            OneOf::Left(edit) => edit,
            OneOf::Right(_) => panic!("unexpected annotated edit"),
        })
        .collect()
}

fn shutdown(editor: &mut Editor) {
    editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
    assert!(editor.response(99)["error"].is_null());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}

#[test]
fn rename_requires_revision_capability_even_for_direct_requests() {
    for capabilities in [
        json!({}),
        json!({"workspace":{"workspaceEdit":{"documentChanges":false}}}),
    ] {
        let mut editor = Editor::start();
        let initialized = editor.initialize_with_capabilities(capabilities);
        assert_eq!(
            initialized["result"]["capabilities"]["renameProvider"],
            false
        );
        editor.open(SOURCE);
        assert_eq!(editor.diagnostics(), json!([]));
        let position = offset_to_position(SOURCE, SOURCE.find("*CHI:").unwrap() + 1);
        assert_eq!(rename(&mut editor, 3, position), Value::Null);
        editor.send(
            json!({"jsonrpc":"2.0","id":4,"method":"textDocument/prepareRename","params":{
            "textDocument":{"uri":URI},"position":position}}),
        );
        assert_eq!(editor.response(4)["result"], Value::Null);
        shutdown(&mut editor);
    }
}

#[test]
fn rename_tracks_edits_reopen_and_utf16_source_without_rewriting_prose() {
    let mut editor = Editor::start();
    let initialized = editor.initialize_with_capabilities(
        json!({"workspace":{"workspaceEdit":{"documentChanges":true}}}),
    );
    assert_eq!(
        initialized["result"]["capabilities"]["renameProvider"]["prepareProvider"],
        true
    );
    let source = SOURCE
        .replace("Minimal valid CHAT file", "CHI 🦀: Minimal valid CHAT file")
        .replace('\n', "\r\n");
    editor.open(&source);
    assert_eq!(editor.diagnostics(), json!([]));
    let position = offset_to_position(&source, source.find("*CHI:").unwrap() + 1);
    let first = edits(rename(&mut editor, 3, position), 1);
    assert_eq!(first.len(), 4);
    assert_eq!(
        rename(&mut editor, 4, Position::new(u32::MAX, u32::MAX)),
        Value::Null
    );
    let changed = source.replace("@End", "@Comment:\tcontinued\r\n@End");
    // Move all speaker positions and request before the diagnostic debounce.
    let changed = changed.replace("@Participants:", "@Comment:\t🦀\r\n@Participants:");
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":changed}]}}),
    );
    let position = offset_to_position(&changed, changed.find("*CHI:").unwrap() + 1);
    let mut current = edits(rename(&mut editor, 5, position), 2);
    assert_eq!(current.len(), first.len());
    assert_ne!(current[0].range, first[0].range);
    current.sort_by_key(|edit| std::cmp::Reverse(position_to_offset(&changed, edit.range.start)));
    let mut applied = changed.clone();
    for edit in current {
        let range = position_to_offset(&changed, edit.range.start)
            ..position_to_offset(&changed, edit.range.end);
        assert_eq!(&changed[range.clone()], "CHI");
        applied.replace_range(range, &edit.new_text);
    }
    let expected = changed
        .replace("@Participants:\tCHI", "@Participants:\tKID")
        .replace("|CHI|", "|KID|")
        .replace("*CHI:", "*KID:");
    assert_eq!(applied, expected);
    assert!(applied.contains("CHI 🦀: Minimal"));
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":3},"contentChanges":[{"text":applied}]}}),
    );
    editor.send(json!({"jsonrpc":"2.0","id":6,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}));
    let diagnosed = editor.response(6);
    assert_eq!(diagnosed["result"]["resultId"], "3");
    assert_eq!(diagnosed["result"]["items"], json!([]));
    editor.close();
    assert_eq!(rename(&mut editor, 7, position), Value::Null);
    editor.open(&source);
    let reopened = offset_to_position(&source, source.find("*CHI:").unwrap() + 1);
    assert_eq!(edits(rename(&mut editor, 8, reopened), 1).len(), 4);
    shutdown(&mut editor);
}

#[test]
fn rename_refuses_invalid_targets_collisions_and_incomplete_syntax() {
    let mut editor = Editor::start();
    editor.initialize_with_capabilities(
        json!({"workspace":{"workspaceEdit":{"documentChanges":true}}}),
    );
    editor.open(SOURCE);
    assert_eq!(editor.diagnostics(), json!([]));
    let position = offset_to_position(SOURCE, SOURCE.find("*CHI:").unwrap() + 1);
    for (index, name) in ["", "A B", "A|B", "A:B", "\n@End", "CHILDREN", "🦀", "MOT"]
        .into_iter()
        .enumerate()
    {
        let id = 10 + index as u64;
        editor.send(
            json!({"jsonrpc":"2.0","id":id,"method":"textDocument/rename","params":{
            "textDocument":{"uri":URI},"position":position,"newName":name}}),
        );
        let response = editor.response(id);
        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert!(response.get("result").is_none(), "no edits on refusal");
    }
    editor.send(
        json!({"jsonrpc":"2.0","id":20,"method":"textDocument/rename","params":{
        "textDocument":{"uri":URI},"position":position,"newName":"CHI"}}),
    );
    let unchanged = editor.response(20);
    assert!(unchanged["error"].is_null());
    assert!(unchanged["result"].is_null());
    // Missing metadata is a semantic finding, not lost syntax coverage.
    let missing_metadata = SOURCE.replacen("@UTF8\n", "", 1);
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":missing_metadata}]}}),
    );
    let position = offset_to_position(
        &missing_metadata,
        missing_metadata.find("*CHI:").unwrap() + 1,
    );
    assert_eq!(edits(rename(&mut editor, 21, position), 2).len(), 4);
    let damaged = SOURCE.replacen("*CHI:", "*CHI", 1);
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":URI,"version":3},"contentChanges":[{"text":damaged}]}}),
    );
    let position = offset_to_position(&damaged, damaged.find("*CHI").unwrap() + 1);
    editor.send(
        json!({"jsonrpc":"2.0","id":22,"method":"textDocument/rename","params":{
        "textDocument":{"uri":URI},"position":position,"newName":"KID"}}),
    );
    assert_eq!(editor.response(22)["error"]["code"], -32602);
    editor.send(
        json!({"jsonrpc":"2.0","id":23,"method":"textDocument/prepareRename","params":{
        "textDocument":{"uri":URI},"position":position}}),
    );
    assert!(editor.response(23)["result"].is_null());
    shutdown(&mut editor);
}

#[test]
fn rename_from_personal_headers_preserves_payloads_and_current_revision() {
    const PERSONAL: &str =
        include_str!("../../../../corpus/reference/core/headers-speaker-info.cha");
    for newline in ["\n", "\r\n"] {
        let mut editor = Editor::start();
        editor.initialize_with_capabilities(
            json!({"workspace":{"workspaceEdit":{"documentChanges":true}}}),
        );
        for (prefix, original) in [
            ("@Birth of ", "MOT"),
            ("@Birthplace of ", "MOT"),
            ("@L1 of ", "F_A_T"),
        ] {
            let source = PERSONAL.replace('\n', newline);
            editor.open(&source);
            assert_eq!(editor.diagnostics(), json!([]));
            // Move the headers in an unsaved revision and retain identity-like prose.
            let changed = source.replace(
                "@Participants:",
                &format!("@Comment:\t🦀 MOT F_A_T{newline}@Participants:"),
            );
            editor.send(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":changed}]}}),
            );
            let offset = changed.find(&format!("{prefix}{original}:")).unwrap() + prefix.len();
            let position = offset_to_position(&changed, offset);
            editor.send(
                json!({"jsonrpc":"2.0","id":30,"method":"textDocument/prepareRename","params":{
                "textDocument":{"uri":URI},"position":position}}),
            );
            let prepared = editor.response(30);
            assert!(prepared["error"].is_null(), "{prepared}");
            assert_eq!(prepared["result"]["placeholder"], original);
            let range: tower_lsp::lsp_types::Range =
                serde_json::from_value(prepared["result"]["range"].clone()).unwrap();
            assert_eq!(position_to_offset(&changed, range.start), offset);
            assert_eq!(
                position_to_offset(&changed, range.end),
                offset + original.len()
            );
            let mut current = edits(rename(&mut editor, 31, position), 2);
            current.sort_by_key(|edit| {
                std::cmp::Reverse(position_to_offset(&changed, edit.range.start))
            });
            let mut applied = changed.clone();
            for edit in current {
                let range = position_to_offset(&changed, edit.range.start)
                    ..position_to_offset(&changed, edit.range.end);
                assert_eq!(&changed[range.clone()], original);
                applied.replace_range(range, &edit.new_text);
            }
            let expected = changed
                .replace(&format!(", {original} "), ", KID ")
                .replace(&format!("|{original}|"), "|KID|")
                .replace(&format!("*{original}:"), "*KID:")
                .replace(&format!("of {original}:"), "of KID:");
            assert_eq!(applied, expected, "{prefix}, {newline:?}");
            editor.send(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":URI,"version":3},"contentChanges":[{"text":applied}]}}),
            );
            editor.send(
                json!({"jsonrpc":"2.0","id":32,"method":"textDocument/diagnostic","params":{
                "textDocument":{"uri":URI}}}),
            );
            let diagnosed = editor.response(32);
            assert_eq!(diagnosed["result"]["resultId"], "3");
            assert_eq!(diagnosed["result"]["items"], json!([]));
            editor.close();
        }
        shutdown(&mut editor);
    }
}
