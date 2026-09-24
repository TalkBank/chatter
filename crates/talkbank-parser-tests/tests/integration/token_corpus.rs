//! Public lexical token contracts exercised with canonical full-file specs.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::Header;
use talkbank_parser::{TreeSitterParser, tokens::parse_age_format_token};
use talkbank_parser_tests::{repo_paths::workspace_root, test_error::strict_parse};

#[test]
fn age_token_lexical_acceptance_is_not_chat_age_validity() {
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let parser = TreeSitterParser::new().expect("parser");
    // These lexical age tokens need not satisfy E517's zero-padding and
    // trailing-period rules. Missing semicolon, however, is not a token.
    for (fixture, accepted, violates) in [
        ("E517_1.cha", true, true),
        ("E517_2.cha", true, true),
        ("E517_3.cha", true, true),
        ("E517_4.cha", true, true),
        ("E517_5.cha", false, true),
        ("E517_6.cha", true, false),
        ("E517_7.cha", true, false),
        ("E517_8.cha", false, false),
        ("E517_9.cha", false, true),
        ("E517_10.cha", false, true),
        ("E517_11.cha", false, true),
        ("E517_12.cha", false, true),
        ("E517_13.cha", false, true),
        ("E517_14.cha", false, true),
    ] {
        let source = std::fs::read_to_string(root.join(fixture)).expect("canonical age spec");
        let file =
            strict_parse(parser.parse_chat_file(&source)).expect("ID syntax preserves age field");
        let mut witnessed = 0;
        for header in file.headers() {
            if let Header::ID(id) = header {
                let age = id.age.as_ref().expect("authored age field");
                let token = parse_age_format_token(age.as_str());
                if accepted {
                    assert_eq!(
                        token.as_ref(),
                        Some(age),
                        "token preserves parsed age: {fixture}"
                    );
                } else {
                    assert!(token.is_none(), "non-token rejected: {fixture}");
                }
                witnessed += 1;
            }
        }
        assert_eq!(witnessed, 1);
        let errors = talkbank_model::ErrorCollector::new();
        file.validate(&errors, talkbank_model::model::TranscriptName::Anonymous);
        assert_eq!(
            errors
                .to_vec()
                .iter()
                .any(|error| error.code.as_str() == "E517"),
            violates,
            "canonical validity is independent of lexical token recognition: {fixture}"
        );
    }
}
