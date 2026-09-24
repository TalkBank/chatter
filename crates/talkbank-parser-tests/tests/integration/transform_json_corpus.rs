//! JSON schema policy must not bypass authored CHAT admission requirements.

use talkbank_model::ParseValidateOptions;
use talkbank_model::model::{ChatFile, FileStem, SemanticEq, TranscriptName};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;
use talkbank_spec_vocabulary::validation_manifest::{FixtureTranscriptName, ValidationManifest};
use talkbank_transform::{JsonSchemaPolicy, PipelineError, chat_to_json_with_schema_policy};

#[test]
fn underline_specs_keep_semantics_without_fabricating_wire_locations() {
    use talkbank_model::alignment::helpers::{ContentItem, walk_content};
    use talkbank_model::model::{UnderlineMarker, Word, WordContent};

    fn inspect_word(word: &Word, inspect: &mut impl FnMut(&UnderlineMarker)) {
        for piece in word.content() {
            if let WordContent::UnderlineBegin(marker) | WordContent::UnderlineEnd(marker) = piece {
                inspect(marker);
            }
        }
    }

    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let manifest: ValidationManifest = serde_json::from_str(
        &std::fs::read_to_string(root.join("manifest.json")).expect("canonical manifest"),
    )
    .expect("producer-owned manifest schema");
    let parser = TreeSitterParser::new().expect("parser");
    for entry in manifest
        .fixtures
        .iter()
        .filter(|entry| matches!(entry.code.as_str(), "E356" | "E357"))
    {
        let source = std::fs::read_to_string(root.join(&entry.fixture)).expect("underline fixture");
        let original =
            strict_parse(parser.parse_chat_file(&source)).expect("underline syntax parses");
        let wire =
            talkbank_transform::json::to_json_validated(&original).expect("schema accepts markers");
        let restored: ChatFile = serde_json::from_str(&wire).expect("typed wire model");
        assert!(
            original.semantic_eq(&restored),
            "source locations are not semantics"
        );
        for (file, located) in [(&original, true), (&restored, false)] {
            let mut markers = 0;
            let mut inspect = |marker: &UnderlineMarker| {
                assert_eq!(marker.span().is_some(), located, "{}", entry.fixture);
                markers += 1;
            };
            for utterance in file.utterances() {
                walk_content(
                    &utterance.main.content.content,
                    None,
                    &mut |item| match item {
                        ContentItem::UnderlineBegin(marker) | ContentItem::UnderlineEnd(marker) => {
                            inspect(marker)
                        }
                        ContentItem::Word(word) => inspect_word(word, &mut inspect),
                        ContentItem::ReplacedWord(replaced) => {
                            inspect_word(&replaced.word, &mut inspect);
                            for word in &replaced.replacement.words {
                                inspect_word(word, &mut inspect);
                            }
                        }
                        ContentItem::Separator(_)
                        | ContentItem::Event(_)
                        | ContentItem::Pause(_)
                        | ContentItem::Action(_)
                        | ContentItem::OverlapPoint(_)
                        | ContentItem::OtherSpokenEvent(_)
                        | ContentItem::Freecode(_)
                        | ContentItem::InternalBullet(_)
                        | ContentItem::LongFeatureBegin(_)
                        | ContentItem::LongFeatureEnd(_)
                        | ContentItem::NonvocalBegin(_)
                        | ContentItem::NonvocalEnd(_)
                        | ContentItem::NonvocalSimple(_) => {}
                    },
                );
            }
            assert!(markers > 0, "spec must witness markers: {}", entry.fixture);
            let errors = talkbank_model::ErrorCollector::new();
            file.validate(&errors, TranscriptName::Anonymous);
            let diagnostics = errors.to_vec();
            assert!(
                entry.claim.satisfied_by(&entry.code, |code| diagnostics
                    .iter()
                    .any(|error| error.code.as_str() == code.as_str())),
                "underline claim after wire transition: {} {diagnostics:?}",
                entry.fixture
            );
        }
    }
}

#[test]
fn media_name_spec_claims_survive_both_schema_policies() {
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let manifest: ValidationManifest = serde_json::from_str(
        &std::fs::read_to_string(root.join("manifest.json")).expect("canonical manifest"),
    )
    .expect("producer-owned manifest schema");
    let parser = TreeSitterParser::new().expect("parser");
    let mut outcomes = [0; 2];
    for entry in manifest
        .fixtures
        .iter()
        .filter(|entry| entry.code.as_str() == "E531")
    {
        let source =
            std::fs::read_to_string(root.join(&entry.fixture)).expect("canonical name fixture");
        let original =
            strict_parse(parser.parse_chat_file(&source)).expect("name examples parse cleanly");
        let name = match &entry.transcript_name {
            FixtureTranscriptName::Anonymous => TranscriptName::Anonymous,
            FixtureTranscriptName::Named(stem) => TranscriptName::Named(FileStem::from_stem(stem)),
        };
        for schema in [JsonSchemaPolicy::Validate, JsonSchemaPolicy::Skip] {
            for pretty in [false, true] {
                match chat_to_json_with_schema_policy(
                    &source,
                    ParseValidateOptions::default().with_validation(),
                    pretty,
                    name,
                    schema,
                ) {
                    Ok(wire) => {
                        assert!(
                            entry.claim.satisfied_by(&entry.code, |_| false),
                            "conversion admitted a violation: {}",
                            entry.fixture
                        );
                        let restored: ChatFile =
                            serde_json::from_str(&wire).expect("typed wire model");
                        assert!(original.semantic_eq(&restored));
                        outcomes[0] += 1;
                    }
                    Err(PipelineError::Validation(errors)) => {
                        assert!(
                            entry.claim.satisfied_by(&entry.code, |code| errors
                                .iter()
                                .any(|error| error.code.as_str() == code.as_str())),
                            "authored name claim: {} {errors:?}",
                            entry.fixture
                        );
                        assert!(
                            errors.iter().any(|error| error.code.as_str() == "E531"),
                            "name mismatch must survive schema policy: {}",
                            entry.fixture
                        );
                        outcomes[1] += 1;
                    }
                    other => panic!(
                        "unexpected conversion outcome: {} {schema:?} {other:?}",
                        entry.fixture
                    ),
                }
            }
        }
    }
    assert!(
        outcomes.iter().all(|count| *count > 0),
        "must witness accepted and refused name mutations: {outcomes:?}"
    );
}
