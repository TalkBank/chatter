//! Rejected participant source retains identity refusal across transports.

use crate::declared_speakers::assert_speaker_transforms_refuse;
use talkbank_parser::TreeSitterParser;

/// Structural recovery cannot promote a fragmentary token to a speaker identity.
#[test]
fn rejected_participant_syntax_survives_json_without_identity_cascades() {
    use talkbank_model::model::{ChatFile, Header, SpeakerCode, SpeakerMembership, TranscriptName};
    use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
    use talkbank_parser_re2c::Re2cParser;
    use talkbank_parser_tests::repo_paths::workspace_root;

    fn check(parser: &impl ChatParser, source: &str) {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 13, &errors)
            .into_option()
            .expect("recovered file");
        let decoded: ChatFile =
            serde_json::from_value(serde_json::to_value(&file).expect("wire")).expect("import");
        for document in [&file, &decoded] {
            let validation = ErrorCollector::new();
            document.validate(&validation, TranscriptName::Anonymous);
            let findings = validation.into_vec();
            assert!(
                findings
                    .iter()
                    .any(|error| error.severity == talkbank_model::Severity::Error)
            );
            assert!(
                findings.iter().all(|error| !matches!(
                    error.code,
                    ErrorCode::SpeakerNotDefined
                        | ErrorCode::OrphanIDHeader
                        | ErrorCode::EmptyParticipantRole
                )),
                "{}: {findings:?}",
                parser.parser_name()
            );
            for header in document.headers() {
                if let Header::UnparsedParticipants(raw) = header {
                    let original = source
                        .split(['\r', '\n'])
                        .find(|line| line.starts_with("@Participants:"))
                        .expect("header");
                    assert_eq!(raw.source().trim_end_matches(['\r', '\n']), original);
                    assert!(document.participant_declaration().is_err());
                    assert_speaker_transforms_refuse(
                        document,
                        talkbank_model::model::ParticipantDeclarationError::Rejected,
                    );
                    assert!(matches!(
                        talkbank_transform::redact::sanitize(
                            document.clone(),
                            &talkbank_transform::redact::SanitizationPolicy::strict()
                        ),
                        Err(talkbank_transform::redact::RedactError::UnparsedParticipants)
                    ));

                    assert_eq!(document.declared_speakers().count(), 0);
                    let roster =
                        talkbank_model::model::ParticipantRoster::from_headers(document.headers());
                    assert_eq!(
                        roster.membership(&SpeakerCode::new("CH")),
                        SpeakerMembership::Indeterminate
                    );
                    assert!(document.to_chat().contains(original));
                }
            }
            match document.participant_declaration() {
                Ok(entries) => {
                    // Lexical admission may retain the complete invalid code,
                    // but cannot erase the declaration or invent an identity.
                    assert!(!entries.is_empty());
                    for entry in entries {
                        assert!(["CHÎ", "子"].contains(&entry.speaker_code.as_str()));
                    }
                }
                Err(talkbank_model::model::ParticipantDeclarationError::Rejected) => {
                    assert!(
                        document
                            .headers()
                            .any(|header| { matches!(header, Header::UnparsedParticipants(_)) })
                    );
                }
                Err(other) => panic!("unexpected declaration state: {other:?}"),
            }
        }
        assert!(errors.into_vec().iter().all(|error| !matches!(
            error.code,
            ErrorCode::SpeakerNotDefined
                | ErrorCode::OrphanIDHeader
                | ErrorCode::EmptyParticipantRole
        )));
    }
    let tree_sitter = TreeSitterParser::new().expect("parser");
    let native = Re2cParser::new();
    for (spec, example) in [("E307", 4), ("E307", 5), ("E506", 3)] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{spec}_{example}.cha"
        )))
        .expect("authored fixture");
        for ending in ["\n", "\r\n", "\r"] {
            let source = source.replace('\n', ending);
            check(&tree_sitter, &source);
            check(&native, &source);
        }
    }
}
