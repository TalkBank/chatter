//! grammar.js admits the bare and x-prefixed Phon labels as equivalent tiers.

use talkbank_model::{ChatParser, DependentTier, ErrorCollector, ParseOutcome, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

#[test]
fn longer_phon_labels_do_not_dispatch_as_pho_or_mod() {
    let tree_sitter = TreeSitterParser::new().unwrap();
    let native = Re2cParser::new();
    for (label, body) in [
        ("modsyl", "a:Np:C"),
        ("phosyl", "ɑ:Np:C"),
        ("phoaln", "a↔ɑ,p↔p"),
        ("phoint", "a \u{15}0_100\u{15}"),
    ] {
        for extension in ["", "x"] {
            let source = format!(
                "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tap .\n%{extension}{label}:\t{body}\n@End\n"
            );
            let canonical_errors = ErrorCollector::new();
            let expected = tree_sitter.parse_chat_file_streaming(&source, &canonical_errors);
            assert!(
                canonical_errors.to_vec().is_empty(),
                "{label}: {:?}",
                canonical_errors.to_vec()
            );
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(actual) = native.parse_chat_file(&source, 0, &errors) else {
                panic!("must parse {label}")
            };
            assert!(errors.to_vec().is_empty(), "{label}: {:?}", errors.to_vec());
            for file in [&expected, &actual] {
                let tiers: Vec<_> = file
                    .utterances()
                    .flat_map(|utterance| &utterance.dependent_tiers)
                    .collect();
                assert_eq!(tiers.len(), 1);
                assert!(matches!(
                    (label, &tiers[0].tier),
                    ("modsyl", DependentTier::Modsyl(_))
                        | ("phosyl", DependentTier::Phosyl(_))
                        | ("phoaln", DependentTier::Phoaln(_))
                        | ("phoint", DependentTier::Xphoint(_))
                ));
            }
            assert!(
                expected.semantic_eq(&actual),
                "wrong tier dispatch for %{extension}{label}: expected {expected:?}, got {actual:?}"
            );
        }
    }
}

#[test]
fn observed_user_labels_survive_both_backends() {
    fn check(parser: &impl ChatParser) {
        let source = include_str!("../../../../corpus/reference/tiers/user-defined.cha");
        let errors = ErrorCollector::new();
        let ParseOutcome::Parsed(file) = parser.parse_chat_file(source, 0, &errors) else {
            panic!("reference user-defined tiers must parse");
        };
        assert!(errors.to_vec().is_empty(), "{:?}", errors.to_vec());
        let labels: Vec<_> = file
            .utterances()
            .flat_map(|utterance| &utterance.dependent_tiers)
            .map(|entry| {
                let DependentTier::UserDefined(tier) = &entry.tier else {
                    panic!("expected user-defined tier: {:?}", entry.tier);
                };
                tier.label.as_str()
            })
            .collect();
        assert_eq!(
            labels,
            [
                "xtra",
                "xcod",
                "xpho",
                "xmod",
                "xp",
                "xcod",
                "xs1",
                "xs2",
                "xdb",
                "xabcdef",
                "xabcdefg",
                "xabcdefgh",
                "xcommunicativefunction",
            ]
        );
    }
    check(&TreeSitterParser::new().unwrap());
    check(&Re2cParser::new());
}
