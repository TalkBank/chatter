//! Limited compatibility checks are not full document-validity proofs.

use talkbank_model::model::{ChatFile, SemanticEq, TranscriptName};
use talkbank_model::{ErrorCollector, ParseError};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_transform::validate::{ValidityLevel, validate_output, validate_to_level};

/// Keep the model and diagnostics from one parse together at this test boundary.
struct ParsedSpec {
    file: ChatFile,
    diagnostics: Vec<ParseError>,
}

impl ParsedSpec {
    fn load(parser: &TreeSitterParser, fixture: &str) -> Self {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{fixture}.cha",
        )))
        .expect("canonical validation specimen");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        Self {
            file,
            diagnostics: errors.into_vec(),
        }
    }
}

#[test]
fn spec_legacy_admission_preserves_parse_provenance_and_structural_policy() {
    let parser = TreeSitterParser::new().expect("parser");
    for fixture in ["E504_3", "E305_2", "E305_3"] {
        let parsed = ParsedSpec::load(&parser, fixture);
        assert!(parsed.diagnostics.is_empty(), "{fixture}");
        for level in [
            ValidityLevel::Parseable,
            ValidityLevel::StructurallyComplete,
            ValidityLevel::MainTierValid,
        ] {
            assert!(
                validate_to_level(&parsed.file, &parsed.diagnostics, level).is_ok(),
                "{fixture}, {level:?}"
            );
        }
    }
    for (fixture, message) in [
        ("E504_1", "@Participants header missing"),
        ("E504_2", "@Languages header missing"),
        ("E522_1", "Speaker *MOT not declared"),
        ("E305_1", "has no terminator"),
    ] {
        let parsed = ParsedSpec::load(&parser, fixture);
        let original = parsed.file.clone();
        for level in [
            ValidityLevel::StructurallyComplete,
            ValidityLevel::MainTierValid,
        ] {
            let errors = validate_to_level(&parsed.file, &parsed.diagnostics, level)
                .expect_err("structural refusal");
            assert!(
                errors
                    .iter()
                    .any(|error| error.level == ValidityLevel::StructurallyComplete
                        && error.message.contains(message)),
                "{fixture}: {errors:?}"
            );
        }
        assert!(original.semantic_eq(&parsed.file));
    }
    let malformed = ParsedSpec::load(&parser, "E702_1");
    let first = malformed
        .diagnostics
        .first()
        .expect("parse refusal witness");
    let errors = validate_to_level(
        &malformed.file,
        &malformed.diagnostics,
        ValidityLevel::Parseable,
    )
    .expect_err("parse errors cannot be hidden");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].level, ValidityLevel::Parseable);
    assert!(errors[0].message.contains(first.code.as_str()));
    assert!(errors[0].message.contains(&first.message));
    let empty = ParsedSpec::load(&parser, "E306_1");
    // Recovery is refused before legacy main-tier checks. This specimen does
    // not witness the literal-empty-content branch in those later checks.
    let first = empty
        .diagnostics
        .first()
        .expect("empty-tier recovery witness");
    let errors = validate_to_level(
        &empty.file,
        &empty.diagnostics,
        ValidityLevel::MainTierValid,
    )
    .expect_err("empty speech refusal");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].level, ValidityLevel::Parseable);
    assert!(errors[0].message.contains(first.code.as_str()));
    assert!(errors[0].message.contains(&first.message));
}

#[test]
fn spec_legacy_output_checks_preserve_morphology_timing_and_ca_boundaries() {
    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, command, expected) in [
        (
            "E705_2",
            "morphotag",
            Some("has 3 words but %mor has 2 items"),
        ),
        ("E362_1", "align", Some("backwards timing")),
        ("E362_3", "align", None),
        ("E305_1", "align", Some("lost its terminator")),
        ("E305_2", "align", None),
        ("E305_3", "align", None),
    ] {
        let parsed = ParsedSpec::load(&parser, fixture);
        assert!(parsed.diagnostics.is_empty(), "{fixture}");
        let original = parsed.file.clone();
        let result = validate_output(&parsed.file, command);
        match expected {
            Some(message) => assert!(
                result
                    .expect_err("output refusal")
                    .iter()
                    .any(|error| error.message.contains(message)),
                "{fixture}"
            ),
            None => assert!(result.is_ok(), "{fixture}: {result:?}"),
        }
        assert!(original.semantic_eq(&parsed.file));
    }
    let source =
        std::fs::read_to_string(workspace_root().join("corpus/reference/tiers/mor-gra.cha"))
            .expect("canonical morphology control");
    let file = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(&source))
        .expect("reference parses cleanly");
    assert!(
        validate_output(&file, "morphotag").is_ok(),
        "aligned morphology control"
    );
}

#[test]
fn spec_legacy_output_success_is_not_document_admission() {
    let parser = TreeSitterParser::new().expect("parser");
    // Each pair is an authored invalid specimen and its valid control.
    // The legacy operation checks neither word syntax nor turn ordering:
    // only the consuming admission API may issue a ValidChatFile proof.
    for (fixture, expected_refusal) in [
        ("E243_standalone_slash_2", Some("E243")),
        ("E243_standalone_slash_1", None),
        ("E362_2", Some("E362")),
        ("E362_3", None),
    ] {
        let parsed = ParsedSpec::load(&parser, fixture);
        assert!(
            parsed.diagnostics.is_empty(),
            "{fixture}: clean syntax required"
        );
        for command in ["align", "morphotag", "other"] {
            assert!(
                validate_output(&parsed.file, command).is_ok(),
                "{fixture}: {command} is only a limited compatibility check",
            );
        }
        let original = parsed.file.clone();
        let errors = ErrorCollector::new();
        let admitted = parsed
            .file
            .validate_into(&errors, TranscriptName::Anonymous);
        match expected_refusal {
            Some(code) => {
                let refused = admitted.expect_err("limited output success cannot confer validity");
                assert!(
                    refused
                        .diagnostics()
                        .iter()
                        .any(|error| error.code.as_str() == code),
                    "{fixture}: authored refusal {code}: {:?}",
                    refused.diagnostics(),
                );
                assert!(original.semantic_eq(refused.document()));
            }
            None => {
                let valid = admitted.expect("authored valid control must earn a proof");
                assert!(original.semantic_eq(valid.document()));
            }
        }
    }
}
