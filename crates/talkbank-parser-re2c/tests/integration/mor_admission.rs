//! Morphology admission and recovery through the public file parser.

use talkbank_model::model::{ParseHealthState, ParseHealthTier};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome, SemanticEq, Span};

#[test]
fn rejected_morphology_retains_taint_and_reports_its_source_boundary() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E316.md")
        .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    let invalid: Vec<_> = spec
        .examples()
        .iter()
        .filter(|example| example.chat.as_str().contains("<sos>"))
        .collect();
    assert_eq!(invalid.len(), 2);
    for example in invalid {
        let source = example.chat.as_str();
        let start = source.find("%mor:").unwrap();
        let end = start + source[start..].find('\n').unwrap();
        for offset in [0, 200] {
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(chat) = parser.parse_chat_file(source, offset, &errors) else {
                panic!("recovery must retain the main utterance");
            };
            let utterance = chat.utterances().next().unwrap();
            assert!(utterance.parse_health.is_tier_tainted(ParseHealthTier::Mor));
            assert!(utterance.dependent_tiers.is_empty());
            let diagnostics = errors.into_vec();
            assert!(
                diagnostics
                    .iter()
                    .any(|error| error.code == ErrorCode::UnparsableContent)
            );
            let tier_errors: Vec<_> = diagnostics
                .iter()
                .filter(|error| error.code == ErrorCode::TierValidationError)
                .collect();
            assert_eq!(tier_errors.len(), 1);
            assert_eq!(
                tier_errors[0].location.span,
                Span::from_usize(start + offset, end + offset)
            );
        }
    }

    let legal = spec
        .examples()
        .iter()
        .find(|example| example.chat.as_str().contains("noun|tos-Sg"))
        .unwrap();
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(actual) = parser.parse_chat_file(legal.chat.as_str(), 0, &errors)
    else {
        panic!("legal morphology must parse");
    };
    assert!(errors.into_vec().is_empty());
    assert_eq!(
        actual.utterances().next().unwrap().parse_health,
        ParseHealthState::Clean
    );
    let canonical = talkbank_parser::TreeSitterParser::new()
        .unwrap()
        .parse_chat_file(legal.chat.as_str());
    assert!(!canonical.has_error_diagnostics());
    assert!(canonical.expect_built().semantic_eq(&actual));
}

#[test]
fn empty_pos_diagnostics_name_the_original_item_at_any_offset() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E760.md")
        .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    let mut cases: Vec<_> = spec
        .examples()
        .iter()
        .map(|example| {
            let source = example.chat.as_str();
            (
                source.to_owned(),
                if source.contains("|home") {
                    "|home"
                } else {
                    "|we"
                },
            )
        })
        .collect();
    // A rich token contains the same substring first. Non-ASCII bytes,
    // Unicode whitespace and a continuation must not change the selected span.
    cases.push((
        spec.examples()[0]
            .chat
            .as_str()
            .replace("%mor:\t|we v|go .", "%mor:\tn|été n|we\u{2003}\n\t|we ."),
        "|we",
    ));
    for (source, item) in cases {
        let source = source.as_str();
        let start = source.rfind(item).unwrap();
        for offset in [0, 200] {
            let errors = ErrorCollector::new();
            let _ = parser.parse_chat_file(source, offset, &errors);
            let diagnoses: Vec<_> = errors
                .into_vec()
                .into_iter()
                .filter(|error| error.code == ErrorCode::MorItemEmptyPos)
                .collect();
            assert_eq!(diagnoses.len(), 1);
            assert_eq!(
                diagnoses[0].location.span,
                Span::from_usize(start + offset, start + item.len() + offset)
            );
            assert!(diagnoses[0].message.contains(item));
        }
    }
}
