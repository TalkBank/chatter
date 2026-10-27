//! Workspace navigation over source-bound parsed headers and utterances.

use talkbank_model::Line;
use tower_lsp::lsp_types::*;

use crate::backend::diagnostics::DocumentAnalysis;
use crate::backend::utils::{LineIndex, protocol_lines};

/// Present only model-owned symbols, using their own source for editor ranges.
/// A recovered analysis can still expose the healthy structures it retained.
pub(crate) fn workspace_symbols_for_document(
    uri: &Url,
    analysis: &DocumentAnalysis,
    query: &str,
) -> Vec<SymbolInformation> {
    let query_lower = query.to_lowercase();
    let document = analysis.source();
    let index = LineIndex::new(document);
    let file = analysis.file();
    file.lines
        .iter()
        .filter_map(|entry| {
            let span = entry.location().span()?;
            let source = document.get(span.start()..span.end())?;
            let first_line = protocol_lines(source).next()?.text();
            let (name, kind) = match entry {
                Line::Header { .. } => {
                    // This is a display label for an already-parsed header, not
                    // an independent classifier for arbitrary @-prefixed text.
                    let label = first_line
                        .split_once('\t')
                        .map_or(first_line, |(label, _)| label);
                    (label.to_owned(), SymbolKind::PROPERTY)
                }
                Line::Utterance(utterance) => (
                    format!("*{}:", utterance.main.speaker),
                    SymbolKind::FUNCTION,
                ),
            };
            if !name.to_lowercase().contains(&query_lower) {
                return None;
            }
            #[allow(deprecated)]
            Some(SymbolInformation {
                name,
                kind,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: Range::new(
                        index.offset_to_position(span.start()),
                        index.offset_to_position(span.start() + first_line.len()),
                    ),
                },
                container_name: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use talkbank_parser::TreeSitterParser;

    const SOURCE: &str =
        include_str!("../../../../../corpus/reference/core/basic-conversation.cha");

    fn symbols(source: &str, query: &str) -> Vec<SymbolInformation> {
        let uri = Url::parse("file:///reference.cha").unwrap();
        let parser = TreeSitterParser::new().unwrap();
        let analysis = DocumentAnalysis::parse(&parser, &uri, Arc::from(source), None);
        workspace_symbols_for_document(&uri, &analysis, query)
    }

    #[test]
    fn protocol_line_endings_keep_workspace_symbol_ranges() {
        let expected = symbols(SOURCE, "");
        assert!(!expected.is_empty());
        for ending in ["\r\n", "\r"] {
            assert_eq!(symbols(&SOURCE.replace('\n', ending), ""), expected);
        }
    }

    #[test]
    fn test_workspace_symbols_finds_headers_and_speakers() {
        let all = symbols(SOURCE, "");
        assert_eq!(all.len(), 13); // Nine headers and four utterances.
        let chi_only = symbols(SOURCE, "CHI");
        assert_eq!(chi_only.len(), 2); // Header payloads are not symbol names.
        assert!(chi_only.iter().all(|symbol| symbol.name == "*CHI:"));
    }

    #[test]
    fn workspace_symbols_query_is_case_insensitive() {
        assert_eq!(symbols(SOURCE, "CHI"), symbols(SOURCE, "chi"));
    }

    #[test]
    fn workspace_symbols_assigns_correct_kinds() {
        let headers = symbols(SOURCE, "@");
        assert_eq!(headers.len(), 9);
        assert!(
            headers
                .iter()
                .all(|symbol| symbol.kind == SymbolKind::PROPERTY)
        );
        let speakers = symbols(SOURCE, "*");
        assert_eq!(speakers.len(), 4);
        assert!(
            speakers
                .iter()
                .all(|symbol| symbol.kind == SymbolKind::FUNCTION)
        );
    }

    #[test]
    fn workspace_symbols_line_positions_are_correct() {
        let all = symbols(SOURCE, "");
        for (name, line) in [("@UTF8", 0), ("@Begin", 1), ("*CHI:", 8), ("@End", 12)] {
            let symbol = all.iter().find(|symbol| symbol.name == name).unwrap();
            assert_eq!(symbol.location.range.start, Position::new(line, 0));
        }
    }
}
