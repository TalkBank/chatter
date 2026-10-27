//! Unicode search results retain source coordinates over the real protocol.

use super::Editor;
use serde_json::{Value, json};

const SOURCE: &str = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
const URI: &str = "file:///scoped-search.cha";
const COMMENT: &str = "%com:\t😀 İ K café 😀 éé banana .";

fn search(editor: &mut Editor, id: u64, query: &str, regex: bool) -> Value {
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"workspace/executeCommand",
        "params":{"command":"talkbank/scopedFind","arguments":[{
            "uri":URI,"query":query,"regex":regex,"scope":"com","speakers":["CHI"]}]}}),
    );
    editor.response(id)
}

#[test]
fn scoped_find_unicode_results_follow_source_and_ranged_edits() {
    let main_line = SOURCE
        .lines()
        .position(|line| line.starts_with("*CHI:"))
        .unwrap();
    let mut source: Vec<_> = SOURCE.lines().collect();
    source.insert(main_line + 1, COMMENT);
    let line = main_line + 1;
    for ending in ["\n", "\r\n", "\r"] {
        let mut editor = Editor::start();
        editor.initialize();
        let text = source.join(ending) + ending;
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":URI,"languageId":"chat","version":1,"text":text}}}),
        );
        for (index, (query, regex, columns)) in [
            ("CAFÉ", false, vec![(13, 4)]),
            ("😀", false, vec![(6, 2), (18, 2)]),
            ("😀", true, vec![(6, 2), (18, 2)]),
            ("k", false, vec![(11, 1)]),
            ("é", false, vec![(16, 1), (21, 1), (22, 1)]),
            ("ana", false, vec![(25, 3), (27, 3)]),
            (".", false, vec![(31, 1)]),
            ("", false, vec![]),
        ]
        .into_iter()
        .enumerate()
        {
            let response = search(&mut editor, index as u64 + 2, query, regex);
            assert!(response.get("error").is_none(), "{response}");
            let expected: Vec<_> = columns
                .into_iter()
                .map(|(character, length)| {
                    json!({
                        "range":{"start":{"line":line,"character":character},
                                 "end":{"line":line,"character":character+length}},
                        "tier":"com","speaker":"CHI","line_text":COMMENT
                    })
                })
                .collect();
            assert_eq!(
                response["result"],
                json!(expected),
                "{ending:?} query={query:?}"
            );
        }
        let invalid = search(&mut editor, 20, "[", true);
        assert!(invalid.get("error").is_some(), "{invalid}");
        assert!(invalid.get("result").is_none(), "{invalid}");
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":2},"contentChanges":[{
            "range":{"start":{"line":line,"character":13},"end":{"line":line,"character":17}},
            "text":"cafe"}]}}),
        );
        assert_eq!(search(&mut editor, 21, "CAFÉ", false)["result"], json!([]));
        let changed = search(&mut editor, 22, "CAFE", false);
        assert!(changed.get("error").is_none(), "{changed}");
        assert_eq!(
            changed["result"],
            json!([{
                "range":{"start":{"line":line,"character":13},"end":{"line":line,"character":17}},
                "tier":"com","speaker":"CHI",
                "line_text":COMMENT.replace("café", "cafe")
            }])
        );
        editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
        assert!(editor.response(99).get("error").is_none());
        editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
        editor.expect_exit(0);
    }
}

#[test]
fn scoped_find_multiline_ranges_apply_to_the_original_source() {
    let reference = include_str!("../../../../corpus/reference/core/multiline-continuation.cha");
    let source = reference.replace("gesturing\n\ttoward", "😀 gesturing\n\t😀 toward");
    assert_ne!(
        source, reference,
        "the source-owned continuation witness moved"
    );
    let line = source
        .lines()
        .position(|line| line.starts_with("%com:"))
        .unwrap();
    let comment = source.lines().nth(line).unwrap().to_owned();
    let prefix = comment.split_once("gesturing").unwrap().0;
    let column = prefix.encode_utf16().count();
    let expected_range = json!({
        "start":{"line":line,"character":column},
        "end":{"line":line+1,"character":10}
    });
    for ending in ["\n", "\r\n", "\r"] {
        let mut editor = Editor::start();
        editor.initialize();
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":URI,"languageId":"chat","version":1,
            "text":source.replace('\n', ending)}}}),
        );
        let pattern = r"gesturing\s+😀 toward";
        let expected = json!([{"range":expected_range,"tier":"com","speaker":"CHI",
            "line_text":comment}]);
        let found = search(&mut editor, 2, pattern, true);
        assert_eq!(found["result"], expected, "{ending:?}: {found}");
        let literal = format!("gesturing{ending}\t😀 toward");
        assert_eq!(search(&mut editor, 3, &literal, false)["result"], expected);
        let zero = search(&mut editor, 4, "^", true);
        assert_eq!(
            zero["result"][0]["range"],
            json!({
            "start":{"line":line,"character":0},"end":{"line":line,"character":0}})
        );
        if ending == "\r\n" {
            for (id, partial) in [(5, r"gesturing\r"), (6, r"\n\t😀 toward")] {
                let refused = search(&mut editor, id, partial, true);
                assert!(refused.get("error").is_some(), "{refused}");
                assert!(
                    refused["error"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("complete line endings"),
                    "{refused}"
                );
                assert!(refused.get("result").is_none(), "{refused}");
            }
        }
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":2},"contentChanges":[{
            "range":found["result"][0]["range"],"text":"gesturing toward"}]}}),
        );
        assert_eq!(search(&mut editor, 7, pattern, true)["result"], json!([]));
        assert_eq!(
            search(&mut editor, 8, "gesturing toward", false)["result"],
            json!([{
                "range":{"start":{"line":line,"character":column},
                         "end":{"line":line,"character":column+16}},
                "tier":"com","speaker":"CHI","line_text":format!("{prefix}gesturing toward the door")
            }])
        );
        editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
        assert!(editor.response(99).get("error").is_none());
        editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
        editor.expect_exit(0);
    }
}
