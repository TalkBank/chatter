//! Interposed lexical validation stays separate from the main speaker's alignment.

use talkbank_model::alignment::helpers::{ContentItem, WordItem, walk_content, walk_words};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, TranscriptName, WriteChat};

#[derive(Debug, Clone, Copy)]
enum Representation {
    Parsed,
    Json,
}

#[test]
fn interposed_words_keep_lexical_validity_through_model_and_json() {
    fn check(parser: &impl ChatParser) {
        let schema = serde_json::to_value(schemars::schema_for!(talkbank_model::ChatFile)).unwrap();
        let schema = jsonschema::validator_for(&schema).unwrap();
        let specs = talkbank_parser_tests::error_specs::load(
            talkbank_parser_tests::repo_paths::workspace_root(),
        )
        .unwrap();
        let spec = specs
            .iter()
            .find(|spec| spec.filename == "E220_interposed_words.md")
            .unwrap();
        assert_eq!(spec.examples().len(), 12);
        for (index, example) in spec.examples().iter().enumerate() {
            let source = example.chat.as_str();
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(source, 43, &errors)
                .into_option()
                .unwrap();
            assert!(errors.to_vec().is_empty(), "{:?}", errors.to_vec());
            let encoded = serde_json::to_string(&file).unwrap();
            let wire: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            let schema_errors: Vec<_> = schema
                .iter_errors(&wire)
                .map(|error| error.to_string())
                .collect();
            assert!(schema_errors.is_empty(), "{schema_errors:?}");
            let restored: talkbank_model::ChatFile = serde_json::from_str(&encoded).unwrap();
            for (representation, retained) in [
                (Representation::Parsed, &file),
                (Representation::Json, &restored),
            ] {
                let validation = ErrorCollector::new();
                retained.validate(&validation, TranscriptName::Anonymous);
                let diagnostics = validation.into_vec();
                assert_eq!(
                    diagnostics
                        .iter()
                        .filter(|error| error.code == ErrorCode::IllegalDigits)
                        .count(),
                    usize::from(example.claim.demonstrates_own_code()),
                    "{} example {}: {diagnostics:?}",
                    parser.parser_name(),
                    index + 1
                );
                assert!(
                    diagnostics
                        .iter()
                        .all(|error| error.code == ErrorCode::IllegalDigits)
                );
                if diagnostics.is_empty() {
                    let sanitized = talkbank_transform::redact::sanitize(
                        retained.clone(),
                        &talkbank_transform::redact::SanitizationPolicy::strict(),
                    )
                    .unwrap();
                    assert!(sanitized.to_chat_string().contains("&*MOT:redacted"));
                }
                let main = &retained.utterances().next().unwrap().main;
                let mut words = Vec::new();
                walk_words(&main.content.content, None, &mut |item| {
                    if let WordItem::Word(word) = item {
                        words.push(word.raw_text());
                    }
                });
                assert_eq!(words, ["hello"]);
                let mut event_count = 0;
                walk_content(&main.content.content, None, &mut |item| {
                    if let ContentItem::OtherSpokenEvent(event) = item {
                        event_count += 1;
                        for (location, spelling) in [
                            (event.location, event.to_chat_string()),
                            (event.word().location, event.word().raw_text()),
                        ] {
                            match representation {
                                Representation::Parsed => {
                                    let span = location
                                        .span()
                                        .expect("source-derived event and word locations");
                                    assert_eq!(
                                        &source[span.start() - 43..span.end() - 43],
                                        spelling
                                    );
                                }
                                Representation::Json => assert!(location.span().is_none()),
                            }
                        }
                    }
                });
                assert_eq!(event_count, 1);
                assert_eq!(retained.to_chat_string(), file.to_chat_string());
            }
        }
    }
    check(&talkbank_parser_re2c::Re2cParser::new());
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
}
