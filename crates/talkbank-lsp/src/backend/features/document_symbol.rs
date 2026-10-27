//! Document symbol provider for CHAT files.
//!
//! Returns a two-level symbol tree for VS Code's Outline panel, breadcrumb bar,
//! and `Cmd+Shift+O` symbol picker:
//!
//! ```text
//! Chat Transcript         (Module, whole file)
//!   Headers               (Namespace, @Begin → first utterance)
//!   Utterances            (Namespace, first utterance → @End)
//!     *CHI: hello         (String, line N)
//!     *MOT: hi            (String, line M)
//! ```
//!
//! Spans are computed from the byte-offset `Span` stored in each utterance's
//! `main` tier, converted to LSP `Position` (line/character) via the existing
//! `offset_to_position` utility.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::Span;
use tower_lsp::lsp_types::*;

use super::gem_navigation::GemNavigation;
use crate::backend::diagnostics::DocumentAnalysis;
use crate::backend::utils::LineIndex;

/// Build the document symbol tree for a CHAT file.
///
/// The analysis owns both the parsed model and its exact source text.
///
/// # Returns
/// A flat list whose first element is the top-level "Chat Transcript" module
/// containing nested "Headers" and "Utterances" namespaces, each of which
/// contains one child symbol per utterance.
pub(crate) fn document_symbol(analysis: &DocumentAnalysis) -> Option<DocumentSymbolResponse> {
    let chat_file = analysis.file();
    let document = analysis.source();
    let doc_len = document.len();
    let index = LineIndex::new(document);

    // Compute the full-file range (start of file → end of file).
    let file_start = Position {
        line: 0,
        character: 0,
    };
    let file_end = index.offset_to_position(doc_len);
    let file_range = Range {
        start: file_start,
        end: file_end,
    };

    // Build one child symbol per utterance.
    let mut utterance_children: Vec<DocumentSymbol> = Vec::new();

    // Track the byte offset of the first utterance to delimit the "Headers" section.
    let mut first_utterance_offset: Option<usize> = None;

    for utterance in chat_file.utterances() {
        // An utterance with no source location has no symbol range.
        let Some(main_span) = utterance.main.location.span() else {
            continue;
        };
        if first_utterance_offset.is_none() {
            first_utterance_offset = Some(main_span.start());
        }

        // Compute the full utterance block extent (main + all dependent tiers).
        let block_start = main_span.start();
        let block_end = utterance_block_end(utterance, main_span);

        let start_pos = index.offset_to_position(block_start);
        let end_pos = index.offset_to_position(block_end);

        // Label: "*SPEAKER: …" truncated at 60 chars for readability.
        let speaker = utterance.main.speaker.as_str();
        // Collect the first line of text from the main tier for the label.
        let label_text = first_line_at_offset(document, block_start, 60);
        let label = if label_text.is_empty() {
            format!("*{speaker}:")
        } else {
            label_text
        };

        #[allow(deprecated)] // `deprecated` field required by LSP spec
        let sym = DocumentSymbol {
            name: label,
            detail: None,
            kind: SymbolKind::STRING,
            tags: None,
            deprecated: None,
            range: Range {
                start: start_pos,
                end: end_pos,
            },
            selection_range: Range {
                start: start_pos,
                end: start_pos,
            },
            children: None,
        };
        utterance_children.push(sym);
    }

    // -----------------------------------------------------------------------
    // Build gem block symbols from @Bg/@Eg pairs.
    // -----------------------------------------------------------------------
    utterance_children.extend(
        GemNavigation::from_analysis(analysis)
            .into_iter()
            .map(GemNavigation::into_symbol),
    );
    // Sort all children by start line for consistent ordering.
    utterance_children.sort_by_key(|s| s.range.start.line);

    // -----------------------------------------------------------------------
    // Build the "Headers" namespace symbol.
    // -----------------------------------------------------------------------
    let headers_end_pos = first_utterance_offset
        .map(|off| index.offset_to_position(off.saturating_sub(1)))
        .unwrap_or(file_end);

    #[allow(deprecated)]
    let headers_sym = DocumentSymbol {
        name: "Headers".to_string(),
        detail: None,
        kind: SymbolKind::NAMESPACE,
        tags: None,
        deprecated: None,
        range: Range {
            start: file_start,
            end: headers_end_pos,
        },
        selection_range: Range {
            start: file_start,
            end: file_start,
        },
        children: None,
    };

    // -----------------------------------------------------------------------
    // Build the "Utterances" namespace symbol wrapping all utterance children.
    // -----------------------------------------------------------------------
    let utterances_start = utterance_children
        .iter()
        .map(|symbol| symbol.range.start)
        .min()
        .unwrap_or(file_end);

    let utterance_count = chat_file.utterances().count();
    #[allow(deprecated)]
    let utterances_sym = DocumentSymbol {
        name: "Utterances".to_string(),
        detail: Some(format!(
            "{utterance_count} utterance{}",
            if utterance_count == 1 { "" } else { "s" }
        )),
        kind: SymbolKind::NAMESPACE,
        tags: None,
        deprecated: None,
        range: Range {
            start: utterances_start,
            end: file_end,
        },
        selection_range: Range {
            start: utterances_start,
            end: utterances_start,
        },
        children: Some(utterance_children),
    };

    // -----------------------------------------------------------------------
    // Root "Chat Transcript" module symbol.
    // -----------------------------------------------------------------------
    #[allow(deprecated)]
    let root_sym = DocumentSymbol {
        name: "Chat Transcript".to_string(),
        detail: None,
        kind: SymbolKind::MODULE,
        tags: None,
        deprecated: None,
        range: file_range,
        selection_range: Range {
            start: file_start,
            end: file_start,
        },
        children: Some(vec![headers_sym, utterances_sym]),
    };

    Some(DocumentSymbolResponse::Nested(vec![root_sym]))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns the byte offset of the end of an utterance block (main tier +
/// all dependent tiers). Falls back to the main tier end if no dependent
/// tiers are present.
fn utterance_block_end(utterance: &talkbank_model::model::Utterance, main_span: Span) -> usize {
    utterance
        .dependent_tiers
        .iter()
        .filter_map(|t| t.tier.location().span())
        .map(|span| span.end())
        .max()
        .unwrap_or(main_span.end())
}

/// Returns the first `max_chars` characters of the line that starts at
/// `byte_offset` in `document`, stripping any leading tab characters.
fn first_line_at_offset(document: &str, byte_offset: usize, max_chars: usize) -> String {
    let text = &document[byte_offset.min(document.len())..];
    let line = crate::backend::utils::protocol_lines(text)
        .map(|line| line.text())
        .next()
        .unwrap_or("");
    let trimmed = line.trim_start_matches('\t');
    trimmed.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::document_analysis;
    const BASIC: &str = include_str!("../../../../../corpus/reference/core/basic-conversation.cha");
    const GEMS: &str =
        include_str!("../../../../../corpus/reference/edge-cases/multiline-paired-gems.cha");

    #[test]
    fn returns_nested_symbol_tree() {
        let input = BASIC;
        let analysis = document_analysis(&Url::parse("file:///reference.cha").unwrap(), input);
        let response = document_symbol(&analysis);
        assert!(response.is_some());

        if let Some(DocumentSymbolResponse::Nested(symbols)) = response {
            assert_eq!(symbols.len(), 1); // Root "Chat Transcript"
            assert_eq!(symbols[0].name, "Chat Transcript");
            assert_eq!(symbols[0].kind, SymbolKind::MODULE);

            let children = symbols[0].children.as_ref().unwrap();
            assert_eq!(children.len(), 2); // Headers + Utterances
            assert_eq!(children[0].name, "Headers");
            assert_eq!(children[1].name, "Utterances");
        } else {
            panic!("Expected nested response");
        }
    }

    #[test]
    fn utterances_contain_speaker_labels() {
        let input = BASIC;
        let analysis = document_analysis(&Url::parse("file:///reference.cha").unwrap(), input);
        let response = document_symbol(&analysis);

        if let Some(DocumentSymbolResponse::Nested(symbols)) = response {
            let utterances = &symbols[0].children.as_ref().unwrap()[1];
            let utt_children = utterances.children.as_ref().unwrap();
            assert_eq!(utt_children.len(), 4);
            assert!(utt_children[0].name.contains("CHI"));
            assert!(utt_children[1].name.contains("MOT"));
        } else {
            panic!("Expected nested response");
        }
    }

    #[test]
    fn gem_blocks_appear_as_event_symbols() {
        for (source, expected) in [
            (GEMS.to_owned(), Some("Gem: morning café play")),
            (
                GEMS.replace("@Bg:\tmorning\n\tcafé\n\tplay", "@Bg")
                    .replace("@Eg:\tmorning café\n\tplay", "@Eg"),
                Some("Gem"),
            ),
            (
                GEMS.replace("@Eg:\tmorning café\n\tplay", "@Eg:\tOther"),
                None,
            ),
        ] {
            for ending in ["\n", "\r\n", "\r"] {
                let input = source.replace('\n', ending);
                let analysis =
                    document_analysis(&Url::parse("file:///reference.cha").unwrap(), &input);
                let Some(DocumentSymbolResponse::Nested(symbols)) = document_symbol(&analysis)
                else {
                    panic!("expected nested response");
                };
                let utterances = &symbols[0].children.as_ref().unwrap()[1];
                assert_eq!(utterances.detail.as_deref(), Some("1 utterance"));
                let children = utterances.children.as_ref().unwrap();
                let gems: Vec<_> = children
                    .iter()
                    .filter(|symbol| symbol.kind == SymbolKind::EVENT)
                    .collect();
                assert_eq!(gems.len(), usize::from(expected.is_some()), "{input}");
                if let Some(name) = expected {
                    let gem = gems[0];
                    assert_eq!(gem.name, name);
                    assert_eq!(gem.range.start, Position::new(6, 0));
                    assert!(utterances.range.start <= gem.range.start);
                    assert!(utterances.range.end >= gem.range.end);
                }
            }
        }
    }

    #[test]
    fn first_line_at_offset_truncates() {
        let text = "\tthis is a very long line that goes on and on";
        let result = first_line_at_offset(text, 0, 10);
        assert_eq!(result.len(), 10);
        assert!(!result.starts_with('\t')); // Tab stripped
    }
}
