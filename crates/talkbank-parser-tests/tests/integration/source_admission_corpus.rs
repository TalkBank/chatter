//! Parsed source ownership travels with complete admission, not replacement alone.

use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{AlignmentValidation, ValidChatFile, ValidationPolicy};
use talkbank_model::{NullErrorSink, RuleSelection, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::{
    AdmittedSourceChat, parse_source_with_parser, parse_validated_with_parser,
};

const SOURCE: &str = include_str!("../../../../corpus/reference/languages/eng-conversation.cha");

fn parse() -> ValidChatFile {
    parse_validated_with_parser(
        &TreeSitterParser::new().expect("grammar loads"),
        SOURCE,
        ValidationPolicy::new(
            RuleSelection::new(),
            AlignmentValidation::IncludeTierAlignment,
        ),
        TranscriptName::Anonymous,
        &NullErrorSink,
    )
    .expect("reference admission")
}

#[test]
fn producer_preservation_handoff_keeps_source_bytes_and_model_without_reparsing() {
    let source = SOURCE.replace('\n', "\r\n");
    let parser = TreeSitterParser::new().expect("grammar loads");
    let admitted = parser
        .admit_planned_tiers(&source, TranscriptName::Anonymous, |_| None)
        .expect("complete source is valid");
    let talkbank_parser::AdmittedDisposition::Preserved(preserved) = admitted.into_disposition()
    else {
        panic!("no replacement was selected");
    };
    let output = AdmittedSourceChat::from_preservation(preserved);
    assert_eq!(output.source(), source);
    assert_eq!(output.document().to_chat_string(), SOURCE);
    let owned = output.into_owned();
    drop(source);
    assert_eq!(owned.source(), SOURCE.replace('\n', "\r\n"));
    assert_eq!(owned.document().to_chat_string(), SOURCE);
}

#[test]
fn source_admission_preserves_exact_bytes_and_can_own_them() {
    let source = SOURCE.replace("@End", "@Comment:\tkept   \n@End");
    let parsed =
        parse_source_with_parser(&TreeSitterParser::new().expect("grammar loads"), &source);
    assert!(parsed.document().is_some());
    let admitted = parsed
        .admit(TranscriptName::Anonymous, &NullErrorSink)
        .expect("the reference and ordinary comment are valid");
    assert_eq!(admitted.source(), source);
    let owned = admitted.into_owned();
    drop(source);
    assert!(owned.source().contains("kept   \n"));
    assert!(owned.document().utterances().next().is_some());
    assert!(owned.into_source().contains("kept   \n"));
}

#[test]
fn invalid_source_cannot_acquire_unchanged_output_authority() {
    let source = SOURCE.replace("@Languages:\teng", "@Languages:\teng\n@Languages:\teng");
    assert_ne!(source, SOURCE, "the deliberate duplicate must be inserted");
    let parsed =
        parse_source_with_parser(&TreeSitterParser::new().expect("grammar loads"), &source);
    assert!(parsed.document().is_some(), "recovery is not admission");
    assert!(
        parsed
            .admit(TranscriptName::Anonymous, &NullErrorSink)
            .is_err()
    );
}

#[test]
fn accepted_payload_keeps_policy_and_serialization() {
    let accepted = parse();
    assert_eq!(
        accepted.policy().alignment(),
        AlignmentValidation::IncludeTierAlignment
    );
    assert_eq!(
        accepted.to_chat_string(),
        accepted.document().to_chat_string()
    );
    assert_eq!(
        serde_json::to_value(&accepted).expect("admitted wrapper wire"),
        serde_json::to_value(accepted.document()).expect("document wire")
    );
}
