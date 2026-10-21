//! Failure-adapter boundary contracts, not canonical CHAT coverage.

use std::path::Path;
use talkbank_model::model::{Line, SemanticEq, TranscriptName};
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{
    ChatFile, CompletedDiagnostics, ErrorCode, ErrorCollector, NullErrorSink, ParseError,
    RuleSelection, Severity, Span, WriteChat,
};
use talkbank_parser::{ParseProduct, TreeSitterParser};
use talkbank_transform::{PipelineError, ValidatedParseError, parse_validated_with_parser};

const REFERENCE: &str = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
const PARSE_ERROR: &str =
    include_str!("../../../../tests/error_corpus/parse_errors/E316_invalid_main_tier_syntax.cha");
const INVALID: &str = include_str!(
    "../../../../tests/error_corpus/validation_errors/E552_unlinked_with_wor_timing.cha"
);

fn policy() -> ValidationPolicy {
    ValidationPolicy::new(RuleSelection::new(), AlignmentValidation::Structure)
}

fn reference_model() -> ChatFile {
    parse_validated_with_parser(
        &TreeSitterParser::new().expect("grammar"),
        REFERENCE,
        policy(),
        TranscriptName::Anonymous,
        &NullErrorSink,
    )
    .expect("reference source is admitted")
    .into_unchecked()
}

#[test]
fn parse_rejection_preserves_every_diagnostic() {
    let sink = ErrorCollector::new();
    let error = parse_validated_with_parser(
        &TreeSitterParser::new().expect("grammar"),
        PARSE_ERROR,
        policy(),
        TranscriptName::Anonymous,
        &sink,
    )
    .expect_err("authored parse rejection");
    let ValidatedParseError::Parse(product) = error else {
        panic!("fixture must witness parse rejection, not another failure phase");
    };
    let expected = product.diagnostics().to_vec();
    assert!(!expected.is_empty());
    assert_eq!(sink.into_vec(), expected);
    let converted = PipelineError::from(ValidatedParseError::Parse(product));
    assert_eq!(converted.diagnostics(), Some(expected.as_slice()));
    let PipelineError::Parse(errors) = converted else {
        panic!("parse rejection must keep its phase");
    };
    assert_eq!(errors.errors, expected);
}

#[test]
fn model_invalidity_is_not_incomplete_provenance() {
    let sink = ErrorCollector::new();
    let error = parse_validated_with_parser(
        &TreeSitterParser::new().expect("grammar"),
        INVALID,
        policy(),
        TranscriptName::Anonymous,
        &sink,
    )
    .expect_err("authored model rejection");
    let ValidatedParseError::Validation(failure) = error else {
        panic!("fixture must witness model rejection");
    };
    assert!(!failure.has_incomplete_parse());
    assert!(!failure.has_internal_failure());
    let expected = failure.diagnostics().to_vec();
    assert!(!expected.is_empty());
    assert_eq!(sink.into_vec(), expected);
    let converted = PipelineError::from(ValidatedParseError::Validation(failure));
    assert_eq!(converted.diagnostics(), Some(expected.as_slice()));
    let PipelineError::Validation(errors) = converted else {
        panic!("ordinary invalidity must keep its phase");
    };
    assert_eq!(errors, expected);
}

#[test]
fn incomplete_provenance_retains_document_policy_name_and_diagnostics() {
    let original = reference_model();
    let wire = serde_json::to_value(&original).expect("model wire");
    let decoded: ChatFile = serde_json::from_value(wire).expect("model readback");
    assert!(decoded.semantic_eq(&original));
    let name = TranscriptName::for_path(Path::new("basic-conversation.cha"));
    let sink = ErrorCollector::new();
    let failure = decoded
        .validate_with_policy(policy(), &sink, name)
        .expect_err("wire content cannot reconstruct parser provenance");
    assert!(failure.has_incomplete_parse());
    assert!(!failure.has_internal_failure());
    let expected = failure.diagnostics().to_vec();
    let before = failure.document().to_chat_string();
    assert_eq!(sink.into_vec(), expected);
    let converted = PipelineError::from(ValidatedParseError::Validation(failure));
    assert_eq!(converted.diagnostics(), Some(expected.as_slice()));
    let PipelineError::IncompleteValidation(retained) = converted else {
        panic!("unknown provenance must not become ordinary invalidity");
    };
    assert!(retained.has_incomplete_parse());
    assert_eq!(retained.policy(), policy());
    assert_eq!(retained.name(), name);
    assert_eq!(retained.diagnostics(), expected);
    assert_eq!(retained.document().to_chat_string(), before);
    assert!(retained.document().semantic_eq(&original));
}

fn injected_diagnostics() -> Vec<ParseError> {
    vec![
        ParseError::at_span(
            ErrorCode::ParseFailed,
            Severity::Warning,
            Span::new(0, 1),
            "input finding",
        ),
        ParseError::at_span(
            ErrorCode::InternalError,
            Severity::Warning,
            Span::new(1, 2),
            "injected producer failure",
        ),
    ]
}

#[test]
fn producer_failure_stays_internal_even_with_only_warnings() {
    let expected = injected_diagnostics();
    for product in [
        ParseProduct::Built {
            file: reference_model(),
            diagnostics: expected.clone(),
        },
        ParseProduct::Unbuildable {
            diagnostics: expected.clone(),
        },
    ] {
        let failure = CompletedDiagnostics::admit(expected.clone()).expect_err("intrinsic E001");
        let converted = PipelineError::from(ValidatedParseError::InternalFailure {
            product: Box::new(product),
            failure,
        });
        assert_eq!(converted.diagnostics(), Some(expected.as_slice()));
        let PipelineError::InternalFailure(failure) = converted else {
            panic!("warning severity cannot turn a tool failure into invalidity");
        };
        assert_eq!(failure.diagnostics(), expected);
    }
}

#[test]
fn public_parse_failure_cannot_hide_an_internal_diagnostic() {
    let expected = injected_diagnostics();
    // Both enums are publicly constructible. The conversion must inspect the
    // product's evidence rather than trusting its caller-selected outer tag.
    for product in [
        ParseProduct::Built {
            file: reference_model(),
            diagnostics: expected.clone(),
        },
        ParseProduct::Unbuildable {
            diagnostics: expected.clone(),
        },
    ] {
        let converted = PipelineError::from(ValidatedParseError::Parse(Box::new(product)));
        assert_eq!(converted.diagnostics(), Some(expected.as_slice()));
        let PipelineError::InternalFailure(failure) = converted else {
            panic!("a caller-selected Parse tag must not hide intrinsic tool failure");
        };
        assert_eq!(failure.diagnostics(), expected);
    }
}

#[test]
fn validation_failure_stays_internal_even_with_unknown_provenance() {
    for unknown_provenance in [false, true] {
        let mut model = reference_model();
        if unknown_provenance {
            model = serde_json::from_value(serde_json::to_value(model).expect("wire"))
                .expect("wire readback");
        }
        // Inject through the existing diagnostic-bearing model boundary, not
        // by constructing ValidationFailure or pretending this is invalid CHAT.
        let injected = injected_diagnostics();
        let utterance = (&mut model.lines)
            .into_iter()
            .find_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance),
                _ => None,
            })
            .expect("reference utterance");
        utterance.alignment_diagnostics = injected.clone();
        let sink = ErrorCollector::new();
        let failure = model
            .validate_with_policy(policy(), &sink, TranscriptName::Anonymous)
            .expect_err("injected tool fault must block model admission");
        assert!(failure.has_internal_failure());
        assert!(
            !failure.has_incomplete_parse(),
            "internal failure has priority"
        );
        let expected = failure.diagnostics().to_vec();
        assert!(
            injected
                .iter()
                .all(|diagnostic| expected.contains(diagnostic))
        );
        assert_eq!(sink.into_vec(), expected);
        let converted = PipelineError::from(ValidatedParseError::Validation(failure));
        assert_eq!(converted.diagnostics(), Some(expected.as_slice()));
        let PipelineError::InternalFailure(retained) = converted else {
            panic!("validation tool failure must not become invalid or incomplete");
        };
        assert_eq!(retained.diagnostics(), expected);
    }
}
