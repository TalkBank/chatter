//! Timing projections are all-or-nothing over the complete authored tier.

use talkbank_model::model::dependent_tier::TimSegment;
use talkbank_model::model::{DependentTier, TranscriptName, ValidationTagged};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};

#[test]
fn range_endpoint_rejection_preserves_text_without_partial_segments() {
    enum Expected {
        Complete,
        Unsupported,
    }
    fn check(parser: &impl ChatParser, source: &str, expected: &Expected) {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 19, &errors)
            .into_option()
            .unwrap();
        assert!(errors.is_empty());
        let tiers: Vec<_> = file
            .utterances()
            .flat_map(|utterance| &utterance.dependent_tiers)
            .collect();
        let [entry] = tiers.as_slice() else {
            panic!("expected one timing tier");
        };
        let DependentTier::Tim(tier) = &entry.tier else {
            panic!("expected timing model");
        };
        let raw = source
            .lines()
            .find_map(|line| line.strip_prefix("%tim:\t"))
            .unwrap();
        assert_eq!(tier.as_str(), raw);
        let decoded = serde_json::from_value::<talkbank_model::model::dependent_tier::TimTier>(
            serde_json::to_value(tier).unwrap(),
        )
        .unwrap();
        assert_eq!(decoded.as_str(), raw);
        assert_eq!(decoded.segments(), tier.segments());
        for value in [tier, &decoded] {
            match expected {
                Expected::Complete => {
                    assert!(!value.has_validation_issue());
                    let [TimSegment::Single(single), TimSegment::Range { start, end }] =
                        value.segments()
                    else {
                        panic!("expected complete two-segment projection");
                    };
                    assert_eq!((single.hours, single.minutes, single.seconds), (0, 7, 55));
                    assert_eq!((start.hours, start.minutes, start.seconds), (0, 1, 30));
                    assert_eq!((end.hours, end.minutes, end.seconds), (0, 2, 0));
                }
                Expected::Unsupported => {
                    assert!(value.has_validation_issue());
                    assert!(value.segments().is_empty(), "no valid prefix may escape");
                }
            }
        }
        let findings = ErrorCollector::new();
        file.validate(&findings, TranscriptName::Anonymous);
        let codes: Vec<_> = findings
            .into_vec()
            .into_iter()
            .map(|finding| finding.code)
            .collect();
        match expected {
            Expected::Complete => assert!(codes.is_empty(), "{codes:?}"),
            Expected::Unsupported => assert_eq!(codes, [ErrorCode::InvalidTimTierFormat]),
        }
    }
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E603.md")
        .unwrap();
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    for (title, expected) in [
        ("A time followed by a complete range", Expected::Complete),
        (
            "An invalid range start rejects the entire timing projection",
            Expected::Unsupported,
        ),
        (
            "An invalid range end rejects the entire timing projection",
            Expected::Unsupported,
        ),
    ] {
        let example = spec
            .examples()
            .iter()
            .find(|example| example.title.as_deref() == Some(title))
            .unwrap();
        check(&tree, example.chat.as_str(), &expected);
        check(&native, example.chat.as_str(), &expected);
    }
}
