//! Malformed Phon tiers retain source and identity without parsed alignment data.

use super::completed;

use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{ChatParser, DependentTier, ErrorCode, ErrorCollector, WriteChat};
use talkbank_parser_tests::{error_specs, repo_paths};

#[test]
fn both_backends_preserve_unparsed_phon_tiers_across_wire_and_origins() {
    let primary = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    let specs = error_specs::load(repo_paths::workspace_root()).unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E601.md")
        .unwrap();
    let mut cases = 0;
    for example in spec.examples().iter().filter(|example| {
        example.chat.as_str().contains("%xphoaln:") || example.chat.as_str().contains("%xphoint:")
    }) {
        for ending in ["\n", "\r\n", "\r"] {
            let source = example.chat.as_str().replace('\n', ending);
            for offset in [0, 200] {
                for file in [
                    parse(&primary, &source, offset),
                    parse(&native, &source, offset),
                ] {
                    let utterance = file.utterances().next().unwrap();
                    let entry = utterance.dependent_tiers.last().unwrap();
                    let raw = match &entry.tier {
                        DependentTier::UnparsedPhoaln(raw) => {
                            assert_eq!(entry.kind(), "phoaln");
                            assert!(utterance.phoaln_tier().is_none());
                            assert!(!utterance.parse_health().can_align_phoaln());
                            raw
                        }
                        DependentTier::UnparsedXphoint(raw) => {
                            assert_eq!(entry.kind(), "xphoint");
                            assert!(utterance.xphoint_tier().is_none());
                            assert!(!utterance.parse_health().can_align_xphoint_to_pho());
                            raw
                        }
                        other => panic!("expected explicit raw recovery: {other:?}"),
                    };
                    let span = raw.location().span().unwrap();
                    assert_eq!(
                        raw.source(),
                        &source[span.start() - offset..span.end() - offset]
                    );
                    assert_eq!(format!("{}{ending}", raw.to_chat_string()), raw.source());
                    assert!(raw.source().starts_with("%xpho"));
                    let encoded = serde_json::to_string(&file).unwrap();
                    let decoded: talkbank_model::ChatFile = serde_json::from_str(&encoded).unwrap();
                    let decoded_entry = &decoded
                        .utterances()
                        .next()
                        .unwrap()
                        .dependent_tiers
                        .last()
                        .unwrap()
                        .tier;
                    let decoded_raw = match decoded_entry {
                        DependentTier::UnparsedPhoaln(raw)
                        | DependentTier::UnparsedXphoint(raw) => raw,
                        other => panic!("wire lost recovery: {other:?}"),
                    };
                    assert_eq!(decoded_raw.source(), raw.source());
                    assert!(decoded_raw.location().span().is_none());
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
                            .any(|error| error.code == ErrorCode::UnparsableContent)
                    );
                    let sanitized = talkbank_transform::redact::sanitize(
                        file,
                        &talkbank_transform::redact::SanitizationPolicy::strict(),
                    )
                    .unwrap();
                    assert!(!sanitized.to_chat_string().contains("%xpho"));
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
        .expect("recovery model");
    let diagnostics = completed(errors).into_diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|error| error.code == ErrorCode::InvalidDependentTier)
    );
    assert!(
        !diagnostics
            .iter()
            .any(|error| error.code == ErrorCode::UnsupportedDependentTier)
    );
    file
}
