//! Rejected word-timing source stays explicit through file parsing and JSON.
use super::completed;

use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{
    ChatParser, DependentTier, ErrorCode, ErrorCollector, ParseHealthTier, ParseOutcome, Severity,
    WriteChat,
};

fn control(parser: &impl ChatParser, source: &str) -> DependentTier {
    let errors = ErrorCollector::new();
    let file = parser
        .parse_chat_file(source, 0, &errors)
        .into_option()
        .unwrap();
    assert!(
        !completed(errors)
            .into_diagnostics()
            .iter()
            .any(|e| e.severity == Severity::Error)
    );
    let tiers: Vec<_> = file
        .utterances()
        .flat_map(|u| &u.dependent_tiers)
        .filter(|e| e.kind() == "wor")
        .collect();
    assert_eq!(tiers.len(), 3);
    assert!(
        tiers
            .iter()
            .all(|e| matches!(e.tier, DependentTier::Wor(_)))
    );
    tiers.last().unwrap().tier.clone()
}

fn check(
    parser: &impl ChatParser,
    source: &str,
    origin: usize,
    replacement: &DependentTier,
) -> Option<talkbank_model::Span> {
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(file) = parser.parse_chat_file(source, origin, &errors) else {
        panic!("file recovery required");
    };
    let diagnostics = completed(errors).into_diagnostics();
    assert!(diagnostics.iter().any(|e| e.severity == Severity::Error));
    assert!(
        !diagnostics
            .iter()
            .any(|e| e.code == ErrorCode::UnsupportedDependentTier)
    );
    let utterance = file
        .utterances()
        .find(|u| {
            u.dependent_tiers
                .iter()
                .any(|e| matches!(e.tier, DependentTier::UnparsedWor(_)))
        })
        .expect("raw WOR retained");
    assert!(utterance.wor_tier().is_none());
    assert!(
        utterance
            .parse_health()
            .is_tier_tainted(ParseHealthTier::Wor)
    );
    assert!(!utterance.parse_health().can_resolve_wor_timing_sidecar());
    let entry = utterance
        .dependent_tiers
        .iter()
        .find(|e| e.kind() == "wor")
        .unwrap();
    let DependentTier::UnparsedWor(raw) = &entry.tier else {
        panic!("raw variant");
    };
    let span = raw.location().span().unwrap();
    assert_eq!(
        raw.source(),
        &source[span.start() - origin..span.end() - origin]
    );
    assert!(file.to_chat_string().contains(&raw.to_chat_string()));
    let decoded: talkbank_model::ChatFile =
        serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
    let decoded_recovery = decoded
        .utterances()
        .find(|u| {
            u.dependent_tiers
                .iter()
                .any(|e| matches!(e.tier, DependentTier::UnparsedWor(_)))
        })
        .unwrap();
    assert!(decoded_recovery.wor_tier().is_none());
    assert!(
        !decoded_recovery
            .parse_health()
            .can_resolve_wor_timing_sidecar()
    );
    let decoded_tier = decoded
        .utterances()
        .flat_map(|u| &u.dependent_tiers)
        .find_map(|entry| match &entry.tier {
            DependentTier::UnparsedWor(raw) => Some(raw),
            _ => None,
        })
        .unwrap();
    assert_eq!(decoded_tier.source(), raw.source());
    let errors = ErrorCollector::new();
    assert!(
        decoded
            .validate_construction_with_policy(
                ValidationPolicy::new(
                    talkbank_model::RuleSelection::new(),
                    AlignmentValidation::IncludeTierAlignment
                ),
                &errors,
                talkbank_model::TranscriptName::Anonymous,
            )
            .is_err()
    );
    assert!(
        completed(errors)
            .into_diagnostics()
            .iter()
            .any(|e| e.code == ErrorCode::UnparsableContent)
    );
    let mut tiers = utterance.dependent_tiers.clone();
    let count = tiers.len();
    talkbank_transform::dependent_tiers::replace_or_add_tier(&mut tiers, replacement.clone());
    assert_eq!(tiers.len(), count);
    assert_eq!(
        &tiers.iter().find(|e| e.kind() == "wor").unwrap().tier,
        replacement
    );
    let rejected_source = raw.to_chat_string();
    let body = entry.content_span();
    let sanitized = talkbank_transform::redact::sanitize(
        file,
        &talkbank_transform::redact::SanitizationPolicy::strict(),
    )
    .unwrap();
    assert!(!sanitized.to_chat_string().contains(&rejected_source));
    body
}

#[test]
fn both_backends_retain_rejected_wor_source() {
    let reference = std::fs::read_to_string(
        talkbank_parser_tests::repo_paths::workspace_root().join("corpus/reference/tiers/wor.cha"),
    )
    .unwrap();
    let primary = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    let primary_control = control(&primary, &reference);
    let native_control = control(&native, &reference);
    let authored = "%wor:\t[- zho]";
    assert_eq!(reference.matches(authored).count(), 1);
    for damaged in ["%wor:\t[- ]", "%wor:\t[- zho"] {
        for ending in ["\n", "\r\n", "\r"] {
            let source = reference
                .replacen(authored, damaged, 1)
                .replace('\n', ending);
            for origin in [0, 200] {
                check(&primary, &source, origin, &primary_control);
                check(&native, &source, origin, &native_control);
            }
        }
    }
}

#[test]
fn both_backends_reject_unassociated_wor_timing_without_source_loss() {
    #[derive(Clone, Copy)]
    enum Expected {
        Parsed,
        Rejected,
    }
    fn run(
        parser: &impl ChatParser,
        source: &str,
        replacement: &DependentTier,
        expected: Expected,
    ) {
        for ending in ["\n", "\r\n", "\r"] {
            let source = source.replace('\n', ending);
            for origin in [0, 200] {
                let errors = ErrorCollector::new();
                let input = source
                    .split(['\r', '\n'])
                    .find_map(|line| line.strip_prefix("%wor:\t"))
                    .unwrap();
                let tier = parser.parse_wor_tier(input, origin, &errors);
                if let Expected::Rejected = expected {
                    assert!(
                        matches!(tier, ParseOutcome::Rejected),
                        "{} accepted {input:?}",
                        parser.parser_name()
                    );
                    assert!(
                        completed(errors)
                            .into_diagnostics()
                            .iter()
                            .any(|e| e.severity == Severity::Error)
                    );
                    check(parser, &source, origin, replacement);
                } else {
                    let tier = tier.into_option().unwrap();
                    assert!(errors.is_empty());
                    assert_eq!(tier.word_count(), 2);
                    assert_eq!(
                        tier.words().filter(|w| w.inline_bullet.is_some()).count(),
                        1
                    );
                    let errors = ErrorCollector::new();
                    let file = parser
                        .parse_chat_file(&source, origin, &errors)
                        .into_option()
                        .unwrap();
                    assert!(errors.is_empty());
                    assert!(file.utterances().next().unwrap().wor_tier().is_some());
                }
            }
        }
    }
    let root = talkbank_parser_tests::repo_paths::workspace_root();
    let reference = std::fs::read_to_string(root.join("corpus/reference/tiers/wor.cha")).unwrap();
    let primary = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    let replacement = control(&primary, &reference);
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    let association = specs
        .iter()
        .find(|spec| spec.filename == "E316_wor_association.md")
        .unwrap();
    assert_eq!(association.examples().len(), 4);
    {
        for example in association.examples() {
            let expected = match example.title.as_deref() {
                Some("Partial timing control") => Expected::Parsed,
                Some(
                    "Leading bullet has no word"
                    | "Bullet after separator has no word"
                    | "A second bullet cannot replace the first",
                ) => Expected::Rejected,
                title => panic!("unreviewed association example {title:?}"),
            };
            run(&primary, example.chat.as_str(), &replacement, expected);
            run(&native, example.chat.as_str(), &replacement, expected);
            if let Expected::Rejected = expected {
                let admitted = primary
                    .admit_word_timing_plan(
                        example.chat.as_str(),
                        talkbank_model::TranscriptName::Anonymous,
                        |_| talkbank_parser::WordTimingPlan::PreferRetained,
                    )
                    .unwrap();
                let [removed] = admitted.removed_tiers() else {
                    panic!("one rejected WOR receipt")
                };
                assert!(
                    matches!(removed.cause(), talkbank_parser::RemovalCause::OwnLowering { diagnostics } if !diagnostics.is_empty())
                );
                assert!(
                    admitted
                        .document()
                        .utterances()
                        .all(|u| u.wor_tier().is_none())
                );
            }
        }
    }
    let numeric = specs
        .iter()
        .find(|spec| spec.filename == "E360.md")
        .unwrap()
        .examples()
        .iter()
        .find(|example| {
            example.title.as_deref() == Some("WOR overflow retains source without invented timing")
        })
        .unwrap();
    for source in [
        numeric.chat.as_str().to_owned(),
        numeric.chat.as_str().replace(
            "\u{15}18446744073709551616_200\u{15}",
            "\u{15}18446744073709551616_200\u{15} \u{15}200_250\u{15}",
        ),
    ] {
        run(&primary, &source, &replacement, Expected::Rejected);
        run(&native, &source, &replacement, Expected::Rejected);
    }
}

#[test]
fn wor_annotations_are_rejected_without_erasure() {
    let root = talkbank_parser_tests::repo_paths::workspace_root();
    let reference = std::fs::read_to_string(root.join("corpus/reference/tiers/wor.cha")).unwrap();
    let primary = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    let replacement = control(&primary, &reference);
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    let annotations = specs
        .iter()
        .find(|spec| spec.filename == "E316_wor_annotations.md")
        .unwrap();
    #[derive(Clone, Copy)]
    enum Expected {
        StandaloneWords,
        RawRecovery,
    }
    fn run(
        parser: &impl ChatParser,
        authored: &str,
        replacement: &DependentTier,
        expected: Expected,
    ) {
        for ending in ["\n", "\r\n", "\r"] {
            let source = authored.replace('\n', ending);
            for origin in [0, 200] {
                let input = source
                    .split(['\r', '\n'])
                    .find_map(|line| line.strip_prefix("%wor:\t"))
                    .unwrap();
                let errors = ErrorCollector::new();
                let parsed = parser.parse_wor_tier(input, origin, &errors);
                match expected {
                    Expected::StandaloneWords => {
                        let tier = parsed
                            .into_option()
                            .expect("word internals remain supported");
                        assert!(errors.is_empty(), "{}", parser.parser_name());
                        assert_eq!(
                            tier.to_chat_string(),
                            format!("%wor:\t{input}"),
                            "{}",
                            parser.parser_name()
                        );
                        assert_eq!(tier.word_count(), 4);
                    }
                    Expected::RawRecovery => {
                        assert!(
                            matches!(parsed, ParseOutcome::Rejected),
                            "{} accepted {input:?}",
                            parser.parser_name()
                        );
                        assert!(
                            completed(errors)
                                .into_diagnostics()
                                .iter()
                                .any(|e| e.severity == Severity::Error)
                        );
                        let body = check(parser, &source, origin, replacement)
                            .expect("rejected WOR preserves its known body geometry");
                        assert_eq!(
                            &source[body.start() - origin..body.end() - origin],
                            input,
                            "{} / {ending:?} / {origin}",
                            parser.parser_name()
                        );
                    }
                }
            }
        }
    }
    for example in annotations.examples() {
        let expected = match example.title.as_deref() {
            Some("Standalone word markers control") => Expected::StandaloneWords,
            Some(
                "Word error annotation is not timing content"
                | "Word explanation is not timing content"
                | "Word replacement is not timing content"
                | "Word retrace is not timing content"
                | "Word correction is not timing content"
                | "Action explanation is not timing content"
                | "Bare action is not a display word",
            ) => Expected::RawRecovery,
            other => panic!("unreviewed WOR annotation example {other:?}"),
        };
        run(&primary, example.chat.as_str(), &replacement, expected);
        run(&native, example.chat.as_str(), &replacement, expected);
    }
}
