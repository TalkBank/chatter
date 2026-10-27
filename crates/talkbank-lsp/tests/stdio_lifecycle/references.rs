//! Speaker reference scope remains distinct from the participant declaration.

use super::{Editor, SOURCE, URI};
use serde_json::json;
use talkbank_lsp::backend::utils::{offset_to_position, position_to_offset};
use tower_lsp::lsp_types::{Location, Position};

fn references(editor: &mut Editor, id: u64, position: Position, include: bool) -> Vec<Location> {
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/references",
        "params":{"textDocument":{"uri":URI},"position":position,
            "context":{"includeDeclaration":include}}}),
    );
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    let result: Option<Vec<Location>> = serde_json::from_value(response["result"].clone()).unwrap();
    result.unwrap_or_default()
}

#[test]
fn references_honor_declaration_scope_after_unsaved_edits() {
    let mut failures = Vec::new();
    for ending in ["\n", "\r\n", "\r"] {
        let mut editor = Editor::start();
        editor.initialize();
        let source = SOURCE.replace("@Participants:", "@Comment:\tCHI 🦀\n@Participants:");
        let id_line = source
            .lines()
            .find(|line| line.starts_with("@ID:") && line.contains("|CHI|"))
            .unwrap();
        let declaration_only = source
            .replace(&format!("{id_line}\n"), "")
            .replace("*CHI:", "*MOT:");
        let source = source.replace('\n', ending);
        let declaration_only = declaration_only.replace('\n', ending);
        editor.open(&source);
        for (index, text) in [&source, &declaration_only, &source]
            .into_iter()
            .enumerate()
        {
            if index > 0 {
                editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange",
                    "params":{"textDocument":{"uri":URI,"version":index + 1},
                        "contentChanges":[{"text":text}]}}));
            }
            let declaration = text.find("@Participants:\tCHI").unwrap() + "@Participants:\t".len();
            let mut uses: Vec<_> = text.match_indices("|CHI|").map(|(at, _)| at + 1).collect();
            uses.extend(text.match_indices("*CHI:").map(|(at, _)| at + 1));
            uses.sort_unstable();
            let mut all = uses.clone();
            all.push(declaration);
            all.sort_unstable();
            let definition_cursor = text.find("*CHI:").map_or(declaration, |at| at + 1);
            let definition_id = 20 + index as u64;
            editor.send(json!({"jsonrpc":"2.0","id":definition_id,"method":"textDocument/definition",
                "params":{"textDocument":{"uri":URI},"position":offset_to_position(text, definition_cursor)}}));
            let definition = editor.response(definition_id);
            assert!(definition["error"].is_null(), "{definition}");
            assert!(
                definition["result"].is_object(),
                "missing participant definition: {definition}"
            );
            let definition: Location =
                serde_json::from_value(definition["result"].clone()).unwrap();
            assert_eq!(definition.uri.as_str(), URI);
            assert_eq!(
                position_to_offset(text, definition.range.start),
                declaration
            );
            assert_eq!(
                position_to_offset(text, definition.range.end),
                declaration + 3
            );
            // Querying from a declaration must still find its uses. @ID metadata
            // is a reference to the participant, not another declaration.
            let cursor = offset_to_position(text, declaration + 1);
            for (include, expected) in [(false, &uses), (true, &all)] {
                let id = 10 + (index as u64 * 2) + u64::from(include);
                let found = references(&mut editor, id, cursor, include);
                let mut actual: Vec<_> = found
                    .iter()
                    .map(|location| {
                        assert_eq!(location.uri.as_str(), URI);
                        let start = position_to_offset(text, location.range.start);
                        let end = position_to_offset(text, location.range.end);
                        assert_eq!(&text[start..end], "CHI");
                        start
                    })
                    .collect();
                actual.sort_unstable();
                if &actual != expected {
                    failures.push(format!(
                        "{ending:?}, revision {}, include={include}: {actual:?} != {expected:?}",
                        index + 1
                    ));
                }
            }
        }
        editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose",
            "params":{"textDocument":{"uri":URI}}}));
        assert!(references(&mut editor, 30, Position::new(0, 0), true).is_empty());
        editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
        assert!(editor.response(99)["error"].is_null());
        editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
        editor.expect_exit(0);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
