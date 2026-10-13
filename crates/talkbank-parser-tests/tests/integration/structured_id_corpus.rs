//! Selected ID payload admission, recovery, policy and wire contracts over authored specs.

use talkbank_model::model::{ChatFile, Header, Line, SemanticEq, TranscriptName, WriteChat};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome};
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{AsRawNode, IdContentsNode, IdHeaderNode};
use talkbank_parser_tests::{repo_paths::workspace_root, test_error::strict_parse};

fn specimen(name: &str) -> String {
    std::fs::read_to_string(workspace_root().join(format!(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{name}.cha"
    )))
    .expect("authored ID specimen")
}

#[test]
fn normalized_id_repeats_retain_typed_fields_and_structural_whitespace() {
    use talkbank_parser::generated_traversal::{
        IdContentsChild2Choice, IdContentsChild6Choice, IdContentsChild8Choice,
        IdContentsChild10Choice, IdContentsChild12Choice, IdContentsChild16Choice,
        IdContentsChild18Choice, ReadableSlot,
    };
    let parser = TreeSitterParser::new().expect("parser");
    let source = specimen("E514_3");
    let errors = ErrorCollector::new();
    let (_, parsed) = parser.parse_chat_file_with_source(&source, &errors);
    assert!(errors.is_empty());
    let parsed = parsed.expect("source owner");
    let contents = parsed
        .root()
        .expect("root")
        .descendants()
        .map(|node| node.expect("source range"))
        .find_map(|node| node.typed::<IdContentsNode>())
        .expect("ID contents");
    let admitted = contents
        .extract()
        .admit_ranges()
        .expect("all selected ranges");
    let fields = admitted.children();
    let mut observed = Vec::new();
    macro_rules! retain {
        ($slots:expr, $choice:ident, $field:ident, $label:literal) => {
            for element in &$slots {
                match &element.slot {
                    ReadableSlot::Present($choice::Whitespaces(value)) => {
                        assert_eq!(value.source(), source);
                        observed.push(("whitespace", value.text()));
                    }
                    ReadableSlot::Present($choice::$field(value)) => {
                        assert_eq!(value.source(), source);
                        observed.push(($label, value.text()));
                    }
                    other => panic!("valid authored ID has no recovered field: {other:?}"),
                }
            }
        };
    }
    retain!(
        fields.child_2.slot,
        IdContentsChild2Choice,
        IdCorpus,
        "corpus"
    );
    retain!(fields.child_6.slot, IdContentsChild6Choice, IdAge, "age");
    retain!(fields.child_8.slot, IdContentsChild8Choice, IdSex, "sex");
    retain!(
        fields.child_10.slot,
        IdContentsChild10Choice,
        IdGroup,
        "group"
    );
    retain!(fields.child_12.slot, IdContentsChild12Choice, IdSes, "ses");
    retain!(
        fields.child_16.slot,
        IdContentsChild16Choice,
        IdEducation,
        "education"
    );
    retain!(
        fields.child_18.slot,
        IdContentsChild18Choice,
        IdCustomField,
        "custom"
    );
    insta::assert_debug_snapshot!(observed, @r#"
    [
        (
            "corpus",
            "corpus",
        ),
        (
            "age",
            "3;06.02",
        ),
        (
            "sex",
            "female",
        ),
        (
            "group",
            "TD",
        ),
        (
            "ses",
            "MC",
        ),
        (
            "education",
            "Études supérieures",
        ),
        (
            "whitespace",
            " ",
        ),
        (
            "custom",
            "café α",
        ),
        (
            "whitespace",
            " ",
        ),
    ]
    "#);
}

#[test]
fn structured_id_populated_fields_survive_range_admission_and_wire() {
    let parser = TreeSitterParser::new().expect("parser");
    for (name, education, custom) in [
        ("E514_2", "College", "extended"),
        ("E514_3", "Études supérieures", "café α"),
    ] {
        let source = specimen(name);
        let errors = ErrorCollector::new();
        let (file, parsed) = parser.parse_chat_file_with_source(&source, &errors);
        assert!(errors.is_empty(), "{name}: {:?}", errors.to_vec());
        let parsed = parsed.expect("source owner");
        let mut payloads = 0;
        for node in parsed.root().expect("root").descendants() {
            let node = node.expect("readable source");
            let Some(contents) = node.typed::<IdContentsNode>() else {
                continue;
            };
            let admitted = contents
                .extract()
                .admit_ranges()
                .expect("selected payload ranges");
            assert_eq!(admitted.parent().source(), source);
            assert!(
                admitted.checked_range_count() > 10,
                "includes structure, not just payload text"
            );
            payloads += 1;
        }
        assert_eq!(payloads, 1, "one actual ID payload");
        let ids: Vec<_> = file
            .headers()
            .filter_map(|header| match header {
                Header::ID(id) => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(ids.len(), 1);
        let id = ids[0];
        assert_eq!(
            id.language
                .iter()
                .map(|code| code.as_str())
                .collect::<Vec<_>>(),
            vec!["eng"]
        );
        assert_eq!(id.corpus.as_str(), "corpus");
        assert_eq!(id.speaker.as_str(), "CHI");
        assert_eq!(id.role.as_str(), "Target_Child");
        assert_eq!(id.age.as_ref().expect("age").as_str(), "3;06.02");
        assert_eq!(id.sex, Some(talkbank_model::model::Sex::Female));
        assert_eq!(id.group.as_ref().expect("group").as_str(), "TD");
        assert!(id.ses.is_some());
        assert_eq!(
            id.education.as_ref().expect("education").as_str(),
            education
        );
        assert_eq!(id.custom_field.as_ref().expect("custom").as_str(), custom);
        let wire: ChatFile = serde_json::from_str(&serde_json::to_string(&file).expect("wire"))
            .expect("wire admission");
        assert!(file.semantic_eq(&wire));
        let errors = ErrorCollector::new();
        let refusal = wire
            .validate_into(&errors, TranscriptName::Anonymous)
            .expect_err("wire values do not carry parse-health evidence");
        assert!(refusal.has_incomplete_parse());
        assert!(errors.is_empty(), "readable ID values remain legal");
        let errors = ErrorCollector::new();
        let valid = file
            .validate_into(&errors, TranscriptName::Anonymous)
            .expect("fully populated ID is valid");
        assert!(errors.is_empty());
        let canonical = valid.document().to_chat_string();
        let reparsed = strict_parse(parser.parse_chat_file(&canonical)).expect("canonical parse");
        assert!(valid.document().semantic_eq(&reparsed));
        assert_eq!(
            reparsed.to_chat_string(),
            canonical,
            "normalization is a fixed point"
        );
    }
}

#[test]
fn structured_id_readable_invalid_fields_remain_invalid_after_wire() {
    let parser = TreeSitterParser::new().expect("parser");
    for (name, code) in [
        ("E510_1", ErrorCode::EmptyIDLanguage),
        ("E510_3", ErrorCode::EmptyIDLanguage),
        ("E514_1", ErrorCode::EmptyIDCorpus),
        ("E517_1", ErrorCode::InvalidAgeFormat),
        ("E542_1", ErrorCode::UnsupportedSex),
        ("E546_1", ErrorCode::UnsupportedSesValue),
        ("E519_5", ErrorCode::InvalidLanguageCode),
    ] {
        let source = specimen(name);
        let file = strict_parse(parser.parse_chat_file(&source))
            .expect("invalidity is not syntax recovery");
        let wire: ChatFile = serde_json::from_str(&serde_json::to_string(&file).expect("wire"))
            .expect("wire admission is not validity");
        assert!(file.semantic_eq(&wire));
        assert!(file.headers().any(|header| matches!(header, Header::ID(_))));
        for model in [file, wire] {
            let errors = ErrorCollector::new();
            let refusal = model
                .validate_into(&errors, TranscriptName::Anonymous)
                .expect_err("no validity proof");
            assert!(
                !refusal.has_internal_failure(),
                "authored invalidity is not a producer failure"
            );
            assert!(
                errors.into_vec().iter().any(|error| error.code == code),
                "{name}: authored invalidity remains visible"
            );
        }
    }
}

#[test]
fn structured_id_required_field_deletions_preserve_invalid_fragment_payloads() {
    let parser = TreeSitterParser::new().expect("parser");
    let source = specimen("E514_2");
    let errors = ErrorCollector::new();
    let (file, parsed) = parser.parse_chat_file_with_source(&source, &errors);
    assert!(errors.is_empty());
    let parsed = parsed.expect("source owner");
    let contents = parsed
        .root()
        .expect("root")
        .descendants()
        .map(|node| node.expect("source range"))
        .find_map(|node| node.typed::<IdContentsNode>())
        .expect("ID payload");
    let children = contents.extract();
    use talkbank_parser::generated_traversal::SourceSlotView;
    let SourceSlotView::Present(speaker) = children.field_child_4().slot().view() else {
        panic!("seed speaker")
    };
    let SourceSlotView::Present(role) = children.field_child_14().slot().view() else {
        panic!("seed role")
    };
    let line_span = file
        .lines
        .iter()
        .find_map(|line| match line {
            Line::Header { header, span, .. } if matches!(header.as_ref(), Header::ID(_)) => {
                Some(*span)
            }
            _ => None,
        })
        .expect("ID line source range");
    let line_start = line_span.start as usize;
    let original = source
        .get(line_start..line_span.end as usize)
        .expect("original ID line");
    for (name, range, code) in [
        (
            "E511_3",
            speaker
                .read()
                .expect("speaker range")
                .raw_node()
                .byte_range(),
            ErrorCode::EmptyIDSpeaker,
        ),
        (
            "E515_3",
            role.read().expect("role range").raw_node().byte_range(),
            ErrorCode::EmptyIDRole,
        ),
    ] {
        let mut mutated = source.clone();
        mutated.replace_range(range.clone(), "");
        assert_eq!(
            mutated,
            specimen(name),
            "authored case is precisely one selected-field deletion"
        );
        let errors = ErrorCollector::new();
        let document = parser.parse_chat_file_streaming(&mutated, &errors);
        let expected = document
            .headers()
            .find_map(|header| match header {
                Header::ID(id) => Some(id),
                _ => None,
            })
            .expect("empty token is readable syntax, not structural omission");
        let errors = ErrorCollector::new();
        assert!(
            document
                .clone()
                .validate_into(&errors, TranscriptName::Anonymous)
                .is_err()
        );
        assert!(errors.into_vec().iter().any(|error| error.code == code));
        let mut fragment = original.to_owned();
        fragment.replace_range(range.start - line_start..range.end - line_start, "");
        for origin in [0, 25] {
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(actual) =
                ChatParser::parse_id_header(&parser, &fragment, origin, &errors)
            else {
                panic!("{name}: fragment parsing must preserve the invalid but readable payload");
            };
            assert!(
                errors.is_empty(),
                "{name}: fragment parsing is not semantic validation"
            );
            assert!(actual.semantic_eq(expected));
        }
    }
}

#[test]
fn structured_id_truncated_specs_do_not_fabricate_typed_headers() {
    let parser = TreeSitterParser::new().expect("parser");
    for name in ["E505_1", "E505_2", "E505_3"] {
        let source = specimen(name);
        let errors = ErrorCollector::new();
        let (file, parsed) = parser.parse_chat_file_with_source(&source, &errors);
        let parsed = parsed.expect("source remains available after recovery");
        assert!(parsed.root_node().has_error());
        assert!(
            !parsed
                .root()
                .expect("root")
                .descendants()
                .map(|node| node.expect("source range"))
                .any(|node| node.typed::<IdHeaderNode>().is_some()),
            "{name}: no typed ID carrier"
        );
        assert!(
            !file
                .headers()
                .any(|header| matches!(header, Header::ID(_) | Header::Unknown { .. }))
        );
        assert!(
            errors
                .into_vec()
                .iter()
                .any(|error| error.code == ErrorCode::UnparsableContent)
        );
        assert!(strict_parse(parser.parse_chat_file(&source)).is_err());
    }
}
