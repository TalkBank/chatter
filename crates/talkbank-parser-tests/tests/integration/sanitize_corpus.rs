//! Sanitizer wire contracts over the canonical reference population.
#![allow(clippy::expect_used)]

use talkbank_model::model::{Header, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};
use talkbank_transform::redact::{SanitizationPolicy, sanitize};

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
                "{}: sanitization is not idempotent",
                fixture.path().display()
            ));
        }
        assert_eq!(
            original.headers_with_spans().count(),
            parsed.headers_with_spans().count()
        );
        for ((before, _), (after, _)) in original
            .headers_with_spans()
            .zip(parsed.headers_with_spans())
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
