//! Native rejected text preserves tier identity through rendering and transforms.

use talkbank_model::model::{ChatFile, DependentTier};
use talkbank_model::{ChatParser, ErrorCollector, WriteChat};

#[test]
fn native_text_recovery_is_removed_by_redaction_and_replaced_by_label() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E316.md")
        .unwrap();
    let required = |title| {
        spec.examples()
            .iter()
            .find(|example| example.title.as_deref() == Some(title))
            .expect("required authored recovery/control example")
    };
    let parser = talkbank_parser_re2c::Re2cParser::new();
    // Required named pairs cannot silently disappear when new specs are appended.
    for (label, rejected, control) in [
        (
            "com",
            required("Malformed media syntax in com text retains its source"),
            required("Closed media syntax in com text retains its source"),
        ),
        (
            "xnote",
            required("Malformed media syntax in xnote text retains its source"),
            required("Closed media syntax in xnote text retains its source"),
        ),
    ] {
        for ending in ["\n", "\r\n", "\r"] {
            let source = rejected.chat.as_str().replace('\n', ending);
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(&source, 13, &errors)
                .into_option()
                .unwrap();
            assert!(errors.has_errors());
            let decoded: ChatFile =
                serde_json::from_value(serde_json::to_value(&file).unwrap()).unwrap();
            let findings = ErrorCollector::new();
            let repaired = parser
                .parse_chat_file(&control.chat.as_str().replace('\n', ending), 13, &findings)
                .into_option()
                .unwrap();
            assert!(!findings.has_errors());
            for document in [&file, &decoded] {
                let mut utterance = document.utterances().next().unwrap().clone();
                let [entry] = utterance.dependent_tiers.as_slice() else {
                    panic!("expected one retained text tier");
                };
                let DependentTier::UnparsedText(raw) = &entry.tier else {
                    panic!("rejected text must not become parsed content");
                };
                assert_eq!(raw.label(), label);
                assert!(source.contains(raw.raw().source()));
                assert!(raw.raw().source().contains("1000X2000"));
                assert_eq!(
                    raw.to_chat_string(),
                    raw.raw().source().trim_end_matches(['\r', '\n'])
                );
                let validation = ErrorCollector::new();
                document.validate(
                    &validation,
                    talkbank_model::model::TranscriptName::Anonymous,
                );
                assert!(validation.has_errors(), "JSON must not certify recovery");
                let sanitized = talkbank_transform::redact::sanitize(
                    document.clone(),
                    &talkbank_transform::redact::SanitizationPolicy::strict(),
                )
                .unwrap();
                assert!(
                    !sanitized
                        .to_chat_string()
                        .lines()
                        .any(|line| line.starts_with('%'))
                );
                let tier = repaired.utterances().next().unwrap().dependent_tiers[0]
                    .tier
                    .clone();
                talkbank_transform::dependent_tiers::replace_or_add_tier(
                    &mut utterance.dependent_tiers,
                    tier,
                );
                assert_eq!(utterance.dependent_tiers.len(), 1);
                assert_eq!(utterance.dependent_tiers[0].kind(), label);
                assert!(!utterance.to_chat_string().contains("1000X2000"));
                assert!(utterance.to_chat_string().contains("1000_2000"));
            }
        }
    }
}
