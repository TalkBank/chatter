//! Public pause projections preserve authored precision and wire spelling.
use talkbank_model::ErrorCollector;
use talkbank_model::model::{
    Pause, PauseDuration, PauseTimedDuration, TranscriptName, UtteranceContent, WriteChat,
};

#[test]
fn timed_pause_schema_matches_model_admission() {
    use talkbank_model::model::TimedPause;
    let schema = serde_json::to_value(schemars::schema_for!(TimedPause)).unwrap();
    let schema = jsonschema::validator_for(&schema).unwrap();
    for text in [
        "0.",
        "7:1.234567",
        "4294967296.5",
        "71582788:16.0",
        "",
        "2",
        "1:2",
        "+1.5",
        "-1.5",
        ".5",
        "1:2:3.4",
        "1.2.3",
        "1.5\n",
        "１.5",
        "1.5)",
    ] {
        let wire = serde_json::json!({ "seconds": text });
        assert_eq!(
            schema.is_valid(&wire),
            TimedPause::new(text).is_ok(),
            "{text:?}"
        );
        assert_eq!(
            schema.is_valid(&wire),
            serde_json::from_value::<TimedPause>(wire).is_ok(),
            "{text:?}"
        );
    }
}
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;

#[test]
fn reference_pause_precision_preserves_display_and_json() {
    let source =
        std::fs::read_to_string(workspace_root().join("corpus/reference/content/pauses-timed.cha"))
            .expect("reference pauses");
    let parser = TreeSitterParser::new().expect("parser");
    let mut file = strict_parse(parser.parse_chat_file(&source)).expect("clean parse");
    let errors = ErrorCollector::new();
    file.validate_with_alignment(&errors, TranscriptName::Anonymous);
    assert!(errors.is_empty(), "{:?}", errors.to_vec());
    let mut observed = Vec::new();
    let mut pauses = 0;
    for utterance in file.utterances() {
        for item in &utterance.main.content.content {
            let UtteranceContent::Pause(pause) = item else {
                continue;
            };
            pauses += 1;
            assert_eq!(pause.to_string(), pause.to_chat_string());
            let json = serde_json::to_string(pause).expect("pause JSON");
            let decoded: Pause = serde_json::from_str(&json).expect("pause admission");
            assert_eq!(decoded.to_chat_string(), pause.to_chat_string());
            if let PauseDuration::Timed(duration) = &pause.duration {
                let PauseTimedDuration::Parsed(parsed) = duration.projection() else {
                    panic!("reference numeric pause must have a checked projection");
                };
                assert_eq!(parsed.as_str(), duration.as_str());
                observed.push((
                    duration.as_str(),
                    parsed.seconds(),
                    parsed.millis(),
                    duration.total_millis(),
                ));
                // External JSON must carry the authored spelling; missing or
                // numeric values cannot substitute an invented duration.
                let wire = serde_json::to_value(duration).expect("duration JSON");
                assert_eq!(wire["seconds"], duration.as_str());
                let mut missing = wire.clone();
                missing
                    .as_object_mut()
                    .expect("duration object")
                    .remove("seconds");
                assert!(serde_json::from_value::<PauseTimedDuration>(missing).is_err());
                let mut wrong_type = wire;
                wrong_type["seconds"] = serde_json::json!(duration.total_millis());
                assert!(serde_json::from_value::<PauseTimedDuration>(wrong_type).is_err());
                let PauseDuration::Timed(decoded_duration) = decoded.duration else {
                    panic!("JSON lost timed duration");
                };
                assert_eq!(&decoded_duration, duration);
            }
        }
    }
    assert_eq!(pauses, 9, "three named and six numeric pauses");
    assert_eq!(
        observed,
        [
            ("3.", 3, None, Some(3000)),
            ("0.5", 0, Some(500), Some(500)),
            ("1:30.5", 90, Some(500), Some(90500)),
            ("0.25", 0, Some(250), Some(250)),
            ("0.125", 0, Some(125), Some(125)),
            ("0.1234", 0, Some(123), Some(123)),
        ]
    );
    assert_eq!(
        file.to_chat_string(),
        source,
        "precision is not rounded in CHAT"
    );
}

#[test]
fn large_pause_syntax_keeps_syllable_and_alignment_roles_without_projection() {
    use talkbank_model::{ChatParser, ErrorCode};
    enum Expected {
        Clean,
        Reconstruction(ErrorCode),
    }
    fn check(parser: &impl ChatParser) {
        let specs = talkbank_parser_tests::error_specs::load(workspace_root()).unwrap();
        for (filename, title, expected) in [
            (
                "E735.md",
                "Large numeric pauses remain fillers without a machine-sized projection",
                Expected::Clean,
            ),
            (
                "E737.md",
                "Different large pause spellings still fail model reconstruction",
                Expected::Reconstruction(ErrorCode::ModsylReconstructionMismatch),
            ),
            (
                "E738.md",
                "Different large pause spellings still fail actual reconstruction",
                Expected::Reconstruction(ErrorCode::PhosylReconstructionMismatch),
            ),
            (
                "E740.md",
                "Large actual-only pause does not advance the model position",
                Expected::Clean,
            ),
            (
                "E741.md",
                "Large model-only pause does not advance the actual position",
                Expected::Clean,
            ),
        ] {
            let spec = specs.iter().find(|spec| spec.filename == filename).unwrap();
            let example = spec
                .examples()
                .iter()
                .find(|example| example.title.as_deref() == Some(title))
                .unwrap();
            let errors = ErrorCollector::new();
            let mut file = parser
                .parse_chat_file(example.chat.as_str(), 29, &errors)
                .into_option()
                .unwrap();
            assert!(errors.is_empty(), "{filename}: {:?}", errors.to_vec());
            file.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let codes: Vec<_> = errors
                .into_vec()
                .into_iter()
                .map(|error| error.code)
                .collect();
            let expected_codes = match expected {
                Expected::Clean => vec![],
                Expected::Reconstruction(code) => vec![code],
            };
            assert_eq!(
                codes,
                expected_codes,
                "{} {filename}: {title}",
                parser.parser_name()
            );
        }
    }
    check(&TreeSitterParser::new().unwrap());
    check(&talkbank_parser_re2c::Re2cParser::new());
}

#[test]
fn oversized_pause_fields_preserve_source_without_numeric_projection() {
    fn check(parser: &impl talkbank_model::ChatParser) {
        let specs = talkbank_parser_tests::error_specs::load(workspace_root()).unwrap();
        let spec = specs
            .iter()
            .find(|spec| spec.filename == "E316.md")
            .unwrap();
        for title in [
            "Timed pause minute field exceeds the numeric projection range",
            "Timed pause second field exceeds the numeric projection range",
        ] {
            let example = spec
                .examples()
                .iter()
                .find(|example| example.title.as_deref() == Some(title))
                .unwrap();
            let errors = ErrorCollector::new();
            let mut file = parser
                .parse_chat_file(example.chat.as_str(), 29, &errors)
                .into_option()
                .unwrap();
            file.validate_with_alignment(&errors, TranscriptName::Anonymous);
            assert!(errors.is_empty(), "{title}: {:?}", errors.to_vec());
            // Spec payloads omit the terminal newline; CHAT rendering supplies it.
            assert_eq!(
                file.to_chat_string(),
                format!("{}\n", example.chat.as_str())
            );
            let pauses: Vec<_> = file
                .utterances()
                .flat_map(|utterance| &utterance.main.content.content)
                .filter_map(|item| match item {
                    UtteranceContent::Pause(pause) => Some(pause),
                    _ => None,
                })
                .collect();
            let [pause] = pauses.as_slice() else {
                panic!("expected one pause: {title}");
            };
            let PauseDuration::Timed(duration) = &pause.duration else {
                panic!("expected timed pause: {title}");
            };
            let PauseTimedDuration::Unsupported(raw) = duration.projection() else {
                panic!("overflow must not fabricate a numeric projection: {title}");
            };
            assert_eq!(raw, duration.as_str());
            assert_eq!(duration.total_millis(), None);
            let wire = serde_json::to_value(pause).unwrap();
            let decoded: Pause = serde_json::from_value(wire).unwrap();
            assert_eq!(decoded.duration, pause.duration);
            assert_eq!(decoded.to_chat_string(), pause.to_chat_string());
        }
    }
    check(&TreeSitterParser::new().unwrap());
    check(&talkbank_parser_re2c::Re2cParser::new());
}
