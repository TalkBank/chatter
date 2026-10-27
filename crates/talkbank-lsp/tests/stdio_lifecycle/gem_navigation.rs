//! Gem outline and folding follow parsed label pairing in the current buffer.

use super::Editor;
use serde_json::json;
use tower_lsp::lsp_types::{CodeLens, DocumentSymbol, FoldingRange, Position, SymbolKind};

const SOURCE: &str = include_str!("../../../../corpus/reference/edge-cases/postcodes-and-gems.cha");
const MULTILINE: &str =
    include_str!("../../../../corpus/reference/edge-cases/multiline-paired-gems.cha");
const URI: &str = "file:///gem-navigation.cha";

fn gem_symbols(symbols: &[DocumentSymbol]) -> Vec<&DocumentSymbol> {
    let mut result = Vec::new();
    for symbol in symbols {
        if symbol.kind == SymbolKind::EVENT {
            result.push(symbol);
        }
        if let Some(children) = &symbol.children {
            result.extend(gem_symbols(children));
        }
    }
    result
}

#[test]
fn gem_navigation_uses_labels_bare_markers_and_multiline_extents() {
    let bare =
        SOURCE
            .replacen("@Bg:\tmorning play", "@Bg", 1)
            .replacen("@Eg:\tmorning play", "@Eg", 1);
    let mismatch = SOURCE.replacen("@Eg:\tmorning play", "@Eg:\tother", 1);
    let overlapping = SOURCE.replace(
        "@Eg:\tmorning play\n@Bg:\treading time",
        "@Bg:\treading time\n@Eg:\tmorning play",
    );
    let mut failures = Vec::new();
    for ending in ["\n", "\r\n", "\r"] {
        let mut editor = Editor::start();
        editor.initialize();
        editor.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":URI,"languageId":"chat","version":1,
                "text":SOURCE.replace('\n', ending)}}}),
        );
        for (case, (name, source, expected, titles)) in [
            (
                "bare",
                bare.as_str(),
                &[(8, 14), (15, 19)][..],
                &["CHI: 5 utterances", "MOT: 3 utterances"][..],
            ),
            (
                "mismatch",
                mismatch.as_str(),
                &[(15, 19)][..],
                &["CHI: 5 utterances", "MOT: 3 utterances"][..],
            ),
            (
                "multiline",
                MULTILINE,
                &[(6, 11)][..],
                &["CHI: 1 utterance"][..],
            ),
            (
                "overlapping",
                overlapping.as_str(),
                &[(8, 15), (14, 19)][..],
                &["CHI: 5 utterances", "MOT: 3 utterances"][..],
            ),
        ]
        .into_iter()
        .enumerate()
        {
            editor.send(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":URI,"version":case + 2},
                "contentChanges":[{"text":source.replace('\n', ending)}]}}),
            );
            editor.send(
                json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
                "params":{"textDocument":{"uri":URI}}}),
            );
            let response = editor.response(2);
            assert!(response["error"].is_null(), "{response}");
            let outline: Vec<DocumentSymbol> =
                serde_json::from_value(response["result"].clone()).unwrap();
            let gems = gem_symbols(&outline);
            let mut outline_starts: Vec<_> = gems.iter().map(|gem| gem.range.start.line).collect();
            outline_starts.sort();
            editor.send(
                json!({"jsonrpc":"2.0","id":3,"method":"textDocument/foldingRange",
                "params":{"textDocument":{"uri":URI}}}),
            );
            let response = editor.response(3);
            assert!(response["error"].is_null(), "{response}");
            let folds: Vec<FoldingRange> =
                serde_json::from_value(response["result"].clone()).unwrap();
            // These controls have no dependent tiers, so only the initial
            // header fold starts at zero; all other folds must be paired gems.
            let mut actual: Vec<_> = folds
                .iter()
                .filter(|fold| fold.start_line != 0)
                .map(|fold| (fold.start_line, fold.end_line))
                .collect();
            actual.sort();
            if actual != expected
                || outline_starts != expected.iter().map(|(start, _)| *start).collect::<Vec<_>>()
            {
                failures.push(format!(
                    "{name} {ending:?}: folds {actual:?}, {} outline gems; expected {expected:?}",
                    gems.len()
                ));
            }
            editor.send(
                json!({"jsonrpc":"2.0","id":4,"method":"textDocument/codeLens",
                "params":{"textDocument":{"uri":URI}}}),
            );
            let response = editor.response(4);
            assert!(response["error"].is_null(), "{response}");
            let lenses: Vec<CodeLens> = serde_json::from_value(response["result"].clone()).unwrap();
            let mut actual_titles: Vec<_> = lenses
                .iter()
                .map(|lens| {
                    assert_eq!(lens.range.start, Position::new(3, 0));
                    lens.command.as_ref().expect("count command").title.as_str()
                })
                .collect();
            actual_titles.sort();
            assert_eq!(actual_titles, titles, "{name} {ending:?}");
        }
        editor.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
        assert!(editor.response(99)["error"].is_null());
        editor.send(json!({"jsonrpc":"2.0","method":"exit"}));
        editor.expect_exit(0);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
