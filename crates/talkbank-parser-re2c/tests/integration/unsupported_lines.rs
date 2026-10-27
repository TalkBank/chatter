//! Unsupported-line recovery retains source locations and following utterances.

use talkbank_model::model::WriteChat;
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome, Severity, Span};

fn check(parser: &impl ChatParser, source: &str, offset: usize) -> (String, Vec<Span>) {
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(file) = parser.parse_chat_file(source, offset, &errors) else {
        panic!("unsupported lines must retain the surrounding document");
    };
    let diagnostics = errors.into_vec();
    for error in &diagnostics {
        assert_eq!(error.code, ErrorCode::UnexpectedLineType);
        assert_eq!(error.severity, Severity::Error);
        let span = error.location.span().expect("located diagnostic");
        assert!(span.start() >= offset);
        let text = &source[span.start() - offset..span.end() - offset];
        assert!(text.ends_with('\n'));
        assert!(!text.starts_with(['@', '*', '%', '\t']));
        let context = error.context.as_ref().unwrap();
        assert_eq!(
            &context.source_text[context.span.start()..context.span.end()],
            text,
        );
    }
    (
        file.to_chat_string(),
        diagnostics
            .into_iter()
            .map(|e| e.location.span().expect("located diagnostic"))
            .collect(),
    )
}

#[test]
fn unsupported_line_reports_the_same_rule_and_source_in_both_backends() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E326.md")
        .unwrap();
    let canonical = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = talkbank_parser_re2c::Re2cParser::new();
    // The original bare-line and TAB-continuation contract is shared.
    // New space-indented tier cases remain explicitly recorded divergences
    // in error_parity::baseline; experimental re2c does not implement them yet.
    for example in spec.examples().iter().take(3) {
        let source = example.chat.as_str();
        for source in [source.to_owned(), source.replace('\n', "\r\n")] {
            for offset in [0, 200] {
                assert_eq!(
                    check(&canonical, &source, offset),
                    check(&re2c, &source, offset)
                );
            }
        }
    }
}

#[test]
fn retired_tier_identity_is_not_promoted_to_morphology() {
    use talkbank_model::{DependentTier, TranscriptName};
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E605.md")
        .unwrap();
    fn check(parser: &impl ChatParser, source: &str, label: &str, payload: &str, tier_line: &str) {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 19, &errors)
            .into_option()
            .unwrap();
        file.validate(&errors, TranscriptName::Anonymous);
        let tier = &file.utterances().next().unwrap().dependent_tiers[0].tier;
        let unsupported = !matches!(label, "mor" | "xtrn");
        match tier {
            DependentTier::Unsupported(raw) => {
                assert!(unsupported);
                assert_eq!(raw.label.as_str(), label);
                assert_eq!(raw.content.as_ref().unwrap().as_str(), payload);
            }
            DependentTier::Mor(_) => assert_eq!(label, "mor"),
            DependentTier::UserDefined(raw) => assert_eq!(raw.label.as_str(), "xtrn"),
            other => panic!("unexpected tier: {other:?}"),
        }
        let diagnostics = errors.into_vec();
        assert_eq!(
            diagnostics
                .iter()
                .any(|error| error.code == ErrorCode::UnsupportedDependentTier
                    && error.severity == Severity::Error),
            unsupported,
            "{label}: {diagnostics:?}"
        );
        assert!(file.to_chat_string().lines().any(|line| line == tier_line));
        let fragment_errors = ErrorCollector::new();
        let fragment = parser
            .parse_dependent_tier(tier_line, 19, &fragment_errors)
            .into_option()
            .unwrap();
        assert_eq!(fragment.to_chat_string(), tier.to_chat_string());
    }
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    assert_eq!(spec.examples().iter().skip(6).count(), 7);
    for example in spec.examples().iter().skip(6) {
        let source = example.chat.as_str();
        let tier_line = source.lines().find(|line| line.starts_with('%')).unwrap();
        let (label, payload) = tier_line[1..].split_once(":\t").unwrap();
        check(&tree, source, label, payload, tier_line);
        check(&native, source, label, payload, tier_line);
    }
}

#[test]
fn malformed_text_tiers_cannot_become_shortened_valid_content() {
    use talkbank_model::TranscriptName;
    #[derive(Clone, Copy)]
    enum TextState {
        Parsed,
        Rejected,
    }
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E316.md")
        .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    // These named pairs remain required when unrelated E316 cases are appended.
    for (title, expected) in [
        (
            "Malformed media syntax in com text retains its source",
            TextState::Rejected,
        ),
        (
            "Closed media syntax in com text retains its source",
            TextState::Parsed,
        ),
        (
            "Malformed media syntax in xnote text retains its source",
            TextState::Rejected,
        ),
        (
            "Closed media syntax in xnote text retains its source",
            TextState::Parsed,
        ),
    ] {
        let cases: Vec<_> = spec
            .examples()
            .iter()
            .filter(|example| example.title.as_deref() == Some(title))
            .collect();
        let [example] = cases.as_slice() else {
            panic!("expected exactly one authored text-tier case: {title}");
        };
        let source = example.chat.as_str();
        let tier_line = source.lines().find(|line| line.starts_with('%')).unwrap();
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 23, &errors)
            .into_option()
            .unwrap();
        assert_eq!(errors.has_errors(), matches!(expected, TextState::Rejected));
        assert!(
            file.to_chat_string().lines().any(|line| line == tier_line),
            "native text recovery changed source: {tier_line:?}"
        );
        let entry = file
            .utterances()
            .next()
            .unwrap()
            .dependent_tiers
            .last()
            .unwrap();
        assert_eq!(entry.kind(), tier_line[1..].split_once(':').unwrap().0);
        match expected {
            TextState::Rejected => {
                let talkbank_model::DependentTier::UnparsedText(raw) = &entry.tier else {
                    panic!("malformed text must retain an explicit rejected state");
                };
                let span = raw.raw().location().span().unwrap();
                assert_eq!(
                    raw.raw().source(),
                    &source[span.start() - 23..span.end() - 23]
                );
            }
            TextState::Parsed => {}
        }
        let wire = serde_json::to_string(&file).unwrap();
        let restored: talkbank_model::ChatFile = serde_json::from_str(&wire).unwrap();
        for retained in [&file, &restored] {
            let validation = ErrorCollector::new();
            retained.validate(&validation, TranscriptName::Anonymous);
            assert_eq!(
                validation.has_errors(),
                matches!(expected, TextState::Rejected),
                "recovery invalidity must survive JSON"
            );
        }
    }
}

#[test]
fn orphan_annotations_cannot_become_freecodes() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E759.md")
        .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    for (index, malformed) in [
        (0, true),
        (1, true),
        (2, true),
        (6, false),
        (7, true),
        (8, false),
    ] {
        let source = spec.examples()[index].chat.as_str();
        let main_line = source.lines().find(|line| line.starts_with('*')).unwrap();
        let findings = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 17, &findings)
            .into_option()
            .unwrap();
        assert_eq!(findings.has_errors(), malformed, "{main_line}");
        let wire = serde_json::to_string(&file).unwrap();
        let restored: talkbank_model::ChatFile = serde_json::from_str(&wire).unwrap();
        for model in [&file, &restored] {
            assert!(
                model.to_chat_string().lines().any(|line| line == main_line),
                "annotation spelling changed: {main_line}"
            );
            let validation = ErrorCollector::new();
            model.validate(&validation, talkbank_model::TranscriptName::Anonymous);
            assert_eq!(
                validation.has_errors(),
                malformed,
                "annotation validity changed: {main_line}"
            );
        }
    }
}

#[test]
fn incomplete_dependent_prefixes_retain_rejection_through_json() {
    use talkbank_model::TranscriptName;
    enum Expected {
        Recovered,
        Parsed,
    }
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E602.md")
        .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    for (index, expected) in [
        (0, Expected::Recovered),
        (1, Expected::Recovered),
        (2, Expected::Parsed),
    ] {
        let authored = spec.examples()[index].chat.as_str();
        for source in [authored.to_owned(), authored.replace('\n', "\r\n")] {
            let tier_line = source.lines().find(|line| line.starts_with('%')).unwrap();
            for origin in [0, 23] {
                let errors = ErrorCollector::new();
                let file = parser
                    .parse_chat_file(&source, origin, &errors)
                    .into_option()
                    .unwrap();
                let entry = &file.utterances().next().unwrap().dependent_tiers[0];
                if let Expected::Recovered = expected {
                    let talkbank_model::DependentTier::UnparsedText(raw) = &entry.tier else {
                        panic!("an incomplete prefix must not supply parsed tier content");
                    };
                    let span = raw.raw().location().span().unwrap();
                    assert_eq!(
                        raw.raw().source(),
                        &source[span.start() - origin..span.end() - origin]
                    );
                    assert_eq!(raw.raw().source().trim_end_matches(['\r', '\n']), tier_line);
                }
                let fragment_errors = ErrorCollector::new();
                let fragment = parser
                    .parse_dependent_tier(tier_line, origin, &fragment_errors)
                    .into_option()
                    .unwrap();
                assert_eq!(
                    fragment.to_chat_string().trim_end_matches(['\r', '\n']),
                    entry.tier.to_chat_string().trim_end_matches(['\r', '\n'])
                );
                let restored: talkbank_model::ChatFile =
                    serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
                for retained in [&file, &restored] {
                    let validation = ErrorCollector::new();
                    retained.validate(&validation, TranscriptName::Anonymous);
                    match expected {
                        Expected::Recovered => {
                            assert!(errors.has_errors());
                            assert!(fragment_errors.has_errors());
                            assert!(
                                validation.has_errors(),
                                "{tier_line:?} became valid model data"
                            );
                            assert!(
                                retained
                                    .to_chat_string()
                                    .lines()
                                    .any(|line| line == tier_line),
                                "recovery repaired the malformed prefix: {}",
                                retained.to_chat_string()
                            );
                        }
                        Expected::Parsed => {
                            assert!(!errors.has_errors());
                            assert!(!fragment_errors.has_errors());
                            assert!(!validation.has_errors());
                        }
                    }
                }
            }
        }
    }
}
