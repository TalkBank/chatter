//! Authored missing-encoding example through both public file parsers.
#![allow(clippy::unwrap_used)]
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, Span, model::TranscriptName};

#[test]
fn missing_encoding_keeps_the_document_and_locates_the_single_refusal() {
    const MISSING: &str =
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E503_1.cha");
    const DECLARED: &str =
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E503_2.cha");
    fn check(parser: &impl ChatParser) {
        for (input, missing) in [(MISSING, true), (DECLARED, false)] {
            for offset in [0, 200, u32::MAX as usize - input.len()] {
                let errors = ErrorCollector::new();
                let file = parser
                    .parse_chat_file(input, offset, &errors)
                    .into_option()
                    .unwrap();
                assert_eq!(
                    file.utterances().count(),
                    1,
                    "{} must retain spoken content",
                    parser.parser_name()
                );
                file.validate(&errors, TranscriptName::Anonymous);
                let errors = errors.into_vec();
                if missing {
                    assert_eq!(errors.len(), 1, "{}: {errors:?}", parser.parser_name());
                    assert_eq!(errors[0].code, ErrorCode::MissingUTF8Header);
                    assert_eq!(
                        errors[0].location.span,
                        Span::at((offset + input.len()) as u32),
                        "{}",
                        parser.parser_name()
                    );
                } else {
                    assert!(errors.is_empty(), "{}: {errors:?}", parser.parser_name());
                }
            }
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&crate::Re2cParser::new());
}

#[test]
fn missing_end_has_no_spurious_syntax_error_with_or_without_final_newline() {
    fn check(parser: &impl ChatParser) {
        let source = include_str!(
            "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E502_1.cha"
        );
        for input in [source, source.trim_end_matches('\n')] {
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(input, 0, &errors)
                .into_option()
                .unwrap();
            file.validate(&errors, TranscriptName::Anonymous);
            let codes: Vec<_> = errors
                .into_vec()
                .into_iter()
                .map(|error| error.code)
                .collect();
            assert_eq!(
                codes,
                [ErrorCode::MissingEndHeader],
                "{}",
                parser.parser_name()
            );
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&crate::Re2cParser::new());
}

#[test]
fn header_lexer_extents_survive_file_lowering_and_rebasing() {
    use talkbank_model::model::Line;
    let original =
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E503_2.cha");
    for newline in ["\n", "\r\n"] {
        let input = original.replace('\n', newline);
        let offset = 200;
        let errors = ErrorCollector::new();
        let file = crate::Re2cParser::new()
            .parse_chat_file(&input, offset, &errors)
            .into_option()
            .unwrap();
        let actual: Vec<_> = file
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Header { span, .. } => Some(*span),
                Line::Utterance(_) => None,
            })
            .collect();
        let expected: Vec<_> = input
            .split_inclusive('\n')
            .scan(offset, |start, text| {
                let span = Span::from_usize(*start, *start + text.len());
                *start += text.len();
                Some((text, span))
            })
            .filter_map(|(text, span)| text.starts_with('@').then_some(span))
            .collect();
        assert_eq!(actual, expected);
        assert!(errors.is_empty());
    }
}
