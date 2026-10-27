//! Rejected native ID source remains invalid through validation, JSON and transforms.
//!
//! This public-source workflow is registered once in the canonical validation
//! family; it does not require identical recovery shapes from the peer parsers.

#[test]
fn native_id_language_list_rejection_preserves_source() {
    use talkbank_model::{
        ChatParser, ErrorCollector, Header, Line, ParseOutcome, TranscriptName, WriteChat,
    };
    let parser = talkbank_parser_re2c::Re2cParser::new();
    let original = include_str!("../error_corpus/validation_errors/E510_1.cha");
    let control_source = original.replace("@ID:\t|", "@ID:\teng|");
    let control = parser
        .parse_chat_file(&control_source, 0, &talkbank_model::NullErrorSink)
        .into_option()
        .unwrap();
    let retained = control.unique_utterance_speakers();
    for languages in ["eng,,ara", ",eng", "eng,", ",", "eng, ,ara"] {
        let original = original.replace("@ID:\t|", &format!("@ID:\t{languages}|"));
        for ending in ["\n", "\r\n", "\r"] {
            let source = original.replace('\n', ending);
            let raw = original
                .lines()
                .find(|line| line.starts_with("@ID:"))
                .unwrap();
            assert!(
                talkbank_parser_re2c::parser::parse_id_header(raw.strip_prefix("@ID:\t").unwrap())
                    .is_none()
            );
            let errors = ErrorCollector::new();
            assert!(matches!(
                parser.parse_id_header(raw, 200, &errors),
                ParseOutcome::Rejected
            ));
            assert!(errors.has_errors());
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(&source, 200, &errors)
                .into_option()
                .unwrap();
            let findings = errors.into_vec();
            assert!(
                findings
                    .iter()
                    .all(|error| error.code != talkbank_model::ErrorCode::SpeakerNotDefined)
            );
            assert!(
                file.headers()
                    .any(|header| matches!(header, Header::UnparsedID(_)))
            );
            let decoded: talkbank_model::ChatFile =
                serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
            for model in [file, decoded] {
                assert!(model.lines.iter().all(|line| !matches!(line, Line::Header { header, .. } if matches!(&**header, Header::ID(_)))));
                assert!(model.to_chat_string().contains(raw));
                let before = serde_json::to_value(&model).unwrap();
                use talkbank_transform::{
                    SpeakerRewriteError, rediarize, speaker_id, transcript_merge,
                };
                let mapping = speaker_id::parse_mapping_spec("CHI=INV:Investigator").unwrap();
                let refusal = speaker_id::apply_mapping_chat(&model, &mapping).unwrap_err();
                let speaker_id::SpeakerIdError::SpeakerRewrite(refusal) = refusal else {
                    panic!("expected unreadable input identity refusal");
                };
                assert_eq!(refusal, SpeakerRewriteError::UnparsedID);
                let recorded = speaker_id::RecordedSpeakerIdentificationAttempt::input_rejected(
                    speaker_id::RecordedSpeakerIdentificationInput::Donor,
                    &talkbank_transform::PipelineError::from(refusal),
                );
                let recorded = serde_json::to_value(recorded).unwrap();
                assert_eq!(recorded["failure_kind"], "unparsed_id");
                assert_eq!(
                    rediarize::rediarize(
                        &model,
                        &rediarize::DiarizationTimeline::new(Vec::new()),
                        None,
                        &talkbank_model::NullErrorSink,
                    )
                    .unwrap_err(),
                    SpeakerRewriteError::UnparsedID
                );
                for (reference, donor, expected_input) in [
                    (&model, &control, transcript_merge::MergeInput::Reference),
                    (&control, &model, transcript_merge::MergeInput::Donor),
                ] {
                    assert!(matches!(
                        transcript_merge::merge_chat_files_by_source_order(reference, donor, &retained, &[]),
                        Err(transcript_merge::MergeError::SpeakerHeaders {input, reason: SpeakerRewriteError::UnparsedID}) if input == expected_input
                    ));
                }
                assert!(
                    speaker_id::apply_mapping_chat(&control, &mapping)
                        .unwrap()
                        .contains("*INV:")
                );
                assert_eq!(serde_json::to_value(&model).unwrap(), before);
                let errors = ErrorCollector::new();
                model.validate(&errors, TranscriptName::Anonymous);
                let findings = errors.into_vec();
                assert!(
                    findings
                        .iter()
                        .all(|error| error.code != talkbank_model::ErrorCode::SpeakerNotDefined)
                );
                assert!(matches!(
                    talkbank_transform::redact::sanitize(
                        model.clone(),
                        &talkbank_transform::redact::SanitizationPolicy::strict()
                    ),
                    Err(talkbank_transform::redact::RedactError::UnparsedID)
                ));
                assert!(
                    findings
                        .iter()
                        .any(|error| error.code == talkbank_model::ErrorCode::UnparsableContent)
                );
            }
        }
    }
    // A genuinely empty field is still represented as observed absence for E510.
    assert!(
        talkbank_parser_re2c::parser::parse_id_header("|corpus|CHI|||||Target_Child|||")
            .unwrap()
            .language
            .is_empty()
    );
}
