//! Required-validation entry point. Recovery remains available through ParseProduct.

use talkbank_model::ErrorSink;
use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{ValidChatFile, ValidationFailure, ValidationPolicy};
use talkbank_parser::{ParseProduct, TreeSitterParser};

/// A failed source-to-valid-model transition retains every model that was built.
#[derive(Debug, thiserror::Error)]
pub enum ValidatedParseError {
    /// Parsing failed or recovered malformed source; the product retains evidence.
    #[error("source parsing did not produce an error-free document")]
    Parse(Box<ParseProduct>),
    /// The parsed model failed the requested validation policy.
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
    errors.report_all(product.diagnostics().to_vec());
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
    use talkbank_model::{
        ErrorCollector, NullErrorSink, ParseHealthState, RuleSelection, WriteChat,
    };

    const SOURCE: &str =
        include_str!("../../../../corpus/reference/languages/eng-conversation.cha");

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
            .parse_health = ParseHealthState::Unknown;
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
