//! Replacement admission over both parsers and the unchecked JSON boundary.

#![allow(clippy::unwrap_used, clippy::panic)]

use talkbank_model::{
    ChatParser, ErrorCollector, ParticipantName, ParticipantRole, SpeakerCode, TranscriptName,
};
use talkbank_transform::speaker_id::{
    MappingAssignmentError, MappingSpec, SpeakerAssignment, SpeakerIdError, apply_mapping_chat,
};

enum RejectedField {
    Speaker,
    Role,
    Name,
}

fn mapping(code: &str, role: &str, name: Option<&str>) -> MappingSpec {
    MappingSpec::from([(
        SpeakerCode::new("CHI"),
        SpeakerAssignment::Rename {
            code: SpeakerCode::new(code),
            role: ParticipantRole::new(role),
            specific_role: name.map(ParticipantName::new),
        },
    )])
}

#[test]
fn speaker_mapping_admits_replacements_before_rendering() {
    fn check(parser: &impl ChatParser) {
        let source = include_str!("../../../../corpus/reference/core/headers-speaker-info.cha");
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .unwrap();
        file.validate(&errors, TranscriptName::Anonymous);
        assert!(!errors.has_errors(), "{:?}", errors.into_vec());
        let decoded = serde_json::from_value(serde_json::to_value(&file).unwrap()).unwrap();
        for document in [&file, &decoded] {
            let before = serde_json::to_value(document).unwrap();
            for (candidate, field) in [
                (mapping("", "Child", None), RejectedField::Speaker),
                (mapping("A B", "Child", None), RejectedField::Speaker),
                (mapping("A:B", "Child", None), RejectedField::Speaker),
                (mapping("A|B", "Child", None), RejectedField::Speaker),
                (mapping("CHILDREN", "Child", None), RejectedField::Speaker),
                (mapping("\n@End", "Child", None), RejectedField::Speaker),
                (mapping("INV", "", None), RejectedField::Role),
                (mapping("INV", "UnknownRole", None), RejectedField::Role),
                (mapping("INV", "Child|extra", None), RejectedField::Role),
                (mapping("INV", "Child\n@End", None), RejectedField::Role),
                (mapping("INV", "Child", Some("")), RejectedField::Name),
                (mapping("INV", "Child", Some(" \t")), RejectedField::Name),
                (
                    mapping("INV", "Child", Some("Alex, MOT Mother")),
                    RejectedField::Name,
                ),
                (
                    mapping("INV", "Child", Some("Alex\r@End")),
                    RejectedField::Name,
                ),
                (
                    mapping("INV", "Child", Some("Alex\n@End")),
                    RejectedField::Name,
                ),
            ] {
                let error = apply_mapping_chat(document, &candidate).unwrap_err();
                match (field, error) {
                    (
                        RejectedField::Speaker,
                        SpeakerIdError::InvalidAssignment(MappingAssignmentError::Speaker {
                            speaker,
                            ..
                        }),
                    )
                    | (
                        RejectedField::Role,
                        SpeakerIdError::InvalidAssignment(MappingAssignmentError::Role {
                            speaker,
                            ..
                        }),
                    )
                    | (
                        RejectedField::Name,
                        SpeakerIdError::InvalidAssignment(MappingAssignmentError::Name {
                            speaker,
                            ..
                        }),
                    ) => {
                        assert_eq!(speaker.as_str(), "CHI");
                    }
                    (_, error) => panic!("wrong refusal: {error:?}"),
                }
                assert_eq!(serde_json::to_value(document).unwrap(), before);
            }

            // Multiword and Unicode names are legal; replacement admission is
            // not an ASCII-only or single-word name policy.
            for name in [
                None,
                Some("First_Investigator"),
                Some("Alex Marie"),
                Some("Éloïse"),
            ] {
                let output =
                    apply_mapping_chat(document, &mapping("INV", "Investigator", name)).unwrap();
                let errors = ErrorCollector::new();
                let rewritten = parser
                    .parse_chat_file(&output, 0, &errors)
                    .into_option()
                    .unwrap();
                rewritten.validate(&errors, TranscriptName::Anonymous);
                assert!(!errors.has_errors(), "{output}: {:?}", errors.into_vec());
                let entry = rewritten
                    .participant_entries()
                    .unwrap()
                    .iter()
                    .find(|entry| entry.speaker_code.as_str() == "INV")
                    .unwrap();
                if let Some(name) = name {
                    assert_eq!(entry.name().map(ParticipantName::as_str), Some(name));
                }
                assert_eq!(serde_json::to_value(document).unwrap(), before);
            }
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&talkbank_parser_re2c::Re2cParser::new());
}

/// E549 forbids repeated declarations, including those introduced by a mapping.
#[test]
fn speaker_mapping_binds_unique_targets_to_the_input_declaration() {
    use talkbank_transform::speaker_id::parse_mapping_spec;
    fn check(parser: &impl ChatParser) {
        for source in [
            include_str!("../../../../corpus/reference/core/headers-speaker-info.cha"),
            include_str!("../error_corpus/validation_errors/E549_4.cha"),
        ] {
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(source, 0, &errors)
                .into_option()
                .unwrap();
            file.validate(&errors, TranscriptName::Anonymous);
            assert!(!errors.has_errors(), "{:?}", errors.into_vec());
            let decoded = serde_json::from_value(serde_json::to_value(&file).unwrap()).unwrap();
            for document in [&file, &decoded] {
                let before = serde_json::to_value(document).unwrap();
                for (text, target) in [
                    ("CHI=MOT:Child", "MOT"),
                    ("CHI=INV:Child,MOT=INV:Mother", "INV"),
                    ("MOT=CHI:Mother", "CHI"),
                ] {
                    let candidate = parse_mapping_spec(text).unwrap();
                    match apply_mapping_chat(document, &candidate).unwrap_err() {
                        SpeakerIdError::InvalidAssignment(
                            MappingAssignmentError::TargetCollision {
                                first,
                                second,
                                target: actual,
                            },
                        ) => {
                            assert_eq!(first.as_str(), "CHI");
                            assert_eq!(second.as_str(), "MOT");
                            assert_eq!(actual.as_str(), target);
                        }
                        other => panic!("wrong collision refusal for {text}: {other:?}"),
                    }
                    assert_eq!(serde_json::to_value(document).unwrap(), before);
                }
                // Judge the simultaneous result, not occupancy of old names.
                for text in [
                    "CHI=MOT:Child,MOT=CHI:Mother",
                    "CHI=MOT:Child,MOT=drop",
                    "CHI=CHI:Child",
                    "CHI=INV:Child,MOT=ADU:Mother",
                    "ZZZ=CHI:Child",
                ] {
                    let output =
                        apply_mapping_chat(document, &parse_mapping_spec(text).unwrap()).unwrap();
                    let errors = ErrorCollector::new();
                    let rewritten = parser
                        .parse_chat_file(&output, 0, &errors)
                        .into_option()
                        .unwrap();
                    rewritten.validate(&errors, TranscriptName::Anonymous);
                    assert!(!errors.has_errors(), "{text}: {:?}", errors.into_vec());
                    assert_eq!(serde_json::to_value(document).unwrap(), before);
                }
            }
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&talkbank_parser_re2c::Re2cParser::new());
}
