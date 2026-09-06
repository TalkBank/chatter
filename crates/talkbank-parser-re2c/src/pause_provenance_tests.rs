//! Cross-layer behavior: full lexer extents must survive AST lowering and rebasing.
#![allow(clippy::unwrap_used, clippy::panic)]

use talkbank_model::alignment::helpers::{ContentItem, walk_content};
use talkbank_model::{ChatParser, ErrorCollector, Span};

#[test]
fn pause_extents_survive_lowering_and_fragment_rebasing() {
    let parser = crate::Re2cParser::new();
    for input in [
        "*CHI:\tcafé (.) (..) (...) (1:02.5) .\n",
        "*CHI:\t<café (.) (..) (...) (1:02.5)> [!] .\r\n",
    ] {
        for offset in [0, 200] {
            let errors = ErrorCollector::new();
            let main = parser
                .parse_main_tier(input, offset, &errors)
                .into_option()
                .unwrap();
            let mut spans = Vec::new();
            walk_content(main.content.content.as_slice(), None, &mut |item| {
                if let ContentItem::Pause(pause) = item {
                    spans.push(pause.span);
                }
            });
            let expected: Vec<_> = ["(.)", "(..)", "(...)", "(1:02.5)"]
                .into_iter()
                .map(|text| {
                    let start = input.find(text).unwrap() + offset;
                    Span::from_usize(start, start + text.len())
                })
                .collect();
            assert_eq!(spans, expected, "{input:?} at {offset}");
            assert!(errors.is_empty());
        }
    }
}

/// Parse-plus-validation must report each missing separator once at both depths.
#[test]
fn pause_spacing_is_shared_located_and_recursive() {
    use talkbank_model::{ErrorCode, model::TranscriptName};
    fn spans(parser: &impl ChatParser, input: &str, code: ErrorCode) -> Vec<Span> {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(input, 0, &errors)
            .into_option()
            .unwrap();
        file.validate(&errors, TranscriptName::Anonymous);
        errors
            .into_vec()
            .into_iter()
            .filter(|e| e.code == code)
            .map(|e| e.location.span)
            .collect()
    }
    let canonical = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = crate::Re2cParser::new();
    for pause in ["(.)", "(..)", "(...)", "(1:02.5)"] {
        for (body, code, expected_count) in [
            (
                format!("café{pause} there ."),
                ErrorCode::PauseGluedToWord,
                1,
            ),
            (
                format!("café {pause}there ."),
                ErrorCode::SeparatorGluedToFollowingContent,
                1,
            ),
            (
                format!("<café {pause}there> [!] ."),
                ErrorCode::SeparatorGluedToFollowingContent,
                1,
            ),
            (
                format!("<café {pause} there> [!] ."),
                ErrorCode::SeparatorGluedToFollowingContent,
                0,
            ),
        ] {
            let input = format!(
                "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\t{body}\n@End\n"
            );
            let expected = spans(&canonical, &input, code);
            let actual = spans(&re2c, &input, code);
            assert_eq!(actual.len(), expected_count, "{body:?}: {actual:?}");
            assert_eq!(actual, expected, "{body:?}");
            assert!(actual.iter().all(|span| !span.is_dummy()));
        }
    }
}
