//! Source-backed recovery stays invalid through the public model and JSON boundary.
//!
//! Registered once under the canonical validation family. Each contract exercises
//! both peer parsers; native-only transform recovery remains separately selected.

use talkbank_model::{ChatParser, ErrorCollector, WriteChat};

#[test]
fn bare_angle_groups_retain_model_invalidity_after_json() {
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum RequiredAnnotation {
        Present,
        Missing,
    }
    fn check(parser: &impl ChatParser) {
        use RequiredAnnotation::{Missing, Present};
        use talkbank_model::ErrorCode;
        let specs = talkbank_parser_tests::error_specs::load(
            talkbank_parser_tests::repo_paths::workspace_root(),
        )
        .unwrap();
        for (filename, index, expected) in [
            ("E342.md", 0, Missing),
            ("E342_nested_group.md", 0, Present),
            ("E342_nested_group.md", 2, Present),
            ("E342_nested_group.md", 3, Missing),
            ("E342_nested_group.md", 4, Present),
            ("E342_nested_group.md", 5, Missing),
            ("E342_nested_group.md", 6, Present),
            ("E342_nested_group.md", 7, Missing),
        ] {
            let spec = specs.iter().find(|spec| spec.filename == filename).unwrap();
            let findings = ErrorCollector::new();
            let file = parser
                .parse_chat_file(spec.examples()[index].chat.as_str(), 31, &findings)
                .into_option()
                .unwrap();
            assert_eq!(findings.has_errors(), expected == Missing);
            let wire = serde_json::to_string(&file).unwrap();
            let decoded: talkbank_model::ChatFile = serde_json::from_str(&wire).unwrap();
            for retained in [&file, &decoded] {
                let errors = ErrorCollector::new();
                retained.validate(&errors, talkbank_model::TranscriptName::Anonymous);
                let diagnostics = errors.into_vec();
                assert_eq!(
                    diagnostics
                        .iter()
                        .any(|error| error.code == ErrorCode::MissingRequiredElement),
                    expected == Missing,
                    "{} {filename}/{}: {diagnostics:?}",
                    parser.parser_name(),
                    index + 1
                );
                if expected == Present {
                    assert!(diagnostics.is_empty());
                }
            }
        }
    }
    check(&talkbank_parser_re2c::Re2cParser::new());
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
}

#[test]
fn recognized_empty_postcodes_retain_source_and_invalidity() {
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum ExpectedContent {
        Present,
        Missing,
    }
    fn check(parser: &impl ChatParser) {
        let specs = talkbank_parser_tests::error_specs::load(
            talkbank_parser_tests::repo_paths::workspace_root(),
        )
        .unwrap();
        let spec = specs
            .iter()
            .find(|spec| spec.filename == "E363.md")
            .unwrap();
        use ExpectedContent::{Missing, Present};
        for (index, expected) in [(0, Missing), (2, Present), (3, Present)] {
            let source = spec.examples()[index].chat.as_str();
            let findings = ErrorCollector::new();
            let file = parser
                .parse_chat_file(source, 43, &findings)
                .into_option()
                .unwrap();
            assert_eq!(findings.has_errors(), expected == Missing);
            let encoded = serde_json::to_string(&file).unwrap();
            let restored: talkbank_model::ChatFile = serde_json::from_str(&encoded).unwrap();
            for retained in [&file, &restored] {
                let postcodes = &retained.utterances().next().unwrap().main.content.postcodes;
                assert_eq!(
                    postcodes.len(),
                    1,
                    "{} dropped the observed postcode",
                    parser.parser_name()
                );
                if expected == Missing {
                    assert_eq!(postcodes[0].to_chat_string(), "[+  ]");
                }
                let validation = ErrorCollector::new();
                retained.validate(&validation, talkbank_model::TranscriptName::Anonymous);
                let errors = validation.into_vec();
                assert_eq!(
                    errors
                        .iter()
                        .any(|error| error.code == talkbank_model::ErrorCode::InvalidPostcode),
                    expected == Missing,
                    "{} postcode invalidity was lost: {errors:?}",
                    parser.parser_name()
                );
                if expected == Present {
                    assert!(errors.is_empty());
                }
                let result = talkbank_transform::redact::sanitize(
                    retained.clone(),
                    &talkbank_transform::redact::SanitizationPolicy::strict(),
                );
                if expected == Missing {
                    assert!(matches!(
                        result,
                        Err(talkbank_transform::redact::RedactError::UnparsedPostcode)
                    ));
                } else {
                    assert!(result.is_ok());
                }
            }
        }
    }
    check(&talkbank_parser_re2c::Re2cParser::new());
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
}

#[test]
fn freecode_recovery_preserves_source_and_invalidity_at_every_depth() {
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum ExpectedContent {
        Present,
        Missing,
    }
    fn check(parser: &impl ChatParser) {
        use ExpectedContent::{Missing, Present};
        let specs = talkbank_parser_tests::error_specs::load(
            talkbank_parser_tests::repo_paths::workspace_root(),
        )
        .unwrap();
        let spec = specs
            .iter()
            .find(|spec| spec.filename == "E316_freecode.md")
            .unwrap();
        for (index, expected) in [
            (0, Missing),
            (1, Present),
            (2, Missing),
            (3, Present),
            (4, Missing),
            (5, Present),
            (6, Missing),
            (7, Present),
            (8, Missing),
            (9, Present),
        ] {
            let source = spec.examples()[index].chat.as_str();
            let findings = ErrorCollector::new();
            let file = parser
                .parse_chat_file(source, 43, &findings)
                .into_option()
                .unwrap();
            assert_eq!(
                findings.has_errors(),
                expected == Missing,
                "{} example {}: {:?}",
                parser.parser_name(),
                index + 1,
                findings.to_vec()
            );
            let encoded = serde_json::to_string(&file).unwrap();
            let restored: talkbank_model::ChatFile = serde_json::from_str(&encoded).unwrap();
            for retained in [&file, &restored] {
                let mut codes = Vec::new();
                for utterance in retained.utterances() {
                    talkbank_model::alignment::helpers::walk_content(
                        &utterance.main.content.content,
                        None,
                        &mut |item| {
                            if let talkbank_model::alignment::helpers::ContentItem::Freecode(code) =
                                item
                            {
                                codes.push(code.to_chat_string());
                            }
                        },
                    );
                }
                let spelling = match expected {
                    Missing => "[^  ]",
                    Present => "[^ note]",
                };
                assert_eq!(
                    codes,
                    [spelling],
                    "{} example {} lost freecode source",
                    parser.parser_name(),
                    index + 1
                );
                let validation = ErrorCollector::new();
                retained.validate(&validation, talkbank_model::TranscriptName::Anonymous);
                let errors = validation.into_vec();
                assert_eq!(
                    errors
                        .iter()
                        .any(|e| e.code == talkbank_model::ErrorCode::UnparsableContent),
                    expected == Missing,
                    "{} example {}: {errors:?}",
                    parser.parser_name(),
                    index + 1
                );
                assert!(
                    !errors
                        .iter()
                        .any(|e| e.code == talkbank_model::ErrorCode::EmptyUtterance)
                );
                if expected == Present {
                    assert!(errors.is_empty());
                }
                let result = talkbank_transform::redact::sanitize(
                    retained.clone(),
                    &talkbank_transform::redact::SanitizationPolicy::strict(),
                );
                match expected {
                    Missing => assert!(matches!(
                        result,
                        Err(talkbank_transform::redact::RedactError::UnparsedFreecode)
                    )),
                    Present => assert!(result.is_ok()),
                }
            }
        }
    }
    check(&talkbank_parser_re2c::Re2cParser::new());
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
}
