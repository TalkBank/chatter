//! Rejected phonology/sign tiers retain their source slot and invalidity.

use super::completed;

use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{
    ChatParser, DependentTier, ErrorCode, ErrorCollector, ParseHealthTier, ParseOutcome, Severity,
    UnparsedTier, WriteChat,
};
use talkbank_parser_tests::repo_paths::workspace_root;

#[derive(Clone, Copy, Debug)]
enum Family {
    Pho,
    Mod,
    Sin,
}

impl Family {
    fn kind(self) -> &'static str {
        match self {
            Self::Pho => "pho",
            Self::Mod => "mod",
            Self::Sin => "sin",
        }
    }

    fn raw(self, tier: &DependentTier) -> &UnparsedTier {
        match (self, tier) {
            (Self::Pho, DependentTier::UnparsedPho(raw))
            | (Self::Mod, DependentTier::UnparsedMod(raw))
            | (Self::Sin, DependentTier::UnparsedSin(raw)) => raw,
            _ => panic!("expected {self:?} recovery, got {tier:?}"),
        }
    }

    fn health(self) -> ParseHealthTier {
        match self {
            Self::Pho => ParseHealthTier::Pho,
            Self::Mod => ParseHealthTier::Mod,
            Self::Sin => ParseHealthTier::Sin,
        }
    }

    fn cannot_align(self, utterance: &talkbank_model::Utterance) -> bool {
        match self {
            Self::Pho => !utterance.parse_health().can_align_main_to_pho(),
            Self::Mod => !utterance.parse_health().can_align_main_to_mod(),
            Self::Sin => !utterance.parse_health().can_align_main_to_sin(),
        }
    }
}

fn check(
    parser: &impl ChatParser,
    control: &str,
    source: &str,
    family: Family,
    offset: usize,
    ending: &str,
) {
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(control_file) = parser.parse_chat_file(control, offset, &errors)
    else {
        panic!("control rejected");
    };
    assert!(
        !completed(errors)
            .into_diagnostics()
            .iter()
            .any(|error| error.severity == Severity::Error)
    );
    let replacement = control_file
        .utterances()
        .flat_map(|u| u.dependent_tiers.iter())
        .find(|entry| entry.kind() == family.kind())
        .expect("authored control tier")
        .tier
        .clone();
    // A valid control must supply an actual parsed tier, not raw recovery.
    assert!(matches!(
        (&replacement, family),
        (DependentTier::Pho(_), Family::Pho)
            | (DependentTier::Mod(_), Family::Mod)
            | (DependentTier::Sin(_), Family::Sin)
    ));

    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(file) = parser.parse_chat_file(source, offset, &errors) else {
        panic!("recovery file rejected");
    };
    let diagnostics = completed(errors).into_diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|error| error.severity == Severity::Error)
    );
    assert!(
        !diagnostics
            .iter()
            .any(|error| error.code == ErrorCode::UnsupportedDependentTier)
    );
    let utterance = file
        .utterances()
        .find(|u| {
            u.dependent_tiers
                .iter()
                .any(|entry| entry.kind() == family.kind())
        })
        .expect("preserved source slot");
    let entry = utterance
        .dependent_tiers
        .iter()
        .find(|entry| entry.kind() == family.kind())
        .unwrap();
    let raw = family.raw(&entry.tier);
    let span = raw.location().span().expect("located source");
    assert_eq!(
        raw.source(),
        &source[span.start() - offset..span.end() - offset]
    );
    assert_eq!(format!("{}{ending}", raw.to_chat_string()), raw.source());
    assert!(family.cannot_align(utterance));
    assert!(utterance.parse_health().is_tier_tainted(family.health()));

    let encoded = serde_json::to_string(&file).unwrap();
    let decoded: talkbank_model::ChatFile = serde_json::from_str(&encoded).unwrap();
    let decoded_utterance = decoded
        .utterances()
        .find(|u| {
            u.dependent_tiers
                .iter()
                .any(|entry| entry.kind() == family.kind())
        })
        .unwrap();
    let decoded_entry = decoded_utterance
        .dependent_tiers
        .iter()
        .find(|entry| entry.kind() == family.kind())
        .unwrap();
    assert_eq!(family.raw(&decoded_entry.tier).source(), raw.source());
    assert!(family.cannot_align(decoded_utterance));
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

    let mut tiers = utterance.dependent_tiers.clone();
    let count = tiers.len();
    talkbank_transform::dependent_tiers::replace_or_add_tier(&mut tiers, replacement.clone());
    assert_eq!(tiers.len(), count);
    assert_eq!(
        tiers
            .iter()
            .find(|entry| entry.kind() == family.kind())
            .unwrap()
            .tier,
        replacement
    );
    let sanitized = talkbank_transform::redact::sanitize(
        file,
        &talkbank_transform::redact::SanitizationPolicy::strict(),
    )
    .unwrap();
    assert!(
        !sanitized
            .to_chat_string()
            .contains(&format!("%{}:", family.kind()))
    );
}

#[test]
fn both_backends_preserve_rejected_phonology_and_sign_source_through_json() {
    let primary = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    let mut cases = 0;
    for (family, fixture, prefix, group, empty, unclosed) in [
        (
            Family::Pho,
            "tiers/pho-groupings.cha",
            "%pho:",
            "‹a b›",
            "‹›",
            "‹a b",
        ),
        (
            Family::Mod,
            "tiers/pho-groupings.cha",
            "%pho:",
            "‹a b›",
            "‹›",
            "‹a b",
        ),
        (
            Family::Sin,
            "annotation/groups-sign.cha",
            "%sin:",
            "〔c d〕",
            "〔〕",
            "〔c d",
        ),
    ] {
        let source =
            std::fs::read_to_string(workspace_root().join("corpus/reference").join(fixture))
                .unwrap();
        assert_eq!(source.matches(group).count(), 1);
        let control = source.replacen(prefix, &format!("%{}:", family.kind()), 1);
        for damaged in [empty, unclosed] {
            for ending in ["\n", "\r\n", "\r"] {
                let control = control.replace('\n', ending);
                let source = control.replacen(group, damaged, 1);
                for offset in [0, 200] {
                    check(&primary, &control, &source, family, offset, ending);
                    check(&native, &control, &source, family, offset, ending);
                    cases += 2;
                }
            }
        }
    }
    assert_eq!(cases, 72);
}
