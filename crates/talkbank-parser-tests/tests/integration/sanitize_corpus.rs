//! Sanitizer wire contracts over the canonical reference population.
#![allow(clippy::expect_used)]

use talkbank_model::model::{Header, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};
use talkbank_transform::redact::{SanitizationPolicy, sanitize};

fn lexical_runs(parser: &impl talkbank_model::ChatParser) {
    let source =
        include_str!("../../../../corpus/reference/word-features/registry-lexical-boundaries.cha");
    let errors = talkbank_model::ErrorCollector::new();
    let original = parser
        .parse_chat_file(source, 0, &errors)
        .into_option()
        .unwrap();
    assert!(errors.into_vec().is_empty());
    let policy = SanitizationPolicy::strict();
    let output = sanitize(original, &policy).unwrap().to_chat_string();
    assert!(
        output.contains("*CHI:\tworda wordb .\n"),
        "one placeholder per lexical run: {output}"
    );
    let errors = talkbank_model::ErrorCollector::new();
    let parsed = parser
        .parse_chat_file(&output, 0, &errors)
        .into_option()
        .unwrap();
    assert!(errors.into_vec().is_empty());
    assert_eq!(sanitize(parsed, &policy).unwrap().to_chat_string(), output);
}

#[test]
fn registry_lexical_runs_sanitize_once_in_both_backends() {
    lexical_runs(&TreeSitterParser::new().unwrap());
    lexical_runs(&talkbank_parser_re2c::Re2cParser::new());
}

#[test]
fn reference_sanitization_is_parseable_deterministic_and_idempotent() {
    let parser = TreeSitterParser::new().expect("parser");
    let policy = SanitizationPolicy::strict();
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut failures = Vec::new();
    let mut redacted_headers = std::collections::BTreeSet::new();
    for fixture in corpus.fixtures() {
        let original =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        let output = sanitize(original.clone(), &policy)
            .expect("sanitize reference")
            .to_chat_string();
        let repeated = sanitize(original.clone(), &policy)
            .expect("repeat sanitization")
            .to_chat_string();
        assert_eq!(
            output,
            repeated,
            "determinism: {}",
            fixture.path().display()
        );
        let parsed = match strict_parse(parser.parse_chat_file(&output)) {
            Ok(parsed) => parsed,
            Err(error) => {
                failures.push(format!(
                    "{}: sanitized output does not parse: {error:?}",
                    fixture.path().display()
                ));
                continue;
            }
        };
        let again = sanitize(parsed.clone(), &policy)
            .expect("sanitize parsed output")
            .to_chat_string();
        if output != again {
            failures.push(format!(
                "{}: sanitization is not idempotent\nfirst pass:\n{output}\nsecond pass:\n{again}",
                fixture.path().display()
            ));
        }
        assert_eq!(
            original.headers_with_locations().count(),
            parsed.headers_with_locations().count()
        );
        for ((before, _), (after, _)) in original
            .headers_with_locations()
            .zip(parsed.headers_with_locations())
        {
            assert_eq!(before.name(), after.name(), "header kind must survive");
            let redacted = match after {
                Header::Situation { text } => Some(text.as_str()),
                Header::TapeLocation { location } => Some(location.as_str()),
                Header::Location { location } => Some(location.as_str()),
                Header::RoomLayout { layout } => Some(layout.as_str()),
                Header::Birthplace { place, .. } => Some(place.as_str()),
                Header::Transcriber { transcriber } => Some(transcriber.as_str()),
                Header::Warning { text } => Some(text.as_str()),
                Header::Activities { activities } => Some(activities.as_str()),
                Header::Bck { bck } => Some(bck.as_str()),
                _ => None,
            };
            if let Some(text) = redacted {
                assert_eq!(
                    text,
                    "[redacted]",
                    "free-text header {}: {}",
                    after.name(),
                    fixture.path().display()
                );
                redacted_headers.insert(after.name().to_owned());
                if let (
                    Header::Birthplace {
                        participant: before,
                        ..
                    },
                    Header::Birthplace {
                        participant: after, ..
                    },
                ) = (before, after)
                {
                    assert_eq!(
                        before, after,
                        "redacting birthplace must not rename its speaker"
                    );
                }
            } else if !matches!(
                before,
                Header::Participants { .. } | Header::ID(_) | Header::Comment { .. }
            ) {
                assert!(
                    before.semantic_eq(after),
                    "preserved header {}: {}",
                    before.name(),
                    fixture.path().display()
                );
            }
        }
        assert_eq!(parsed.utterances().count(), original.utterances().count());
        for (before, after) in original.utterances().zip(parsed.utterances()) {
            assert_eq!(before.main.speaker, after.main.speaker);
            assert!(
                before
                    .main
                    .content
                    .bullet
                    .semantic_eq(&after.main.content.bullet),
                "main timing: {}",
                fixture.path().display()
            );
            assert!(
                match (before.gra_tier(), after.gra_tier()) {
                    (Some(before), Some(after)) => before.semantic_eq(after),
                    (None, None) => true,
                    _ => false,
                },
                "grammatical relations: {}",
                fixture.path().display()
            );
        }
    }
    assert!(
        failures.is_empty(),
        "canonical sanitizer failures:\n{}",
        failures.join("\n")
    );
    assert_eq!(
        redacted_headers.len(),
        9,
        "every promised free-text header needs a reference witness: {redacted_headers:?}"
    );
}

#[test]
fn source_sanitization_checks_loss_before_intentional_redaction() {
    use talkbank_model::ErrorCollector;
    use talkbank_transform::{parse_source_with_parser, redact::SourceSanitizationError};
    let parser = TreeSitterParser::new().unwrap();
    let policy = SanitizationPolicy::strict();
    let authored = include_str!("../error_corpus/validation_errors/E601_1.cha");
    for original in [
        authored.replace("%mor:\t|||", "%mor:\t|||\n%mor:\t|||"),
        authored.replace("*CHI:\thello .\n", ""),
        authored.replace("%mor:", "@Comment:\tintervening\n%mor:"),
    ] {
        for ending in ["\n", "\r\n", "\r"] {
            let source = original.replace('\n', ending);
            let errors = ErrorCollector::new();
            assert!(matches!(
                parse_source_with_parser(&parser, &source).sanitize(&policy, &errors),
                Err(SourceSanitizationError::DroppedContent(_))
            ));
            assert!(errors.has_errors());
        }
    }
    // A reference source can legitimately change whitespace and lexical text.
    let source = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
    for ending in ["\n", "\r\n", "\r"] {
        let source = source.replace('\n', ending);
        let errors = ErrorCollector::new();
        let result = parse_source_with_parser(&parser, &source)
            .sanitize(&policy, &errors)
            .unwrap_or_else(|error| panic!("reference refused: {error}"));
        assert!(!errors.has_errors());
        assert!(!result.to_chat_string().is_empty());
    }
}
