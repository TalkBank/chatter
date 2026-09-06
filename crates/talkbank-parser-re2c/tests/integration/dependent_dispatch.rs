//! grammar.js admits the bare and x-prefixed Phon labels as equivalent tiers.

use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

#[test]
fn longer_phon_labels_do_not_dispatch_as_pho_or_mod() {
    let canonical = TreeSitterParser::new().unwrap();
    let oracle = Re2cParser::new();
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
            let expected = canonical.parse_chat_file_streaming(&source, &canonical_errors);
            assert!(
                canonical_errors.to_vec().is_empty(),
                "{label}: {:?}",
                canonical_errors.to_vec()
            );
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(actual) = oracle.parse_chat_file(&source, 0, &errors) else {
                panic!("must parse {label}")
            };
            assert!(errors.to_vec().is_empty(), "{label}: {:?}", errors.to_vec());
            assert!(
                expected.semantic_eq(&actual),
                "wrong tier dispatch for %{extension}{label}: expected {expected:?}, got {actual:?}"
            );
        }
    }
}
