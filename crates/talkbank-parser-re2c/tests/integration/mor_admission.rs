//! Morphology admission and recovery through the public file parser.

use talkbank_model::model::{ParseHealthState, ParseHealthTier};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome, SemanticEq, Span};
use talkbank_parser_tests::test_error::ParseProductTestExt;

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
            assert!(
                utterance
                    .parse_health()
                    .is_tier_tainted(ParseHealthTier::Mor)
            );
            let talkbank_model::DependentTier::UnparsedMor(raw) =
                &utterance.dependent_tiers[0].tier
            else {
                panic!("rejected morphology must retain explicit source");
            };
            assert_eq!(raw.source(), &source[start..end + 1]);
            assert_eq!(
                raw.location().span(),
                Some(Span::new(start + offset, end + 1 + offset))
            );
            let diagnostics = errors.into_vec();
            assert!(
                diagnostics
                    .iter()
                    .any(|error| error.code == ErrorCode::UnparsableContent)
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
        actual.utterances().next().unwrap().parse_health(),
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
        .filter(|example| example.claim.demonstrates_own_code())
        .map(|example| {
            let source = example.chat.as_str();
            (
                source.to_owned(),
                source
                    .split_whitespace()
                    .find(|item| item.starts_with('|'))
                    .expect("violating E760 fixture contains its empty-POS item"),
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
                diagnoses[0].location.span().expect("located diagnostic"),
                Span::new(start + offset, start + item.len() + offset)
            );
            assert!(diagnoses[0].message.contains(item));
        }
    }
}

/// Reuse the existing MOR feature/clitic control; transport faults must not
/// turn a valid prefix into a successful shortened model.
#[test]
fn fragment_refusal_is_not_an_empty_or_truncated_mor_model() {
    use talkbank_parser_re2c::parser::{parse_mor_tier_with_errors, parse_mor_word_with_errors};
    let body = "pron|it~aux|be-Fin-Ind-Pres-S3 .";
    for ending in ["", "\n", "\r", "\r\n"] {
        let input = format!("{body}{ending}");
        let errors = ErrorCollector::new();
        let tier = parse_mor_tier_with_errors(&input, &errors)
            .into_option()
            .unwrap();
        assert!(errors.into_vec().is_empty());
        assert_eq!(tier.items().len(), 1);
        assert_eq!(tier.items()[0].post_clitics.len(), 1);
        assert_eq!(tier.items()[0].post_clitics[0].features.len(), 4);
        assert_eq!(tier.location.span(), Some(Span::new(0, input.len())));
    }
    for input in [
        String::new(),
        body.trim_end_matches(" .").to_owned(),
        format!("{body}\n\n"),
        format!("{body}\0ignored"),
    ] {
        let errors = ErrorCollector::new();
        assert!(parse_mor_tier_with_errors(&input, &errors).is_rejected());
        assert!(!errors.into_vec().is_empty());
    }
    let word = "verb|want-Fin-Ind-Pres";
    let errors = ErrorCollector::new();
    let parsed = parse_mor_word_with_errors(word, &errors)
        .into_option()
        .unwrap();
    assert_eq!(parsed.features.len(), 3);
    assert!(errors.into_vec().is_empty());
    for input in [
        String::new(),
        format!("{word}\0ignored"),
        format!("{word} |broken"),
    ] {
        let errors = ErrorCollector::new();
        assert!(parse_mor_word_with_errors(&input, &errors).is_rejected());
        assert!(!errors.into_vec().is_empty());
    }
}
