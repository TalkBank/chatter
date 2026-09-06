//! Unsupported-line recovery retains source locations and following utterances.

use talkbank_model::model::WriteChat;
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome, Severity, Span};

fn check(parser: &impl ChatParser, source: &str, offset: usize) -> (String, Vec<Span>) {
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(file) = parser.parse_chat_file(source, offset, &errors) else {
        panic!("unsupported lines must retain the surrounding document");
    };
    let diagnostics = errors.into_vec();
    for error in &diagnostics {
        assert_eq!(error.code, ErrorCode::UnexpectedLineType);
        assert_eq!(error.severity, Severity::Error);
        let span = error.location.span;
        assert!(span.start as usize >= offset);
        let text = &source[span.start as usize - offset..span.end as usize - offset];
        assert!(text.ends_with('\n'));
        assert!(!text.starts_with(['@', '*', '%', '\t']));
        let context = error.context.as_ref().unwrap();
        assert_eq!(
            &context.source_text[context.span.start as usize..context.span.end as usize],
            text,
        );
    }
    (
        file.to_chat_string(),
        diagnostics.into_iter().map(|e| e.location.span).collect(),
    )
}

#[test]
fn unsupported_line_reports_the_same_rule_and_source_in_both_backends() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E326.md")
        .unwrap();
    let canonical = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = talkbank_parser_re2c::Re2cParser::new();
    for example in spec.examples() {
        let source = example.chat.as_str();
        for source in [source.to_owned(), source.replace('\n', "\r\n")] {
            for offset in [0, 200] {
                assert_eq!(
                    check(&canonical, &source, offset),
                    check(&re2c, &source, offset)
                );
            }
        }
    }
}
