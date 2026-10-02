//! Async admission contracts over canonical spec inputs, not fabricated models.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{OwnedTranscriptName, SemanticEq};
use talkbank_model::validation::{AsyncValidationError, validate_async};
use talkbank_model::{ErrorCollector, ErrorSink, NullErrorSink, ParseError};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{repo_paths::workspace_root, test_error::strict_parse};
use talkbank_spec_vocabulary::validation_manifest::{FixtureTranscriptName, ValidationManifest};

/// An external consumer can fail independently of the transcript's validity.
struct PanickingDiagnosticConsumer;

impl ErrorSink for PanickingDiagnosticConsumer {
    fn report(&self, _error: ParseError) {
        panic!("injected diagnostic consumer failure");
    }
}

#[tokio::test]
async fn spec_async_consumer_failure_is_a_task_failure_not_a_validation_result() {
    let source = std::fs::read_to_string(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E501_3.cha"),
    )
    .expect("canonical duplicate language header");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("cleanly parsed spec input");

    // Neither API may mistake an interrupted diagnostic stream for completed
    // validation. In particular, the admission API must never yield a proof.
    let admission = validate_async(
        file.clone(),
        PanickingDiagnosticConsumer,
        OwnedTranscriptName::Anonymous,
    )
    .await;
    assert!(matches!(admission, Err(AsyncValidationError::Join(error)) if error.is_panic()));

    let streamed = talkbank_model::validation::validate_with_rules_async(
        file,
        talkbank_model::validation::RuleSelection::new(),
        PanickingDiagnosticConsumer,
        OwnedTranscriptName::Anonymous,
    )
    .await;
    assert!(matches!(streamed, Err(AsyncValidationError::Join(error)) if error.is_panic()));
}

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
        let owned = match &entry.transcript_name {
            FixtureTranscriptName::Anonymous => OwnedTranscriptName::Anonymous,
            FixtureTranscriptName::Named(stem) => OwnedTranscriptName::Named(
                talkbank_model::model::OwnedFileStem::new(stem).expect("a stem"),
            ),
        };
        let name = owned.borrow();
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
            owned.clone(),
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
        let asynchronous = validate_async(file, NullErrorSink, owned.clone()).await;
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
