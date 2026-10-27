//! Rejected file GRA retains source rather than exposing a fabricated graph.

use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{ChatParser, DependentTier, ErrorCode, ErrorCollector, WriteChat};
use talkbank_parser_tests::test_error::ParseProductTestExt;
use talkbank_parser_tests::{error_specs, repo_paths};

#[test]
fn both_backends_preserve_rejected_gra_source_and_wire_invalidity() {
    let primary = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    let specs = error_specs::load(repo_paths::workspace_root()).unwrap();
    let control = primary
        .parse_chat_file(include_str!(
            "../error_corpus/validation_errors/E604_gra_without_mor_1.cha"
        ))
        .expect_built();
    let replacement = control
        .utterances()
        .next()
        .unwrap()
        .gra_tier()
        .unwrap()
        .clone();
    let mut cases = 0;
    for spec in specs
        .iter()
        .filter(|spec| matches!(spec.filename.as_str(), "E708.md" | "E709.md" | "E710.md"))
    {
        for example in spec.examples() {
            for ending in ["\n", "\r\n", "\r"] {
                let source = example.chat.as_str().replace('\n', ending);
                for offset in [0, 200] {
                    for file in [
                        parse(&primary, &source, offset),
                        parse(&native, &source, offset),
                    ] {
                        let utterance = file.utterances().next().unwrap();
                        assert!(utterance.gra_tier().is_none());
                        assert!(!utterance.parse_health().can_align_mor_to_gra());
                        let entry = utterance
                            .dependent_tiers
                            .iter()
                            .find(|entry| entry.kind() == "gra")
                            .expect("original GRA slot");
                        let DependentTier::UnparsedGra(raw) = &entry.tier else {
                            panic!("recovery must retain raw source: {entry:?}");
                        };
                        let span = raw.location().span().unwrap();
                        assert_eq!(
                            raw.source(),
                            &source[span.start() - offset..span.end() - offset]
                        );
                        let body = raw
                            .source()
                            .strip_prefix("%gra:\t")
                            .expect("authored GRA fixture prefix");
                        let body_start = span.start() + "%gra:\t".len();
                        reject_fragment(&primary, body, body_start);
                        reject_fragment(&native, body, body_start);
                        assert_eq!(format!("{}{ending}", raw.to_chat_string()), raw.source());
                        assert!(file.to_chat_string().contains(&raw.to_chat_string()));
                        let mut replaced = utterance.dependent_tiers.clone();
                        let position = replaced
                            .iter()
                            .position(|entry| entry.kind() == "gra")
                            .unwrap();
                        talkbank_transform::dependent_tiers::replace_or_add_tier(
                            &mut replaced,
                            DependentTier::Gra(replacement.clone()),
                        );
                        assert_eq!(replaced.len(), utterance.dependent_tiers.len());
                        assert!(matches!(replaced[position].tier, DependentTier::Gra(_)));
                        assert_eq!(replaced[position].separator, entry.separator);
                        for (index, unchanged) in utterance.dependent_tiers.iter().enumerate() {
                            if index != position {
                                assert_eq!(&replaced[index], unchanged);
                            }
                        }
                        let encoded = serde_json::to_string(&file).unwrap();
                        let decoded: talkbank_model::ChatFile =
                            serde_json::from_str(&encoded).unwrap();
                        let decoded_entry = decoded
                            .utterances()
                            .next()
                            .unwrap()
                            .dependent_tiers
                            .iter()
                            .find(|entry| entry.kind() == "gra")
                            .unwrap();
                        let DependentTier::UnparsedGra(decoded_raw) = &decoded_entry.tier else {
                            panic!("JSON must preserve recovery");
                        };
                        assert_eq!(decoded_raw.source(), raw.source());
                        assert!(decoded_raw.location().span().is_none());
                        let findings = ErrorCollector::new();
                        assert!(
                            decoded
                                .validate_construction_with_policy(
                                    ValidationPolicy::new(
                                        talkbank_model::RuleSelection::new(),
                                        AlignmentValidation::IncludeTierAlignment
                                    ),
                                    &findings,
                                    talkbank_model::TranscriptName::Anonymous,
                                )
                                .is_err()
                        );
                        let findings = findings.into_vec();
                        assert!(
                            findings
                                .iter()
                                .any(|error| error.code == ErrorCode::UnparsableContent)
                        );
                        assert_eq!(
                            findings
                                .iter()
                                .any(|error| error.code == ErrorCode::GraWithoutMor),
                            utterance.mor_tier().is_none()
                        );
                        assert!(matches!(
                            talkbank_transform::redact::sanitize(
                                file,
                                &talkbank_transform::redact::SanitizationPolicy::strict(),
                            ),
                            Err(talkbank_transform::redact::RedactError::UnparsedGrammar)
                        ));
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 60);
}

fn parse(parser: &impl ChatParser, source: &str, offset: usize) -> talkbank_model::ChatFile {
    let errors = ErrorCollector::new();
    let file = parser
        .parse_chat_file(source, offset, &errors)
        .into_option()
        .expect("recovery model");
    assert!(errors.has_errors(), "rejection requires a diagnostic");
    file
}

fn reject_fragment(parser: &impl ChatParser, body: &str, offset: usize) {
    let errors = ErrorCollector::new();
    assert!(matches!(
        parser.parse_gra_tier(body, offset, &errors),
        talkbank_model::ParseOutcome::Rejected
    ));
    let diagnostics = errors.into_vec();
    assert!(
        !diagnostics.is_empty(),
        "fragment rejection must be diagnosed"
    );
    for diagnostic in diagnostics {
        let span = diagnostic.location.span().expect("actual fragment source");
        assert!(
            span.start() >= offset && span.end() <= offset + body.len(),
            "{diagnostic:?}"
        );
    }
}
