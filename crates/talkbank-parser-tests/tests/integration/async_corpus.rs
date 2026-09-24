//! Async admission contracts over canonical spec inputs, not fabricated models.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{FileStem, SemanticEq, TranscriptName};
use talkbank_model::validation::{AsyncValidationError, validate_async};
use talkbank_model::{ErrorCollector, NullErrorSink};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{repo_paths::workspace_root, test_error::strict_parse};
use talkbank_spec_vocabulary::validation_manifest::{FixtureTranscriptName, ValidationManifest};

#[tokio::test]
async fn spec_async_admission_preserves_sync_proofs_and_refusals() {
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let manifest: ValidationManifest = serde_json::from_str(
        &std::fs::read_to_string(root.join("manifest.json")).expect("canonical manifest"),
    )
    .expect("producer-owned manifest schema");
    let parser = TreeSitterParser::new().expect("parser");
    let mut outcomes = [0; 2];
    for entry in &manifest.fixtures {
        let source = std::fs::read_to_string(root.join(&entry.fixture)).expect("canonical fixture");
        // This boundary validates cleanly parsed documents. Parse-refusal
        // transport is covered by the synchronous pipeline corpus workflow.
        let Ok(file) = strict_parse(parser.parse_chat_file(&source)) else {
            continue;
        };
        let filename = match &entry.transcript_name {
            FixtureTranscriptName::Anonymous => None,
            FixtureTranscriptName::Named(stem) => Some(stem.clone()),
        };
        let name = filename
            .as_deref()
            .map_or(TranscriptName::Anonymous, |stem| {
                TranscriptName::Named(FileStem::from_stem(stem))
            });
        let errors = ErrorCollector::new();
        let synchronous = file.clone().validate_into(&errors, name);
        // The custom-rule API is a streamed-diagnostic operation, not a
        // validity proof. Compare its complete stream under authored rules.
        let selected_errors = ErrorCollector::new();
        file.validate_with_rules(entry.rules.selection(), &selected_errors, name);
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        talkbank_model::validation::validate_with_rules_async(
            file.clone(),
            entry.rules.selection(),
            talkbank_model::errors::AsyncChannelErrorSink::new(sender),
            filename.clone(),
        )
        .await
        .expect("async validation task completes");
        let mut streamed = Vec::new();
        while let Some(diagnostic) = receiver.recv().await {
            streamed.push(diagnostic);
        }
        assert_eq!(
            selected_errors.to_vec(),
            streamed,
            "authored-rule stream: {}",
            entry.fixture
        );
        // Discarding streamed diagnostics must never manufacture a proof.
        let asynchronous = validate_async(file, NullErrorSink, filename.clone()).await;
        match (synchronous, asynchronous) {
            (Ok(expected), Ok(actual)) => {
                assert_eq!(expected.policy(), actual.policy());
                assert_eq!(expected.name(), actual.name());
                assert_eq!(expected.diagnostics(), actual.diagnostics());
                assert!(expected.document().semantic_eq(actual.document()));
                outcomes[0] += 1;
            }
            (Err(expected), Err(AsyncValidationError::Validation(actual))) => {
                assert_eq!(expected.policy(), actual.policy());
                assert_eq!(expected.name(), actual.name());
                assert_eq!(expected.diagnostics(), actual.diagnostics());
                assert_eq!(
                    expected.has_incomplete_parse(),
                    actual.has_incomplete_parse()
                );
                assert!(expected.document().semantic_eq(actual.document()));
                outcomes[1] += 1;
            }
            (expected, actual) => panic!(
                "async admission differs for {}: {expected:?} / {actual:?}",
                entry.fixture
            ),
        }
    }
    assert!(
        outcomes.iter().all(|count| *count > 0),
        "both proof and refusal required: {outcomes:?}"
    );
}
