//! Channel transport cannot alter the proof/refusal produced from canonical CHAT.

use talkbank_model::errors::ChannelErrorSink;
use talkbank_model::model::{SemanticEq, TranscriptName};
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{ErrorCollector, RuleSelection};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_transform::parse_validated_with_parser;

#[test]
fn spec_channel_transport_preserves_diagnostics_and_admission() {
    let parser = TreeSitterParser::new().expect("parser");
    // Legal morphology, parse recovery, and cleanly parsed structural invalidity.
    for (fixture, valid) in [("E702_3", true), ("E702_1", false), ("E370_1", false)] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{fixture}.cha",
        )))
        .expect("canonical channel input");
        let policy = ValidationPolicy::new(
            RuleSelection::new(),
            AlignmentValidation::IncludeTierAlignment,
        );
        let expected_errors = ErrorCollector::new();
        let expected = parse_validated_with_parser(
            &parser,
            &source,
            policy,
            TranscriptName::Anonymous,
            &expected_errors,
        );
        assert_eq!(
            expected.is_ok(),
            valid,
            "{fixture}: authored validity control"
        );

        let (sender, receiver) = crossbeam_channel::unbounded();
        let sink = ChannelErrorSink::new(sender);
        let streamed =
            parse_validated_with_parser(&parser, &source, policy, TranscriptName::Anonymous, &sink);
        // Sender ownership closes the stream; the drain cannot wait for a
        // sender retained elsewhere. This is transport completion, not validity.
        drop(sink);
        let received: Vec<_> = receiver.into_iter().collect();
        assert_eq!(
            received,
            expected_errors.into_vec(),
            "{fixture}: complete ordered diagnostics"
        );

        let (sender, receiver) = crossbeam_channel::unbounded();
        drop(receiver);
        let disconnected = parse_validated_with_parser(
            &parser,
            &source,
            policy,
            TranscriptName::Anonymous,
            &ChannelErrorSink::new(sender),
        );
        match (expected, streamed, disconnected) {
            (Ok(expected), Ok(streamed), Ok(disconnected)) => {
                assert!(expected.document().semantic_eq(streamed.document()));
                assert!(expected.document().semantic_eq(disconnected.document()));
                assert_eq!(expected.policy(), streamed.policy());
                assert_eq!(expected.policy(), disconnected.policy());
            }
            (Err(expected), Err(streamed), Err(disconnected)) => {
                assert_eq!(
                    std::mem::discriminant(&expected),
                    std::mem::discriminant(&streamed)
                );
                assert_eq!(
                    std::mem::discriminant(&expected),
                    std::mem::discriminant(&disconnected)
                );
            }
            results => panic!("{fixture}: transport changed admission: {results:?}"),
        }
    }
}
