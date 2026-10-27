//! Authored header contracts through both public file and fragment parsers.
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
                        errors[0].location.span().expect("located diagnostic"),
                        Span::at(offset + input.len()),
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
                Line::Header { location: span, .. } => Some(*span),
                Line::Utterance(_) => None,
            })
            .collect();
        let expected: Vec<_> = input
            .split_inclusive('\n')
            .scan(offset, |start, text| {
                let span = Span::new(*start, *start + text.len());
                *start += text.len();
                Some((text, span))
            })
            .filter_map(|(text, span)| {
                text.starts_with('@')
                    .then_some(talkbank_model::Location::Located(span))
            })
            .collect();
        assert_eq!(actual, expected);
        assert!(errors.is_empty());
    }
}

/// New authored optional-field controls must mean the same thing through both APIs.
#[test]
fn structured_id_specs_preserve_legal_semantics_across_backends() {
    use talkbank_model::model::{ChatFile, SemanticEq};
    fn parse_clean(parser: &impl ChatParser, input: &str, origin: usize) -> ChatFile {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(input, origin, &errors)
            .into_option()
            .unwrap();
        assert!(
            errors.is_empty(),
            "{}: {:?}",
            parser.parser_name(),
            errors.into_vec()
        );
        file
    }
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = crate::Re2cParser::new();
    for input in [
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E514_2.cha"),
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E514_3.cha"),
    ] {
        for origin in [0, 25] {
            let left = parse_clean(&tree, input, origin);
            let right = parse_clean(&re2c, input, origin);
            assert!(left.semantic_eq(&right), "supported ID semantics agree");
            for file in [left, right] {
                let errors = ErrorCollector::new();
                file.validate_into(&errors, TranscriptName::Anonymous)
                    .unwrap();
                assert!(errors.is_empty());
            }
        }
    }
}

/// Detection needs an error, not warning-only output or an internal tool failure.
#[test]
fn structured_id_specs_require_error_level_invalidity_from_each_backend() {
    fn check(parser: &impl ChatParser, input: &str, origin: usize) {
        let errors = ErrorCollector::new();
        if let Some(file) = parser.parse_chat_file(input, origin, &errors).into_option() {
            assert!(
                file.validate_into(&errors, TranscriptName::Anonymous)
                    .is_err(),
                "{}: invalid ID cannot gain validity",
                parser.parser_name()
            );
        }
        let completed = talkbank_model::CompletedDiagnostics::admit(errors.into_vec()).unwrap();
        assert!(
            completed
                .diagnostics()
                .iter()
                .any(|error| error.severity == talkbank_model::Severity::Error),
            "{}: warnings or silence do not satisfy detection",
            parser.parser_name()
        );
    }
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = crate::Re2cParser::new();
    for input in [
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E511_3.cha"),
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E515_3.cha"),
    ] {
        for origin in [0, 25] {
            check(&tree, input, origin);
            check(&re2c, input, origin);
        }
    }
}

#[test]
fn metadata_headers_legal_specs_agree_through_files_and_fragments() {
    use talkbank_model::model::{Header, Line, SemanticEq};
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = crate::Re2cParser::new();
    for source in [
        include_str!(
            "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_1.cha"
        ),
        include_str!(
            "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_2.cha"
        ),
    ] {
        for newline in ["\n", "\r\n"] {
            let source = source.replace('\n', newline);
            for origin in [0, 25] {
                let left_errors = ErrorCollector::new();
                let left = ChatParser::parse_chat_file(&tree, &source, origin, &left_errors)
                    .into_option()
                    .unwrap();
                let right_errors = ErrorCollector::new();
                let right = re2c
                    .parse_chat_file(&source, origin, &right_errors)
                    .into_option()
                    .unwrap();
                assert!(
                    left.semantic_eq(&right),
                    "supported metadata semantics agree ({newline:?}, origin {origin}):\nleft: {:?}\nright: {:?}",
                    left.headers().collect::<Vec<_>>(),
                    right.headers().collect::<Vec<_>>()
                );
                let left = left
                    .validate_into(&left_errors, TranscriptName::Anonymous)
                    .unwrap();
                right
                    .validate_into(&right_errors, TranscriptName::Anonymous)
                    .unwrap();
                assert!(left_errors.is_empty(), "{:?}", left_errors.to_vec());
                assert!(right_errors.is_empty(), "{:?}", right_errors.to_vec());
                let mut compared = 0;
                for line in &left.document().lines {
                    let Line::Header {
                        header,
                        location: span,
                        ..
                    } = line
                    else {
                        continue;
                    };
                    if !matches!(
                        header.as_ref(),
                        Header::Languages { .. } | Header::Types(_) | Header::Situation { .. }
                    ) {
                        continue;
                    }
                    let header_span = span.span().expect("located");
                    let input = &source[header_span.start() - origin..header_span.end() - origin];
                    let left_errors = ErrorCollector::new();
                    let left_fragment =
                        ChatParser::parse_header(&tree, input, origin, &left_errors)
                            .into_option()
                            .unwrap();
                    let right_errors = ErrorCollector::new();
                    let right_fragment = re2c
                        .parse_header(input, origin, &right_errors)
                        .into_option()
                        .unwrap();
                    assert!(left_fragment.semantic_eq(&right_fragment));
                    assert!(header.as_ref().semantic_eq(&left_fragment));
                    assert!(left_errors.is_empty());
                    assert!(right_errors.is_empty());
                    compared += 1;
                }
                assert_eq!(compared, 3);
            }
        }
    }
}

#[test]
fn metadata_headers_invalid_specs_require_error_level_file_detection() {
    fn check(parser: &impl ChatParser, input: &str, origin: usize) {
        let errors = ErrorCollector::new();
        if let Some(file) = parser.parse_chat_file(input, origin, &errors).into_option() {
            // Preserve parse diagnostics alongside model validation. A
            // salvaged model does not certify the original source bytes.
            file.validate(&errors, TranscriptName::Anonymous);
        }
        let completed = talkbank_model::CompletedDiagnostics::admit(errors.into_vec()).unwrap();
        assert!(
            completed
                .diagnostics()
                .iter()
                .any(|error| error.severity == talkbank_model::Severity::Error),
            "{}: warnings or silence cannot satisfy detection",
            parser.parser_name()
        );
    }
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = crate::Re2cParser::new();
    for input in [
        include_str!(
            "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_3.cha"
        ),
        include_str!(
            "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_4.cha"
        ),
        include_str!(
            "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_5.cha"
        ),
        include_str!(
            "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_6.cha"
        ),
        include_str!(
            "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_13.cha"
        ),
        include_str!(
            "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_14.cha"
        ),
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E507_2.cha"),
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E519_1.cha"),
    ] {
        for newline in ["\n", "\r\n"] {
            let input = input.replace('\n', newline);
            for origin in [0, 25] {
                check(&tree, &input, origin);
                check(&re2c, &input, origin);
            }
        }
    }
}

#[test]
fn metadata_headers_types_mutations_detect_faults_through_isolated_headers() {
    use talkbank_parser::generated_traversal::{AsRawNode, SourceSlotView, TypesHeaderNode};
    fn check(parser: &impl ChatParser, input: &str, origin: usize) {
        let errors = ErrorCollector::new();
        let _outcome = parser.parse_header(input, origin, &errors);
        let completed = talkbank_model::CompletedDiagnostics::admit(errors.into_vec()).unwrap();
        assert!(
            completed
                .diagnostics()
                .iter()
                .any(|error| error.severity == talkbank_model::Severity::Error)
        );
        for error in completed.diagnostics() {
            assert!(error.location.span().expect("located diagnostic").start() >= origin);
            assert!(
                error.location.span().expect("located diagnostic").end() <= origin + input.len()
            );
        }
    }
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = crate::Re2cParser::new();
    let control = include_str!(
        "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_1.cha"
    );
    for newline in ["\n", "\r\n"] {
        let source = control.replace('\n', newline);
        let parsed = tree.parse_source_incremental(&source, None).unwrap();
        let mut mutations = 0;
        for node in parsed.root().unwrap().descendants() {
            let node = node.unwrap();
            let Some(types) = node.typed::<TypesHeaderNode>() else {
                continue;
            };
            let children = types.extract();
            let SourceSlotView::Present(design) = children.field_child_2().slot().view() else {
                panic!("design")
            };
            let SourceSlotView::Present(activity) = children.field_child_6().slot().view() else {
                panic!("activity")
            };
            let SourceSlotView::Present(group) = children.field_child_10().slot().view() else {
                panic!("group")
            };
            let design = design.read().unwrap();
            let activity = activity.read().unwrap();
            let group = group.read().unwrap();
            let start = types.raw_node().start_byte();
            let end = types.raw_node().end_byte();
            for (range, replacement, authored) in [
                (
                    design.raw_node().end_byte()..group.raw_node().end_byte(),
                    "",
                    include_str!(
                        "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_3.cha"
                    ),
                ),
                (
                    activity.raw_node().byte_range(),
                    "toy play",
                    include_str!(
                        "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_4.cha"
                    ),
                ),
                (
                    group.raw_node().byte_range(),
                    "T-D",
                    include_str!(
                        "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_5.cha"
                    ),
                ),
                (
                    group.raw_node().byte_range(),
                    "TDé",
                    include_str!(
                        "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_6.cha"
                    ),
                ),
            ] {
                let mut whole = source.clone();
                whole.replace_range(range.clone(), replacement);
                assert_eq!(
                    whole,
                    authored.replace('\n', newline),
                    "only the admitted field range changes"
                );
                let mut fragment = source[start..end].to_owned();
                fragment.replace_range(range.start - start..range.end - start, replacement);
                for origin in [0, 25] {
                    check(&tree, &fragment, origin);
                    check(&re2c, &fragment, origin);
                }
                mutations += 1;
            }
        }
        assert_eq!(mutations, 4);
    }
}

#[test]
fn rejected_headers_preserve_original_source_through_model_and_json() {
    use talkbank_model::model::{ChatFile, Header, Line, WriteChat};
    const TYPES: &str = include_str!(
        "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_3.cha"
    );
    const MEDIA: &str =
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E509_4.cha");
    const L1: &str =
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E519_2.cha");
    const PARTIAL_MEDIA: &str =
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E342_7.cha");
    const PARTIAL_TYPES: &str = include_str!(
        "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_6.cha"
    );
    let empty_l1 = L1.replace("@L1 of CHI:\tqzz", "@L1 of CHI:\t");
    for (original, prefix) in [
        (TYPES, "@Types:"),
        (MEDIA, "@Media:"),
        (PARTIAL_MEDIA, "@Media:"),
        (PARTIAL_TYPES, "@Types:"),
        (&empty_l1, "@L1 of"),
    ] {
        for newline in ["\n", "\r\n", "\r"] {
            let input = original.replace('\n', newline);
            let expected = original
                .lines()
                .find(|line| line.starts_with(prefix))
                .unwrap();
            for offset in [0, 200] {
                let fragment_errors = ErrorCollector::new();
                let fragment = crate::Re2cParser::new()
                    .parse_header(expected, offset, &fragment_errors)
                    .into_option()
                    .expect("retained header fragment");
                assert_eq!(fragment.to_chat_string(), expected);
                let errors = ErrorCollector::new();
                let file = crate::Re2cParser::new()
                    .parse_chat_file(&input, offset, &errors)
                    .into_option()
                    .expect("retained document");
                let restored: ChatFile =
                    serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
                for model in [&file, &restored] {
                    let recovered = model
                        .lines
                        .iter()
                        .find_map(|line| match line {
                            Line::Header { header, .. }
                                if matches!(header.as_ref(), Header::Unknown { .. }) =>
                            {
                                Some(header)
                            }
                            _ => None,
                        })
                        .expect("explicit rejected header");
                    assert_eq!(recovered.to_chat_string(), expected, "{prefix}");
                    let validation = ErrorCollector::new();
                    model.validate(&validation, TranscriptName::Anonymous);
                    assert!(
                        validation
                            .into_vec()
                            .iter()
                            .any(|error| error.code == ErrorCode::UnknownHeader
                                && error.severity == talkbank_model::Severity::Error)
                    );
                }
            }
        }
    }
}

/// Unknown names cannot inherit a known header's payload or parser policy.
#[test]
fn header_name_prefixes_preserve_unknown_source_and_refuse_validation() {
    fn check(parser: &impl ChatParser, original: &str, name: &str) {
        for newline in ["\n", "\r\n", "\r"] {
            let input = original.replace('\n', newline);
            for origin in [0, 200] {
                let errors = ErrorCollector::new();
                let file = parser
                    .parse_chat_file(&input, origin, &errors)
                    .into_option()
                    .unwrap();
                let expected = original
                    .lines()
                    .find(|line| line.starts_with(name))
                    .unwrap();
                let unknown: Vec<_> = file
                    .lines
                    .iter()
                    .filter_map(|line| match line {
                        talkbank_model::model::Line::Header { header, .. } => match header.as_ref()
                        {
                            talkbank_model::Header::Unknown { text, .. } => {
                                Some(text.as_str().trim_end_matches(['\r', '\n']))
                            }
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect();
                assert_eq!(unknown, [expected], "{}: {name}", parser.parser_name());
                assert_eq!(file.utterances().count(), 1);
                file.validate(&errors, TranscriptName::Anonymous);
                assert!(
                    errors
                        .into_vec()
                        .iter()
                        .any(|error| error.code == ErrorCode::UnknownHeader),
                    "{}: {name} must remain unsupported",
                    parser.parser_name()
                );
            }
        }
    }
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = crate::Re2cParser::new();
    for (source, name) in [
        (
            include_str!(
                "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E525_5.cha"
            ),
            "@Commentary:",
        ),
        (
            include_str!(
                "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E525_6.cha"
            ),
            "@DateExtra:",
        ),
        (
            include_str!(
                "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E525_7.cha"
            ),
            "@OptionsExtra:",
        ),
        (
            include_str!(
                "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E525_8.cha"
            ),
            "@BgExtra:",
        ),
    ] {
        check(&tree, source, name);
        check(&native, source, name);
    }
}

#[test]
fn types_header_requires_one_complete_triple() {
    fn check(parser: &impl ChatParser, source: &str, fragment: &str, origin: usize) {
        #[derive(Debug, Clone, Copy)]
        enum Input<'a> {
            File(&'a str),
            Fragment(&'a str),
        }
        for input in [Input::File(source), Input::Fragment(fragment)] {
            let errors = ErrorCollector::new();
            match input {
                Input::Fragment(text) => {
                    let _ = parser.parse_header(text, origin, &errors);
                }
                Input::File(text) => {
                    let _ = parser.parse_chat_file(text, origin, &errors);
                }
            }
            let completed = talkbank_model::CompletedDiagnostics::admit(errors.into_vec()).unwrap();
            assert!(
                completed
                    .diagnostics()
                    .iter()
                    .any(|error| { error.severity == talkbank_model::Severity::Error }),
                "extra Types triple must be rejected ({input:?}, origin={origin})"
            );
        }
    }
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = crate::Re2cParser::new();
    let authored = include_str!(
        "../../talkbank-parser-tests/tests/error_corpus/validation_errors/E316_metadata_headers_7.cha"
    );
    for newline in ["\n", "\r\n"] {
        let source = authored.replace('\n', newline);
        let start = source.find("@Types:").unwrap();
        let end = source.find("@Situation:").unwrap();
        let fragment = &source[start..end];
        for origin in [0, 37] {
            check(&tree, &source, fragment, origin);
            check(&native, &source, fragment, origin);
            let errors = ErrorCollector::new();
            let header = native
                .parse_header(fragment, origin, &errors)
                .into_option()
                .unwrap();
            let talkbank_model::Header::Unknown { text, .. } = header else {
                panic!("a rejected Types body must not become a valid triple")
            };
            assert_eq!(text.as_str(), fragment.trim_end_matches(['\r', '\n']));
        }
    }
}
