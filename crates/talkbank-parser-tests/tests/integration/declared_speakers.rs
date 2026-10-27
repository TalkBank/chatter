// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Tests for `ChatFile::declared_speakers`.
//!
//! The property under test is that the `@Participants` header, not the `@ID`
//! join, decides who is in a transcript. It needs a real parse to be
//! meaningful, since the divergence between the two is created by the parser's
//! participant builder rather than by anything a hand-built model would
//! reproduce, so it lives here rather than in `talkbank-model`.
//!
//! Parse idiom (inherent `parse_chat_file`, not the trait method of the same
//! name): see the header of `utterance_containment.rs`.

use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::ParseProductTestExt;
use talkbank_parser_tests::test_error::TestError;

/// Header edits and legacy JSON cannot supply stale demographics to readers.
#[test]
fn participant_metadata_tracks_current_headers_in_both_backends() {
    use talkbank_model::model::{ChatFile, Header, Line, TranscriptName};
    use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
    use talkbank_parser_re2c::Re2cParser;
    use talkbank_parser_tests::repo_paths::workspace_root;

    fn check(parser: &impl ChatParser, source: &str) {
        let errors = ErrorCollector::new();
        let mut file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .expect("reference");
        assert!(!errors.has_errors());
        let original = serde_json::to_value(
            talkbank_model::model::participant::join::build_participants_from_lines(&file.lines)
                .report_into(&errors),
        )
        .expect("detached legacy snapshot");
        assert_eq!(
            file.get_participant("CHI").expect("CHI").age(),
            Some("1;08.02")
        );
        assert!(file.get_participant("CHI").expect("CHI").has_birth_date());
        assert_eq!(
            talkbank_transform::speaker_id::judgment::session_context(None, "reference", &file)
                .age_months,
            Some(talkbank_transform::speaker_id::judgment::AgeMonths(20))
        );

        let birth = file.lines.iter().position(|line| matches!(line,
            Line::Header { header, .. } if matches!(header.as_ref(), Header::Birth { participant, .. } if participant.as_str() == "CHI")
        )).expect("birth header");
        file.lines.remove(birth);
        assert!(!file.get_participant("CHI").expect("CHI").has_birth_date());
        let id = file.lines.iter().position(|line| matches!(line,
            Line::Header { header, .. } if matches!(header.as_ref(), Header::ID(id) if id.speaker.as_str() == "CHI")
        )).expect("ID header");
        file.lines.remove(id);

        let mut wire = serde_json::to_value(&file).expect("wire");
        assert!(wire.get("participants").is_none());
        wire["participants"] = original;
        let imported: ChatFile = serde_json::from_value(wire).expect("legacy JSON");
        for document in [&file, &imported] {
            assert!(document.get_participant("CHI").is_none());
            assert_eq!(
                talkbank_transform::speaker_id::judgment::session_context(
                    None,
                    "reference",
                    document
                )
                .age_months,
                None
            );
            assert_eq!(document.participant_count(), 2);
            assert_eq!(document.declared_speakers().count(), 3);
            assert!(
                document
                    .declared_speakers()
                    .find(|speaker| speaker.code().as_str() == "CHI")
                    .expect("declared CHI")
                    .id_metadata()
                    .is_none()
            );
            let errors = ErrorCollector::new();
            document.validate(&errors, TranscriptName::Anonymous);
            assert_eq!(
                errors
                    .to_vec()
                    .iter()
                    .filter(|error| error.code == ErrorCode::SpeakerNotDefined)
                    .count(),
                1
            );
            assert!(
                serde_json::to_value(document)
                    .expect("export")
                    .get("participants")
                    .is_none()
            );
        }
    }
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/headers-speaker-info.cha"),
    )
    .expect("reference");
    check(&TreeSitterParser::new().expect("parser"), &source);
    check(&Re2cParser::new(), &source);
}

pub(crate) fn assert_speaker_transforms_refuse(
    document: &talkbank_model::model::ChatFile,
    expected: talkbank_model::model::ParticipantDeclarationError,
) {
    use talkbank_transform::{rediarize, speaker_id};
    let mapping = speaker_id::parse_mapping_spec("CHI=INV:Investigator").expect("mapping");
    assert!(matches!(
        speaker_id::apply_mapping_chat(document, &mapping),
        Err(speaker_id::SpeakerIdError::SpeakerRewrite(reason)) if reason == expected.into()
    ));
    let timeline = rediarize::DiarizationTimeline::new(Vec::new());
    assert_eq!(
        rediarize::rediarize(document, &timeline, None, &talkbank_model::NullErrorSink)
            .unwrap_err(),
        expected.into()
    );
}

/// Public mutable-model callers must not rewrite through absent or competing declarations.
#[test]
fn speaker_transform_admission_distinguishes_absent_and_repeated_headers() {
    use talkbank_model::model::{ChatFile, Header, Line, ParticipantDeclarationError};
    let source = include_str!("../error_corpus/validation_errors/E513_2.cha");
    let file = TreeSitterParser::new()
        .expect("parser")
        .parse_chat_file(source)
        .expect_built();
    let original: Vec<_> = file.lines.iter().cloned().collect();
    let declaration = original
        .iter()
        .find(|line| {
            matches!(line,
        Line::Header { header, .. } if matches!(header.as_ref(), Header::Participants { .. }))
        })
        .expect("declaration")
        .clone();
    let absent = ChatFile::new(
        original
            .iter()
            .filter(|line| {
                !matches!(line,
        Line::Header { header, .. } if matches!(header.as_ref(), Header::Participants { .. }))
            })
            .cloned()
            .collect(),
    );
    let mut repeated_lines = original;
    repeated_lines.push(declaration);
    let repeated = ChatFile::new(repeated_lines);
    for (document, reason) in [
        (&absent, ParticipantDeclarationError::Absent),
        (&repeated, ParticipantDeclarationError::Repeated),
    ] {
        assert_speaker_transforms_refuse(document, reason);
        let decoded =
            serde_json::from_value(serde_json::to_value(document).expect("wire")).expect("import");
        assert_speaker_transforms_refuse(&decoded, reason);
    }
}

/// Authored omissions survive both parsers and JSON without borrowed ID roles.
#[test]
fn authored_missing_roles_preserve_declarations_without_cascades() {
    use talkbank_model::model::{ChatFile, TranscriptName};
    use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
    use talkbank_parser_re2c::Re2cParser;
    use talkbank_parser_tests::repo_paths::workspace_root;

    fn check(parser: &impl ChatParser, example: usize, source: &str, offset: usize) {
        let missing = matches!(example, 1 | 4);
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, offset, &errors)
            .into_option()
            .expect("retained file");
        let findings = errors.into_vec();
        assert_eq!(
            findings
                .iter()
                .any(|error| error.code == ErrorCode::EmptyParticipantRole),
            missing
        );
        assert!(
            findings
                .iter()
                .all(|error| error.code == ErrorCode::EmptyParticipantRole),
            "{}: {findings:?}",
            parser.parser_name()
        );
        for finding in &findings {
            let span = finding.location.span().expect("source-located omission");
            assert!(span.start() >= offset && span.end() <= offset + source.len());
        }
        let decoded: ChatFile =
            serde_json::from_value(serde_json::to_value(&file).expect("wire")).expect("import");
        let mut wire = serde_json::to_value(&file).expect("wire");
        wire["participants"] = serde_json::json!({"CHI": {"details": {"role": "Investigator"}}});
        let stale_metadata: ChatFile = serde_json::from_value(wire).expect("legacy cache ignored");
        for document in [&file, &decoded, &stale_metadata] {
            let entries = document.participant_entries().expect("declaration");
            assert_eq!(entries.len(), if example <= 2 { 1 } else { 2 });
            assert_eq!(
                entries
                    .iter()
                    .filter(|entry| entry.role().is_none())
                    .count(),
                usize::from(missing)
            );
            let errors = ErrorCollector::new();
            document.validate(&errors, TranscriptName::Anonymous);
            let findings = errors.into_vec();
            assert_eq!(
                findings
                    .iter()
                    .any(|error| error.code == ErrorCode::EmptyParticipantRole),
                missing
            );
            assert!(
                findings
                    .iter()
                    .all(|error| error.code == ErrorCode::EmptyParticipantRole),
                "{findings:?}"
            );
            assert_eq!(
                document.to_chat(),
                source.replace("\r\n", "\n").replace('\r', "\n")
            );
        }
        if example <= 2 {
            let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
            let body = normalized
                .lines()
                .find_map(|line| line.strip_prefix("@Participants:\t"))
                .expect("authored declaration");
            let errors = ErrorCollector::new();
            assert_eq!(
                parser
                    .parse_participant_entry(body, offset, &errors)
                    .into_option()
                    .is_none(),
                missing
            );
        }
    }

    let tree_sitter = TreeSitterParser::new().expect("parser");
    let native = Re2cParser::new();
    for example in 1..=4 {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E513_{example}.cha"
        )))
        .expect("authored E513 example");
        for ending in ["\n", "\r\n", "\r"] {
            let transported = source.replace('\n', ending);
            for offset in [0, 19] {
                check(&tree_sitter, example, &transported, offset);
                check(&native, example, &transported, offset);
            }
        }
    }
}

/// Missing ID metadata must not turn a declared speaker into an unknown speaker.
#[test]
fn authored_roster_validation_does_not_use_the_metadata_join() {
    use talkbank_model::model::{ChatFile, SpeakerMembership, TranscriptName};
    use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
    use talkbank_parser_re2c::Re2cParser;
    use talkbank_parser_tests::repo_paths::workspace_root;

    fn check(parser: &impl ChatParser, example: usize) {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E522_{example}.cha"
        )))
        .expect("authored E522 example");
        let parse_errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(&source, 0, &parse_errors)
            .into_option()
            .expect("retained model");
        if example == 2 {
            assert!(file.get_participant("CHI").is_none(), "missing ID metadata");
            assert!(
                parse_errors
                    .into_vec()
                    .iter()
                    .any(|error| error.code == ErrorCode::SpeakerNotDefined)
            );
        }
        let decoded: ChatFile =
            serde_json::from_value(serde_json::to_value(&file).expect("wire")).expect("import");
        let mut wire = serde_json::to_value(&file).expect("wire");
        wire["participants"] = serde_json::json!({});
        let without_cache: ChatFile =
            serde_json::from_value(wire).expect("legacy empty cache ignored");
        for document in [&file, &decoded, &without_cache] {
            let errors = ErrorCollector::new();
            let context = document.validate_headers_only(&errors, TranscriptName::Anonymous);
            for declared in document.declared_speakers() {
                assert_eq!(
                    context
                        .shared
                        .participant_roster
                        .membership(declared.code()),
                    SpeakerMembership::Declared
                );
            }
            let errors = ErrorCollector::new();
            document.validate(&errors, TranscriptName::Anonymous);
            let findings = errors.into_vec();
            assert_eq!(
                findings
                    .iter()
                    .filter(|error| error.code == ErrorCode::SpeakerNotDefined)
                    .count(),
                1,
                "retain the genuine E522 declaration/metadata error exactly once: {findings:?}"
            );
            for code in [
                ErrorCode::UndeclaredSpeaker,
                ErrorCode::SpeakerNotFoundInParticipants,
            ] {
                assert_eq!(
                    findings.iter().any(|error| error.code == code),
                    example == 1,
                    "{} E522_{example}: {findings:?}",
                    parser.parser_name()
                );
            }
        }
    }
    let tree_sitter = TreeSitterParser::new().expect("parser");
    let native = Re2cParser::new();
    for example in [1, 2] {
        check(&tree_sitter, example);
        check(&native, example);
    }
}

/// Parses a file that is deliberately INVALID, keeping the model the parser
/// built rather than discarding it along with the diagnostics.
///
/// `strict_parse` is the right helper for fixtures expected to be clean; these
/// tests are about what the model holds when the file is not, which is what
/// `expect_built` gives: the model regardless of diagnostics.
fn parse_keeping_model(source: &str) -> Result<talkbank_model::model::ChatFile, TestError> {
    Ok(TreeSitterParser::new()?
        .parse_chat_file(source)
        .expect_built())
}

/// A clean file: every declared speaker carries its `@ID` metadata, and the
/// order is the declaration order rather than any incidental map order.
#[test]
fn declared_speakers_are_enriched_and_in_declaration_order() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n\
        @Participants:\tCHI Ruth Target_Child, MOT Mary Mother\n\
        @ID:\teng|corpus|CHI|10;03.||||Target_Child|||\n\
        @ID:\teng|corpus|MOT|||||Mother|||\n\
        *CHI:\thi .\n@End\n";
    let file = talkbank_parser_tests::test_error::strict_parse(
        TreeSitterParser::new()?.parse_chat_file(source),
    )?;

    let declared: Vec<_> = file.declared_speakers().collect();
    assert_eq!(declared.len(), 2, "both declared speakers must appear");

    assert_eq!(declared[0].code().as_str(), "CHI");
    assert_eq!(declared[0].name().map(|n| n.as_str()), Some("Ruth"));
    assert_eq!(
        declared[0].role().expect("authored role").as_str(),
        "Target_Child"
    );
    assert!(declared[0].id_metadata().is_some());
    assert_eq!(
        declared[0]
            .id_metadata()
            .and_then(|m| m.age())
            .map(|a| a.to_string()),
        Some("10;03.".to_string()),
        "the @ID header's age must reach the caller through the enrichment"
    );

    assert_eq!(declared[1].code().as_str(), "MOT");
    assert!(declared[1].id_metadata().is_some());
    Ok(())
}

/// The reported defect: a speaker declared in `@Participants` with no `@ID`
/// header is absent from the participant map, so the map under-reports the
/// roster. `declared_speakers` reports it, with `id_metadata()` `None`.
#[test]
fn a_speaker_without_an_id_header_is_still_declared() -> Result<(), TestError> {
    // MOT is declared but has no @ID. This file is INVALID (E522); the point
    // is what the model holds about it, not whether it validates.
    let source = "@UTF8\n@Begin\n@Languages:\teng\n\
        @Participants:\tCHI Ruth Target_Child, MOT Mary Mother\n\
        @ID:\teng|corpus|CHI|10;03.||||Target_Child|||\n\
        *CHI:\thi .\n@End\n";
    let file = parse_keeping_model(source)?;

    assert_eq!(
        file.all_participants().len(),
        1,
        "precondition: the @ID join drops MOT, which is the defect being covered"
    );

    let declared: Vec<_> = file.declared_speakers().collect();
    assert_eq!(
        declared.len(),
        2,
        "the roster comes from @Participants, so MOT must still be reported"
    );

    assert_eq!(declared[1].code().as_str(), "MOT");
    assert_eq!(declared[1].name().map(|n| n.as_str()), Some("Mary"));
    assert_eq!(
        declared[1].role().expect("authored role").as_str(),
        "Mother",
        "role comes from the declaration, which is the header that establishes it"
    );
    assert!(
        declared[1].id_metadata().is_none(),
        "the missing @ID must be visible to the caller, not silently absent"
    );
    Ok(())
}

/// No `@Participants` header at all yields an empty iterator rather than a
/// panic or a fabricated roster.
#[test]
fn no_participants_header_yields_no_declared_speakers() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n*CHI:\thi .\n@End\n";
    let file = parse_keeping_model(source)?;
    assert_eq!(file.declared_speakers().count(), 0);
    Ok(())
}

/// Participant-specific payloads keep their identity through rename/drop and wire import.
#[test]
fn reference_speaker_mapping_reconciles_personal_headers() {
    use talkbank_model::{ChatParser, ErrorCollector, TranscriptName, WriteChat};
    use talkbank_transform::speaker_id::{apply_mapping_chat, parse_mapping_spec};
    fn check(parser: &impl ChatParser) {
        let original = include_str!("../../../../corpus/reference/core/headers-speaker-info.cha");
        for ending in ["\n", "\r\n", "\r"] {
            let source = original.replace('\n', ending);
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(&source, 200, &errors)
                .into_option()
                .unwrap();
            assert!(!errors.has_errors(), "{}", parser.parser_name());
            let decoded = serde_json::from_value(serde_json::to_value(&file).unwrap()).unwrap();
            for document in [&file, &decoded] {
                let before = serde_json::to_value(document).unwrap();
                for (mapping, expected) in [
                    (
                        "CHI=INV:Investigator,MOT=ADU:Adult,F_A_T=PAR:Participant",
                        vec![
                            "@Birth of INV:\t28-JUN-2001",
                            "@Birth of ADU:\t15-MAR-1975",
                            "@Birthplace of ADU:\tTaipei, Taiwan",
                            "@L1 of PAR:\tara",
                        ],
                    ),
                    ("MOT=drop,F_A_T=drop", vec!["@Birth of CHI:\t28-JUN-2001"]),
                    (
                        "MOT=ADU:Adult",
                        vec![
                            "@Birth of CHI:\t28-JUN-2001",
                            "@Birth of ADU:\t15-MAR-1975",
                            "@Birthplace of ADU:\tTaipei, Taiwan",
                            "@L1 of F_A_T:\tara",
                        ],
                    ),
                ] {
                    let output =
                        apply_mapping_chat(document, &parse_mapping_spec(mapping).unwrap())
                            .unwrap();
                    let actual: Vec<_> = output
                        .lines()
                        .filter(|line| {
                            line.starts_with("@Birth of ")
                                || line.starts_with("@Birthplace of ")
                                || line.starts_with("@L1 of ")
                        })
                        .collect();
                    assert_eq!(actual, expected, "{}: {mapping}", parser.parser_name());
                    let errors = ErrorCollector::new();
                    let rewritten = parser
                        .parse_chat_file(&output, 0, &errors)
                        .into_option()
                        .unwrap();
                    rewritten.validate(&errors, TranscriptName::Anonymous);
                    assert!(
                        !errors.has_errors(),
                        "{}: {mapping}: {:?}",
                        parser.parser_name(),
                        errors.into_vec()
                    );
                    let unchanged_comments: Vec<_> = document
                        .headers()
                        .filter(|h| matches!(h, talkbank_model::Header::Comment { .. }))
                        .map(WriteChat::to_chat_string)
                        .collect();
                    let rewritten_comments: Vec<_> = rewritten
                        .headers()
                        .filter(|h| matches!(h, talkbank_model::Header::Comment { .. }))
                        .map(WriteChat::to_chat_string)
                        .collect();
                    assert_eq!(unchanged_comments, rewritten_comments);
                }
                assert_eq!(serde_json::to_value(document).unwrap(), before);
            }
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&talkbank_parser_re2c::Re2cParser::new());
}
