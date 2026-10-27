//! A command failure must never masquerade as a successful replacement string.

use super::Editor;
use serde_json::{Value, json};

const URI: &str = "file:///command-admission.cha";
const SOURCE: &str = include_str!("../../../../corpus/reference/core/basic-conversation.cha");

fn execute(editor: &mut Editor, id: u64, command: &str, arguments: Value) -> Value {
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"workspace/executeCommand",
        "params":{"command":command,"arguments":arguments}}),
    );
    editor.response(id)
}

fn finish(mut editor: Editor) {
    editor.send(json!({"jsonrpc":"2.0","id":999,"method":"shutdown"}));
    assert!(editor.response(999).get("error").is_none());
    editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
    editor.expect_exit(0);
}

fn error(response: &Value, code: i64) -> bool {
    response.get("result").is_none()
        && response["error"]["code"] == code
        && response["error"]["message"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
}

#[test]
fn every_advertised_command_refuses_malformed_arguments() {
    let mut editor = Editor::start();
    let initialized = editor.initialize_with_capabilities(json!({}));
    let commands = initialized["result"]["capabilities"]["executeCommandProvider"]["commands"]
        .as_array()
        .expect("advertised command list");
    assert!(!commands.is_empty());
    let mut failures = Vec::new();
    for (index, command) in commands.iter().enumerate() {
        let command = command.as_str().unwrap();
        let response = execute(&mut editor, index as u64 + 10, command, json!([]));
        if !error(&response, -32602) {
            failures.push(format!("{command}: {response}"));
        }
    }
    let unknown = execute(&mut editor, 100, "talkbank/unknownCommand", json!([]));
    if !error(&unknown, -32602) {
        failures.push(format!("unknown command: {unknown}"));
    }
    finish(editor);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[derive(Clone, Copy)]
enum Payload {
    Array,
    Object,
    Transcript,
}

#[test]
fn command_failures_remain_errors_and_later_successes_keep_their_payloads() {
    let mut editor = Editor::start();
    editor.initialize();
    let line = SOURCE
        .lines()
        .position(|line| line.starts_with("*CHI:"))
        .unwrap();
    let cases = [
        (
            "talkbank/showDependencyGraph",
            json!([URI, {"line":line,"character":0}]),
            Payload::Object,
        ),
        (
            "talkbank/getAlignmentSidecar",
            json!([URI]),
            Payload::Object,
        ),
        ("talkbank/getParticipants", json!([URI]), Payload::Array),
        ("talkbank/getSpeakers", json!([URI]), Payload::Array),
        (
            "talkbank/filterDocument",
            json!([{"uri":URI,"speakers":["CHI"]}]),
            Payload::Transcript,
        ),
        ("talkbank/getUtterances", json!([URI]), Payload::Array),
        (
            "talkbank/scopedFind",
            json!([{"uri":URI,"query":"cookies","scope":"main"}]),
            Payload::Array,
        ),
    ];
    let mut failures = Vec::new();
    for (index, (command, arguments, _)) in cases.iter().enumerate() {
        let response = execute(&mut editor, index as u64 + 10, command, arguments.clone());
        if !error(&response, -32001) {
            failures.push(format!("unopened {command}: {response}"));
        }
    }
    editor.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":URI,"languageId":"chat","version":1,"text":SOURCE}}}),
    );
    for (index, (command, arguments, payload)) in cases.iter().enumerate() {
        let response = execute(&mut editor, index as u64 + 30, command, arguments.clone());
        let value = &response["result"];
        let valid = match payload {
            Payload::Array => value.is_array(),
            Payload::Object => value.is_object(),
            Payload::Transcript => value.as_str().is_some_and(|text| {
                text.starts_with("@UTF8") && text.contains("*CHI:") && !text.contains("*MOT:")
            }),
        };
        if response.get("error").is_some() || !valid {
            failures.push(format!("opened {command}: {response}"));
        }
    }
    let no_utterance = execute(
        &mut editor,
        50,
        "talkbank/showDependencyGraph",
        json!([URI, {"line":0,"character":0}]),
    );
    if !error(&no_utterance, -32001) {
        failures.push(format!("no utterance: {no_utterance}"));
    }
    let mut fields = json!({"language":"","corpus":"sample","speaker":"CHI",
        "age":"","sex":"","group":"","ses":"","role":"Child","education":"","custom":""});
    let refused = execute(&mut editor, 51, "talkbank/formatIdLine", json!([fields]));
    if !error(&refused, -32001) {
        failures.push(format!("invalid ID fields: {refused}"));
    }
    fields["language"] = json!("eng");
    let formatted = execute(&mut editor, 52, "talkbank/formatIdLine", json!([fields]));
    if formatted.get("error").is_some()
        || !formatted["result"]
            .as_str()
            .is_some_and(|line| line.starts_with("@ID:\teng|sample|CHI|"))
    {
        failures.push(format!("valid ID fields: {formatted}"));
    }
    let bullet = execute(
        &mut editor,
        53,
        "talkbank/formatBulletLine",
        json!([{"prev_ms":100,"current_ms":200,"speaker":"CHI"}]),
    );
    if bullet.get("error").is_some()
        || bullet["result"] != json!({"bullet":"•100_200•","new_line":"*CHI:\t"})
    {
        failures.push(format!("bullet: {bullet}"));
    }
    editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose",
        "params":{"textDocument":{"uri":URI}}}));
    let closed = execute(
        &mut editor,
        54,
        "talkbank/filterDocument",
        json!([{"uri":URI,"speakers":["CHI"]}]),
    );
    if !error(&closed, -32001) {
        failures.push(format!("closed document: {closed}"));
    }
    finish(editor);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
