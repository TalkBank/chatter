//! Required-validation entry point. Recovery remains available through ParseProduct.

use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{ValidChatFile, ValidationFailure, ValidationPolicy};
use talkbank_model::{CompletedDiagnostics, ErrorSink, InternalFailure};
use talkbank_parser::{ParseProduct, TreeSitterParser};

/// A failed source-to-valid-model transition retains every model that was built.
#[derive(Debug, thiserror::Error)]
pub enum ValidatedParseError {
    /// The tool failed; retain its partial product without a CHAT verdict.
    #[error("{failure}")]
    InternalFailure {
        /// Any model and diagnostics available for inspecting the failed run.
        product: Box<ParseProduct>,
        /// Evidence that the run failed internally, independent of severity.
        failure: InternalFailure,
    },
    /// Parsing failed or recovered malformed source; the product retains evidence.
    #[error("source parsing did not produce an error-free document")]
    Parse(Box<ParseProduct>),
    /// Model admission failed. The retained evidence distinguishes an internal
    /// failure from invalidity or incomplete parse provenance.
    #[error(transparent)]
    Validation(#[from] ValidationFailure),
}

/// Parse a complete source document and require the requested model checks to pass.
/// Unlike optional-validation APIs, this function cannot return unchecked output.
/// All parse diagnostics are forwarded before model validation starts.
pub fn parse_validated_with_parser(
    parser: &TreeSitterParser,
    content: &str,
    policy: ValidationPolicy,
    name: TranscriptName<'_>,
    errors: &impl ErrorSink,
) -> Result<ValidChatFile, ValidatedParseError> {
    let product = parser.parse_chat_file(content);
    admit_product(product, policy, name, errors)
}

/// One admission boundary for every complete parse attempt, before severity
/// filtering or model validation can discard producer-failure evidence.
fn admit_product(
    product: ParseProduct,
    policy: ValidationPolicy,
    name: TranscriptName<'_>,
    errors: &impl ErrorSink,
) -> Result<ValidChatFile, ValidatedParseError> {
    errors.report_all(product.diagnostics().to_vec());
    if let Err(failure) = CompletedDiagnostics::admit(product.diagnostics().to_vec()) {
        return Err(ValidatedParseError::InternalFailure {
            product: Box::new(product),
            failure,
        });
    }
    if product.has_error_diagnostics() {
        return Err(ValidatedParseError::Parse(Box::new(product)));
    }
    match product {
        ParseProduct::Built { file, .. } => Ok(file.validate_with_policy(policy, errors, name)?),
        unbuildable @ ParseProduct::Unbuildable { .. } => {
            Err(ValidatedParseError::Parse(Box::new(unbuildable)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::validation::AlignmentValidation;
    use talkbank_model::{ErrorCollector, NullErrorSink, RuleSelection, WriteChat};

    const SOURCE: &str =
        include_str!("../../../../corpus/reference/languages/eng-conversation.cha");

    #[test]
    fn internal_failure_retains_product_and_blocks_admission_even_at_warning_severity() {
        let diagnostic = talkbank_model::ParseError::at_span(
            talkbank_model::ErrorCode::InternalError,
            talkbank_model::Severity::Warning,
            talkbank_model::Span::new(0, 1),
            "producer fault",
        );
        for product in [
            ParseProduct::Built {
                file: TreeSitterParser::new()
                    .unwrap()
                    .parse_chat_file(SOURCE)
                    .expect_built(),
                diagnostics: vec![diagnostic.clone()],
            },
            ParseProduct::Unbuildable {
                diagnostics: vec![diagnostic.clone()],
            },
        ] {
            let was_built = product.is_built();
            let sink = ErrorCollector::new();
            let result = admit_product(
                product,
                ValidationPolicy::new(
                    RuleSelection::new(),
                    AlignmentValidation::IncludeTierAlignment,
                ),
                TranscriptName::Anonymous,
                &sink,
            );
            let Err(ValidatedParseError::InternalFailure { product, failure }) = result else {
                panic!("a producer failure must never become a validity verdict");
            };
            assert_eq!(product.is_built(), was_built);
            assert_eq!(failure.diagnostics().len(), 1);
            assert_eq!(
                sink.into_vec()[0].code,
                talkbank_model::ErrorCode::InternalError
            );
        }
    }

    fn parse() -> ValidChatFile {
        parse_validated_with_parser(
            &TreeSitterParser::new().unwrap(),
            SOURCE,
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            TranscriptName::Anonymous,
            &NullErrorSink,
        )
        .unwrap()
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
            serde_json::to_value(&accepted).unwrap(),
            serde_json::to_value(accepted.document()).unwrap()
        );
    }

    #[test]
    fn editing_consumes_proof_and_invalid_output_cannot_regain_it() {
        let mut editable = parse().into_unchecked();
        editable.lines = Vec::new().into();
        assert!(
            editable
                .validate_into(&NullErrorSink, TranscriptName::Anonymous)
                .is_err()
        );
    }

    #[test]
    fn skipped_validation_on_unknown_parse_health_does_not_prove_validity() {
        let mut editable = parse().into_unchecked();
        editable
            .lines
            .as_mut_slice()
            .iter_mut()
            .find_map(|line| match line {
                talkbank_model::Line::Utterance(u) => Some(u),
                talkbank_model::Line::Header { .. } => None,
            })
            .unwrap()
            .forget_parse_provenance();
        let failure = editable
            .validate_into(&NullErrorSink, TranscriptName::Anonymous)
            .unwrap_err();
        assert!(failure.has_incomplete_parse());
    }

    #[test]
    fn rejected_source_is_retained_even_when_caller_discards_diagnostics() {
        let errors = ErrorCollector::new();
        let result = parse_validated_with_parser(
            &TreeSitterParser::new().unwrap(),
            "not a CHAT document",
            ValidationPolicy::new(RuleSelection::new(), AlignmentValidation::Structure),
            TranscriptName::Anonymous,
            &errors,
        );
        assert!(result.is_err());
        assert!(errors.has_errors());
    }
}
