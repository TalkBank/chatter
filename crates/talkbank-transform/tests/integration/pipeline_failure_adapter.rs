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

/// A backend choice cannot select a different source/model admission policy.
#[test]
fn both_backends_share_retained_admission_and_refusal() {
    use talkbank_model::ChatParser;
    use talkbank_parser_re2c::Re2cParser;
    use talkbank_transform::parse_source_with_parser;

    fn check(parser: &impl ChatParser) {
        let errors = ErrorCollector::new();
        let admitted = parse_validated_with_parser(
            parser,
            REFERENCE,
            policy(),
            TranscriptName::Anonymous,
            &errors,
        )
        .expect("authored reference admission");
        assert!(!errors.has_errors());
        let preserved = parse_source_with_parser(parser, REFERENCE)
            .admit(TranscriptName::Anonymous, &errors)
            .expect("source admission with full alignment policy");
        assert_eq!(preserved.source(), REFERENCE);
        assert!(preserved.document().semantic_eq(admitted.document()));

        let parse_errors = ErrorCollector::new();
        let refusal = parse_source_with_parser(parser, PARSE_ERROR)
            .admit(TranscriptName::Anonymous, &parse_errors)
            .expect_err("authored malformed input");
        let ValidatedParseError::Parse(product) = refusal else {
            panic!("malformed source must retain parse failure");
        };
        assert!(product.has_error_diagnostics());
        assert_eq!(product.diagnostics(), parse_errors.into_vec());

        let model_errors = ErrorCollector::new();
        let refusal = parse_validated_with_parser(
            parser,
            INVALID,
            policy(),
            TranscriptName::Anonymous,
            &model_errors,
        )
        .expect_err("authored model invalidity");
        assert!(matches!(refusal, ValidatedParseError::Validation(_)));
        assert!(model_errors.has_errors());
    }
    check(&TreeSitterParser::new().expect("grammar"));
    check(&Re2cParser::new());
}

/// Optional validation changes admission policy, never backend ownership or
/// diagnostic delivery. Recovery is observable without treating it as validity.
#[test]
fn both_backends_preserve_optional_policy_and_streams() {
    use talkbank_model::{ChatParser, ParseValidateOptions};
    use talkbank_parser_re2c::Re2cParser;
    use talkbank_transform::{
        parse_and_validate_named, parse_and_validate_streaming_named,
        parse_and_validate_streaming_with_parser, parse_and_validate_with_parser,
    };
    const MEDIA_MISMATCH: &str = include_str!(
        "../../../../tests/error_corpus/validation_errors/E531_media_filename_mismatch.cha"
    );
    fn check(parser: &impl ChatParser) {
        let parsed =
            parse_and_validate_with_parser(parser, REFERENCE, ParseValidateOptions::default())
                .expect("reference parse-only");
        let errors = ErrorCollector::new();
        let streamed = parse_and_validate_streaming_with_parser(
            parser,
            REFERENCE,
            ParseValidateOptions::default(),
            &errors,
        )
        .expect("reference streaming");
        assert!(parsed.semantic_eq(&streamed));
        assert!(errors.into_vec().is_empty());

        let expected = ParseProduct::parse(parser, PARSE_ERROR);
        let PipelineError::Parse(refused) =
            parse_and_validate_with_parser(parser, PARSE_ERROR, ParseValidateOptions::default())
                .expect_err("ordinary parse-only rejects parse errors")
        else {
            panic!("parse refusal must not become parser creation failure");
        };
        assert_eq!(refused.errors, expected.diagnostics());
        let recovery_errors = ErrorCollector::new();
        parse_and_validate_streaming_with_parser(
            parser,
            PARSE_ERROR,
            ParseValidateOptions::default(),
            &recovery_errors,
        )
        .expect("parse-only streaming retains the backend's recovered model");
        assert_eq!(recovery_errors.into_vec(), expected.diagnostics());

        let forwarded = ErrorCollector::new();
        let product = ParseProduct::parse_streaming(parser, PARSE_ERROR, &forwarded);
        assert_eq!(product.diagnostics(), forwarded.into_vec());
        let discarded = ParseProduct::parse_streaming(parser, PARSE_ERROR, &NullErrorSink);
        assert_eq!(discarded.diagnostics(), expected.diagnostics());
        for source in [PARSE_ERROR, INVALID] {
            assert!(
                parse_and_validate_streaming_with_parser(
                    parser,
                    source,
                    ParseValidateOptions::default().with_alignment(),
                    &NullErrorSink,
                )
                .is_err(),
                "required checks cannot be disabled by the sink"
            );
        }
        parse_and_validate_with_parser(parser, INVALID, ParseValidateOptions::default())
            .expect("parse-only does not claim model validity");

        let name = TranscriptName::for_path(Path::new("different-transcript.cha"));
        let options = ParseValidateOptions::default().with_validation();
        let ordinary = parse_and_validate_named(parser, MEDIA_MISMATCH, options.clone(), name)
            .expect_err("named validation observes the transcript identity");
        let errors = ErrorCollector::new();
        let streamed =
            parse_and_validate_streaming_named(parser, MEDIA_MISMATCH, options, &errors, name)
                .expect_err("streaming named validation preserves identity");
        assert!(
            ordinary
                .diagnostics()
                .expect("diagnostics")
                .iter()
                .any(|d| d.code == ErrorCode::MediaFilenameMismatch)
        );
        assert_eq!(ordinary.diagnostics(), streamed.diagnostics());
        assert_eq!(streamed.diagnostics(), Some(errors.into_vec().as_slice()));
    }
    check(&TreeSitterParser::new().expect("grammar"));
    check(&Re2cParser::new());
}
