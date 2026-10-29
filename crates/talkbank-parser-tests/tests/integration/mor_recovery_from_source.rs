//! Shared malformed morphology remains source data, never parsed morphology.

use super::completed;

use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{
    ChatParser, DependentTier, ErrorCode, ErrorCollector, RuleSelection, TranscriptName, WriteChat,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;
use talkbank_parser_tests::test_error::ParseProductTestExt;
use talkbank_parser_tests::{error_specs, repo_paths};

#[test]
fn both_backends_preserve_malformed_mor_source_and_wire_invalidity() {
    let primary = TreeSitterParser::new().unwrap();
    let native = Re2cParser::new();
    let specs = error_specs::load(repo_paths::workspace_root()).unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E760.md")
        .unwrap();
    let mut cases = 0;
    for example in spec.examples().iter().filter(|example| {
        example.chat.as_str().contains("\t|go") || example.chat.as_str().contains("\t|we")
    }) {
        for ending in ["\n", "\r\n", "\r"] {
            let source = example.chat.as_str().replace('\n', ending);
            for offset in [0, 200] {
                for file in [
                    parse(&primary, &source, offset),
                    parse(&native, &source, offset),
                ] {
                    let utterance = file.utterances().next().unwrap();
                    assert!(utterance.mor_tier().is_none());
                    let entry = &utterance.dependent_tiers[0];
                    assert_eq!(entry.kind(), "mor");
                    let DependentTier::UnparsedMor(raw) = &entry.tier else {
                        panic!("explicit recovery required");
                    };
                    let span = raw.location().span().unwrap();
                    assert_eq!(
                        raw.source(),
                        &source[span.start() - offset..span.end() - offset]
                    );
                    assert!(raw.source().starts_with("%mor:"));
                    assert!(raw.source().contains('|'));
                    assert!(!utterance.parse_health().can_align_main_to_mor());
                    let rendered = raw.to_chat_string();
                    assert_eq!(format!("{rendered}{ending}"), raw.source());
                    let encoded = serde_json::to_string(&file).unwrap();
                    let decoded: talkbank_model::ChatFile = serde_json::from_str(&encoded).unwrap();
                    let decoded_raw = &decoded.utterances().next().unwrap().dependent_tiers[0].tier;
                    let DependentTier::UnparsedMor(decoded_raw) = decoded_raw else {
                        panic!("wire must retain recovery");
                    };
                    assert_eq!(decoded_raw.source(), raw.source());
                    assert!(decoded_raw.location().span().is_none());
                    let errors = ErrorCollector::new();
                    let result = decoded.validate_construction_with_policy(
                        ValidationPolicy::new(
                            RuleSelection::new(),
                            AlignmentValidation::IncludeTierAlignment,
                        ),
                        &errors,
                        TranscriptName::Anonymous,
                    );
                    assert!(result.is_err(), "JSON cannot certify raw morphology");
                    assert!(
                        completed(errors)
                            .into_diagnostics()
                            .iter()
                            .any(|error| error.code == ErrorCode::UnparsableContent)
                    );
                    assert!(matches!(
                        talkbank_transform::redact::sanitize(
                            file,
                            &talkbank_transform::redact::SanitizationPolicy::strict()
                        ),
                        Err(talkbank_transform::redact::RedactError::UnparsedMorphology)
                    ));
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 24);
}

fn parse(parser: &impl ChatParser, source: &str, offset: usize) -> talkbank_model::ChatFile {
    let errors = ErrorCollector::new();
    let file = parser
        .parse_chat_file(source, offset, &errors)
        .into_option()
        .expect("recovered file");
    assert!(
        completed(errors)
            .diagnostics()
            .iter()
            .any(|error| error.severity == talkbank_model::Severity::Error)
    );
    file
}

#[test]
fn recovered_slot_is_replaced_in_place_without_missing_mor_cascade() {
    let parser = TreeSitterParser::new().unwrap();
    let specs = error_specs::load(repo_paths::workspace_root()).unwrap();
    let mor_spec = specs
        .iter()
        .find(|spec| spec.filename == "E760.md")
        .unwrap();
    let original = mor_spec.examples()[0].chat.as_str();
    let good = mor_spec.examples()[2].chat.as_str();
    let replacement = parser
        .parse_chat_file(good)
        .expect_built()
        .utterances()
        .next()
        .unwrap()
        .mor_tier()
        .unwrap()
        .clone();
    let gra_spec = specs
        .iter()
        .find(|spec| spec.filename == "E604_gra_without_mor.md")
        .unwrap();
    let gra_file = gra_spec
        .examples()
        .iter()
        .find_map(|example| {
            let file = parser.parse_chat_file(example.chat.as_str()).expect_built();
            file.utterances().next().and_then(|u| u.gra_tier().cloned())
        })
        .unwrap();
    for mut file in [
        parse(&parser, original, 0),
        parse(&Re2cParser::new(), original, 0),
    ] {
        let utterance = file
            .lines
            .as_mut_slice()
            .iter_mut()
            .find_map(|line| match line {
                talkbank_model::Line::Utterance(u) => Some(&mut **u),
                _ => None,
            })
            .unwrap();
        utterance
            .dependent_tiers
            .push(DependentTier::Gra(gra_file.clone()).into());
        let errors = ErrorCollector::new();
        let _ = file.clone().validate_with_policy(
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            &errors,
            TranscriptName::Anonymous,
        );
        let diagnostics = completed(errors).into_diagnostics();
        assert!(
            diagnostics
                .iter()
                .any(|error| error.code == ErrorCode::UnparsableContent)
        );
        assert!(
            !diagnostics
                .iter()
                .any(|error| error.code == ErrorCode::GraWithoutMor)
        );
        let utterance = file
            .lines
            .as_mut_slice()
            .iter_mut()
            .find_map(|line| match line {
                talkbank_model::Line::Utterance(u) => Some(&mut **u),
                _ => None,
            })
            .unwrap();
        let separator = utterance.dependent_tiers[0].separator;
        talkbank_transform::dependent_tiers::replace_or_add_tier(
            &mut utterance.dependent_tiers,
            DependentTier::Mor(replacement.clone()),
        );
        assert_eq!(utterance.dependent_tiers.len(), 2);
        assert!(matches!(
            utterance.dependent_tiers[0].tier,
            DependentTier::Mor(_)
        ));
        assert!(matches!(
            utterance.dependent_tiers[1].tier,
            DependentTier::Gra(_)
        ));
        assert_eq!(utterance.dependent_tiers[0].separator, separator);
    }
}

#[test]
fn file_level_mor_recovery_retains_source_and_refuses_sanitization() {
    let primary = TreeSitterParser::new().unwrap();
    let native = Re2cParser::new();
    let original = include_str!("../error_corpus/validation_errors/E601_1.cha");
    for ending in ["\n", "\r\n", "\r"] {
        let source = original.replace('\n', ending);
        for offset in [0, 200] {
            for file in [
                parse(&primary, &source, offset),
                parse(&native, &source, offset),
            ] {
                let utterance = file.utterances().next().unwrap();
                assert!(utterance.mor_tier().is_none());
                let DependentTier::UnparsedMor(raw) = &utterance.dependent_tiers[0].tier else {
                    panic!("file-level rejection must retain MOR identity");
                };
                assert_eq!(raw.to_chat_string(), "%mor:\t|||");
                let span = raw.location().span().unwrap();
                assert_eq!(
                    raw.source(),
                    &source[span.start() - offset..span.end() - offset]
                );
                assert!(!utterance.parse_health().can_align_main_to_mor());
                let decoded: talkbank_model::ChatFile =
                    serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
                for model in [file, decoded] {
                    let diagnostics = ErrorCollector::new();
                    model.validate(&diagnostics, TranscriptName::Anonymous);
                    assert!(
                        completed(diagnostics)
                            .into_diagnostics()
                            .iter()
                            .any(|error| error.code == ErrorCode::UnparsableContent)
                    );
                    // All document content survives; source line endings render
                    // canonically as LF, including after JSON import.
                    assert_eq!(model.to_chat_string(), original);
                    assert!(matches!(
                        talkbank_transform::redact::sanitize(
                            model,
                            &talkbank_transform::redact::SanitizationPolicy::strict()
                        ),
                        Err(talkbank_transform::redact::RedactError::UnparsedMorphology)
                    ));
                }
            }
        }
    }
}

#[test]
fn file_level_recovery_preserves_mor_and_gra_continuations() {
    use talkbank_transform::redact::{RedactError, SanitizationPolicy, sanitize};
    #[derive(Clone, Copy)]
    enum RejectedKind {
        Mor,
        Gra,
    }
    let primary = TreeSitterParser::new().unwrap();
    let native = Re2cParser::new();
    let original = include_str!("../error_corpus/validation_errors/E601_1.cha");
    for kind in [RejectedKind::Mor, RejectedKind::Gra] {
        let tier = match kind {
            RejectedKind::Mor => "%mor:\t|||",
            RejectedKind::Gra => "%gra:\t|||",
        };
        for body in [tier.to_owned(), format!("{tier}\n\t|||")] {
            for ending in ["\n", "\r\n", "\r"] {
                let source = original.replace("%mor:\t|||", &body).replace('\n', ending);
                let expected = body.replace('\n', ending);
                for offset in [0, 200] {
                    for file in [
                        parse(&primary, &source, offset),
                        parse(&native, &source, offset),
                    ] {
                        let utterance = file.utterances().next().unwrap();
                        assert!(utterance.mor_tier().is_none());
                        assert!(utterance.gra_tier().is_none());
                        assert_eq!(utterance.dependent_tiers.len(), 1);
                        let raw = match (kind, &utterance.dependent_tiers[0].tier) {
                            (RejectedKind::Mor, DependentTier::UnparsedMor(raw)) => raw,
                            (RejectedKind::Gra, DependentTier::UnparsedGra(raw)) => raw,
                            _ => panic!("wrong recovery identity"),
                        };
                        assert_eq!(raw.source().trim_end_matches(['\r', '\n']), expected);
                        let span = raw.location().span().unwrap();
                        assert_eq!(
                            raw.source(),
                            &source[span.start() - offset..span.end() - offset]
                        );
                        let decoded: talkbank_model::ChatFile =
                            serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
                        for model in [file, decoded] {
                            let diagnostics = ErrorCollector::new();
                            model.validate(&diagnostics, TranscriptName::Anonymous);
                            assert!(
                                completed(diagnostics)
                                    .into_diagnostics()
                                    .iter()
                                    .any(|error| error.code == ErrorCode::UnparsableContent)
                            );
                            assert!(model.to_chat_string().contains(&expected));
                            match (kind, sanitize(model, &SanitizationPolicy::strict())) {
                                (RejectedKind::Mor, Err(RedactError::UnparsedMorphology))
                                | (RejectedKind::Gra, Err(RedactError::UnparsedGrammar)) => {}
                                (_, Err(error)) => panic!("wrong sanitizer refusal: {error}"),
                                (_, Ok(_)) => panic!("recovery must refuse sanitization"),
                            }
                        }
                    }
                }
            }
        }
    }
}
