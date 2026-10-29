//! Complete duration admission from an authored malformed header, through both parsers.

use talkbank_model::model::{
    ChatFile, Header, SemanticEq, TimeDurationValue, TranscriptName, WriteChat,
};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};

#[test]
fn duration_empty_segment_spec_cannot_become_a_partial_clock() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let mut specs = specs
        .iter()
        .filter(|spec| spec.filename == "E540_numeric_boundaries.md");
    let spec = specs.next().expect("duration boundary specification");
    assert!(specs.next().is_none(), "duplicate specification");
    let mut examples = spec.examples().iter().filter(|example| {
        example.title.as_deref() == Some("A trailing empty segment does not disappear")
    });
    let mut source = examples
        .next()
        .expect("authored empty-segment witness")
        .chat
        .as_str()
        .to_owned();
    assert!(examples.next().is_none(), "duplicate witness title");
    if !source.ends_with('\n') {
        source.push('\n');
    }

    fn check(parser: &impl ChatParser, source: &str) {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .unwrap();
        assert!(
            !errors.has_errors(),
            "header recovery belongs to validation"
        );
        let restored: ChatFile =
            serde_json::from_value(serde_json::to_value(&file).unwrap()).unwrap();
        for file in [&file, &restored] {
            let mut durations = file.headers().filter_map(|header| match header {
                Header::TimeDuration { duration } => Some(duration),
                _ => None,
            });
            let duration = durations.next().expect("retained duration header");
            assert!(durations.next().is_none());
            assert_eq!(duration.as_str(), "00:00:00,");
            assert!(duration.segments().is_empty());
            assert!(!duration.semantic_eq(&TimeDurationValue::from_text("00:00:00")));
            assert_eq!(file.to_chat_string(), source);
            let findings = ErrorCollector::new();
            file.validate_headers_only(&findings, TranscriptName::Anonymous);
            let findings = findings.into_vec();
            assert_eq!(findings.len(), 1, "{findings:?}");
            assert_eq!(findings[0].code, ErrorCode::InvalidTimeDuration);
        }
    }

    check(&talkbank_parser::TreeSitterParser::new().unwrap(), &source);
    check(&talkbank_parser_re2c::Re2cParser::new(), &source);
}
