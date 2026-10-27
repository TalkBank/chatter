//! Visible-range filtering uses each hint's own anchor, including GRA-only views.

use super::{Editor, URI};
use serde_json::json;
use tower_lsp::lsp_types::{InlayHint, InlayHintLabel, Position, Range};

#[derive(Clone, Copy, Debug)]
enum Mismatch {
    Mor,
    Gra,
}

impl Mismatch {
    fn sources(self) -> (String, String) {
        match self {
            Self::Mor => {
                let source = include_str!(
                    "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E705_2.cha"
                )
                .replace("*CHI:\tI want cookie .", "*CHI:\tI want cookie [= 🦀] .");
                let repaired =
                    source.replace("%mor:\tpro|I v|want .", "%mor:\tpro|I v|want n|cookie .");
                assert_ne!(source, repaired);
                (source, repaired)
            }
            Self::Gra => {
                let repaired = include_str!("../../../../corpus/reference/tiers/mor-gra.cha");
                assert_eq!(repaired.matches(" 6|4|PUNCT").count(), 1);
                (repaired.replacen(" 6|4|PUNCT", "", 1), repaired.to_owned())
            }
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Mor => "*CHI:",
            Self::Gra => "%gra:",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mor => " [alignment: 3 main ↔ 2 mor]",
            Self::Gra => " [alignment: 5 gra ↔ 6 mor]",
        }
    }
}

fn hints(editor: &mut Editor, id: u64, range: Range) -> Vec<InlayHint> {
    editor.send(
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/inlayHint",
        "params":{"textDocument":{"uri":URI},"range":range}}),
    );
    let response = editor.response(id);
    assert!(response["error"].is_null(), "{response}");
    let result: Option<Vec<InlayHint>> =
        serde_json::from_value(response["result"].clone()).unwrap();
    result.unwrap_or_default()
}

#[test]
fn inlay_hints_follow_their_own_visible_range_after_unsaved_edits() {
    let mut failures = Vec::new();
    for case in [Mismatch::Mor, Mismatch::Gra] {
        for ending in ["\n", "\r\n", "\r"] {
            let (source, repaired) = case.sources();
            let source = source.replace('\n', ending);
            let repaired = repaired.replace('\n', ending);
            let (line, text) = source
                .split(ending)
                .enumerate()
                .find(|(_, text)| text.starts_with(case.prefix()))
                .unwrap();
            let anchor = Position::new(line as u32, text.encode_utf16().count() as u32);
            let next_line = Position::new(anchor.line + 1, 0);
            let line_start = Position::new(anchor.line, 0);
            let full = Range::new(Position::new(0, 0), Position::new(1000, 0));
            let mut editor = Editor::start();
            let init =
                editor.initialize_with_capabilities(json!({"textDocument":{"inlayHint":{}}}));
            let advertised = &init["result"]["capabilities"]["inlayHintProvider"];
            assert!(
                advertised == &json!(true) || advertised.is_object(),
                "{init}"
            );
            editor.open(&source);
            editor.diagnostics();
            for (index, (scope, range, expected)) in [
                ("full document", full, 1),
                ("own line", Range::new(line_start, next_line), 1),
                ("anchor included at start", Range::new(anchor, next_line), 1),
                ("anchor excluded at end", Range::new(line_start, anchor), 0),
                (
                    "prefix only",
                    Range::new(line_start, Position::new(anchor.line, 1)),
                    0,
                ),
                ("empty", Range::new(anchor, anchor), 0),
                ("reversed", Range::new(next_line, anchor), 0),
            ]
            .into_iter()
            .enumerate()
            {
                let actual = hints(&mut editor, 10 + index as u64, range);
                if actual.len() != expected || actual.iter().any(|hint|
                    hint.position != anchor || !matches!(&hint.label, InlayHintLabel::String(label) if label == case.label())) {
                    failures.push(format!("{case:?} {ending:?} {scope}: expected {expected} at {anchor:?}, got {actual:?}"));
                }
            }
            editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange",
                "params":{"textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":repaired}]}}));
            assert_eq!(editor.diagnostics(), json!([]));
            assert!(hints(&mut editor, 30, full).is_empty());
            editor.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange",
                "params":{"textDocument":{"uri":URI,"version":3},"contentChanges":[{"text":source}]}}));
            editor.diagnostics();
            let restored = hints(&mut editor, 31, Range::new(anchor, next_line));
            if restored.len() != 1
                || restored[0].position != anchor
                || !matches!(&restored[0].label, InlayHintLabel::String(label) if label == case.label())
            {
                failures.push(format!("{case:?} {ending:?} restored: {restored:?}"));
            }
            editor.close();
            assert!(hints(&mut editor, 32, full).is_empty());
            editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
            assert!(editor.response(99)["error"].is_null());
            editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
            editor.expect_exit(0);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
