//! Current-buffer declaration admission through the real execute-command wire.

use serde_json::{Value, json};

use super::Editor;

const URI: &str = "file:///speaker-admission.cha";
const COMPLETE: &str =
    include_str!("../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E513_2.cha");
const MISSING_ROLE: &str =
    include_str!("../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E513_1.cha");
const REJECTED: &str =
    include_str!("../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E506_3.cha");

enum ExpectedDeclaration {
    Complete,
    MissingRole,
    Unavailable(&'static str),
}

fn speakers(editor: &mut Editor, id: u64) -> Value {
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"workspace/executeCommand",
        "params":{"command":"talkbank/getSpeakers","arguments":[URI]}}),
    );
    editor.response(id)
}

fn assert_declaration(response: Value, expected: ExpectedDeclaration) {
    match expected {
        ExpectedDeclaration::Complete => {
            assert!(response.get("error").is_none(), "{response}");
            assert_eq!(
                response["result"],
                json!([
                    {"code":"CHI","name":"Ruth","role":"Target_Child"}
                ])
            );
        }
        ExpectedDeclaration::MissingRole => {
            assert!(response.get("error").is_none(), "{response}");
            assert_eq!(
                response["result"],
                json!([
                    {"code":"CHI","name":null,"role":null}
                ])
            );
        }
        ExpectedDeclaration::Unavailable(reason) => {
            assert!(response.get("result").is_none(), "{response}");
            assert_eq!(response["error"]["code"], -32001);
            assert_eq!(
                response["error"]["message"],
                format!("Speaker error: @Participants declaration is {reason}")
            );
        }
    }
}

#[test]
fn speaker_declaration_damage_repair_and_reopen_use_current_buffer() {
    for newline in ["\n", "\r\n"] {
        let mut editor = Editor::start();
        editor.initialize();
        editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":URI,"languageId":"chat","version":1,
                "text":COMPLETE.replace('\n',newline)}}}));
        assert_declaration(speakers(&mut editor, 2), ExpectedDeclaration::Complete);

        let declaration = "@Participants:\tCHI Ruth Target_Child\n";
        let absent = COMPLETE.replace(declaration, "");
        let repeated = COMPLETE.replace(declaration, &declaration.repeat(2));
        let revisions = [
            (MISSING_ROLE, ExpectedDeclaration::MissingRole),
            (
                repeated.as_str(),
                ExpectedDeclaration::Unavailable("repeated"),
            ),
            (REJECTED, ExpectedDeclaration::Unavailable("unparsed")),
            (absent.as_str(), ExpectedDeclaration::Unavailable("absent")),
            (COMPLETE, ExpectedDeclaration::Complete),
        ];
        for (index, (source, expected)) in revisions.into_iter().enumerate() {
            editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange",
                "params":{"textDocument":{"uri":URI,"version":index + 2},
                    "contentChanges":[{"text":source.replace('\n',newline)}]}}));
            // Request immediately: do not let debounced diagnostics refresh a stale cache.
            assert_declaration(speakers(&mut editor, index as u64 + 3), expected);
        }
        editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose",
            "params":{"textDocument":{"uri":URI}}}));
        editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":URI,"languageId":"chat","version":1,
                "text":REJECTED.replace('\n',newline)}}}));
        assert_declaration(
            speakers(&mut editor, 8),
            ExpectedDeclaration::Unavailable("unparsed"),
        );
        editor.send(json!({"jsonrpc":"2.0","id":9,"method":"shutdown"}));
        assert!(editor.response(9).get("error").is_none());
        editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
        editor.expect_exit(0);
    }
}
