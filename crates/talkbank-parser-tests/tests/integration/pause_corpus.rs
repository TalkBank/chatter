//! Public pause projections preserve authored precision and wire spelling.
use talkbank_model::ErrorCollector;
use talkbank_model::model::{
    Pause, PauseDuration, PauseTimedDuration, TranscriptName, UtteranceContent, WriteChat,
};
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
                let PauseTimedDuration::Parsed(parsed) = duration else {
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
