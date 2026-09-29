//! Empty and malformed header boundaries over canonical specs and JSON replay.

use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;

#[derive(Clone, Copy)]
enum HeaderField {
    Date,
    Birth,
    Duration,
    Start,
}

impl HeaderField {
    fn matches(self, header: &talkbank_model::model::Header) -> bool {
        use talkbank_model::model::Header;
        matches!(
            (self, header),
            (Self::Date, Header::Date { .. })
                | (Self::Birth, Header::Birth { .. })
                | (Self::Duration, Header::TimeDuration { .. })
                | (Self::Start, Header::TimeStart { .. })
        )
    }
}

/// These are established model/validation policies, independent of which
/// lexical alternative supplies the preserved header value.
#[test]
fn header_empty_and_suffix_specs_preserve_values_and_validation() {
    use HeaderField::{Birth, Date, Duration, Start};
    use talkbank_model::ErrorCollector;
    use talkbank_model::model::{ChatFile, TranscriptName, WriteChat};

    let parser = TreeSitterParser::new().expect("parser");
    for (specimen, field, expected) in [
        ("E518_7", Date, None),
        ("E518_16", Date, Some("E518")),
        ("E516_1", Date, Some("E516")),
        ("E545_5", Birth, None),
        ("E545_8", Birth, None),
        ("E540_numeric_boundaries_1", Duration, None),
        ("E540_numeric_boundaries_12", Duration, None),
        ("E540_numeric_boundaries_13", Duration, Some("E540")),
        ("E541_clock_3", Start, None),
        ("E541_clock_10", Start, None),
        ("E541_clock_11", Start, Some("E541")),
    ] {
        let path = workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
            .join(format!("{specimen}.cha"));
        let source = std::fs::read_to_string(path).expect("canonical header specimen");
        let parse_errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &parse_errors);
        assert!(
            parse_errors.is_empty(),
            "{specimen}: {:?}",
            parse_errors.to_vec()
        );
        let wire: ChatFile =
            serde_json::from_str(&serde_json::to_string(&file).expect("header wire output"))
                .expect("header wire admission");
        for model in [&file, &wire] {
            assert_eq!(
                model
                    .headers()
                    .filter(|header| field.matches(header))
                    .count(),
                1,
                "header must not disappear: {specimen}"
            );
            assert_eq!(
                model.to_chat_string(),
                source,
                "no invented/truncated value: {specimen}"
            );
            let errors = ErrorCollector::new();
            model.validate(&errors, TranscriptName::Anonymous);
            let codes: Vec<_> = errors
                .into_vec()
                .into_iter()
                .map(|error| error.code.to_string())
                .collect();
            assert_eq!(
                codes,
                expected.into_iter().collect::<Vec<_>>(),
                "{specimen}"
            );
        }
    }
}

/// Grammar recognition must not manufacture a modeled header or certify a
/// document containing an unsupported header. Recovery retains following speech.
#[test]
fn unsupported_thumbnail_spec_refuses_admission_without_losing_speech() {
    use talkbank_model::model::SemanticEq;
    use talkbank_model::{ErrorCode, ErrorCollector, Severity};
    use talkbank_parser_tests::test_error::strict_parse;

    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let control_source =
        std::fs::read_to_string(root.join("E525_2.cha")).expect("known comment control");
    let source =
        std::fs::read_to_string(root.join("E525_4.cha")).expect("unsupported thumbnail mutation");
    let parser = TreeSitterParser::new().expect("parser");
    let control =
        strict_parse(parser.parse_chat_file(&control_source)).expect("comment control admitted");
    let errors = ErrorCollector::new();
    let recovered = parser.parse_chat_file_streaming(&source, &errors);
    let diagnostics = errors.into_vec();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.code, ErrorCode::UnknownHeader);
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.message, "Unsupported @Thumbnail header");
    let span = diagnostic.location.span;
    assert_eq!(
        source.get(span.start as usize..span.end as usize),
        Some("@Thumbnail:\tanything at all\n"),
        "diagnostic covers the actual unsupported declaration"
    );
    assert_eq!(recovered.utterances().count(), control.utterances().count());
    assert_eq!(control.utterances().count(), 1);
    for (actual, expected) in recovered.utterances().zip(control.utterances()) {
        assert!(
            actual.semantic_eq(expected),
            "recovery must preserve following speech"
        );
    }
    let refused = strict_parse(parser.parse_chat_file(&source))
        .expect_err("unsupported header cannot yield a strict document");
    assert_eq!(
        refused.errors, diagnostics,
        "strict refusal retains the diagnosis"
    );
}
