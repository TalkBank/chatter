//! Canonical grouped alignment policy and user-facing mismatch presentation.

use talkbank_model::alignment::helpers::{PositionalDomain, collect_tier_items};
use talkbank_model::model::{TranscriptName, WriteChat};
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;

/// Position labels are a presentation contract, not another alignment counter.
/// The domain type and its existing traversal retain ownership of inclusion.
#[test]
fn grouped_specs_retain_atomic_positions_in_mismatch_diagnostics() {
    let parser = TreeSitterParser::new().expect("parser");
    let phonetic = [
        ("non", None),
        ("‹il y a›", Some("phonological group")),
        ("(.)", Some("pause")),
        ("pas", None),
    ];
    let sign = [
        ("0", Some("action")),
        ("foo", None),
        ("0 [=! nods]", Some("action")),
        ("〔bar baz〕", Some("sign group")),
    ];
    for (spec, domain, code, tier, expected) in [
        (
            "E714",
            PositionalDomain::Pho,
            ErrorCode::PhoCountMismatchTooFew,
            "%pho",
            phonetic.as_slice(),
        ),
        (
            "E718",
            PositionalDomain::Sin,
            ErrorCode::SinCountMismatchTooFew,
            "%sin",
            sign.as_slice(),
        ),
    ] {
        for example in [3, 4] {
            let source = std::fs::read_to_string(workspace_root().join(format!(
                "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{spec}_{example}.cha",
            ))).expect("canonical grouped alignment spec");
            let mut file =
                strict_parse(parser.parse_chat_file(&source)).expect("clean grouped syntax");
            let utterance = file.utterances().next().expect("spec utterance");
            let positions = collect_tier_items(&utterance.main.content.content, domain);
            let rendered: Vec<_> = positions
                .iter()
                .map(|item| (item.text.as_str(), item.description.as_deref()))
                .collect();
            assert_eq!(
                rendered, expected,
                "{spec}_{example}: authored domain presentation"
            );
            let errors = ErrorCollector::new();
            file.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let findings = errors.into_vec();
            if example == 3 {
                assert!(findings.is_empty(), "{spec}: valid control: {findings:?}");
            } else {
                assert_eq!(
                    findings.len(),
                    1,
                    "{spec}: one-token deletion: {findings:?}"
                );
                assert_eq!(findings[0].code, code);
                let message = &findings[0].message;
                assert!(
                    message.starts_with(&format!(
                        "Main tier has 4 alignable items, but {tier} tier has 3 items",
                    )),
                    "{message}"
                );
                for (text, _) in expected {
                    assert!(message.contains(text), "{text}: {message}");
                }
                assert!(
                    message.contains('⊖'),
                    "missing target remains visible: {message}"
                );
            }
            assert_eq!(
                file.to_chat_string(),
                source,
                "diagnostics must not rewrite source"
            );
        }
    }
}
