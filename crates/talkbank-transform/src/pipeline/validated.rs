//! Required-validation entry point. Recovery remains available through ParseProduct.

use std::borrow::Cow;
use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{ValidChatFile, ValidationFailure, ValidationPolicy};
use talkbank_model::{ChatParser, ParseProduct};
use talkbank_model::{CompletedDiagnostics, ErrorSink, InternalFailure};

/// One source parse, before its admission or release for editing.
/// The source and product can only be paired by the parser entry point.
#[derive(Debug)]
pub struct ParsedSourceChat<'source> {
    source: &'source str,
    product: ParseProduct,
}

impl<'source> ParsedSourceChat<'source> {
    /// Inspect the producer's model without mutation or admission authority.
    pub fn document(&self) -> Option<&talkbank_model::ChatFile> {
        match &self.product {
            ParseProduct::Built { file, .. } => Some(file),
            ParseProduct::Unbuildable { .. } => None,
        }
    }

    /// Admit this exact parse, including tier alignment, without reparsing.
    /// Callers cannot select a weaker policy for unchanged-output authority.
    pub fn admit(
        self,
        name: TranscriptName<'_>,
        errors: &impl ErrorSink,
    ) -> Result<AdmittedSourceChat<'source>, ValidatedParseError> {
        let file = admit_product(
            self.product,
            ValidationPolicy::new(
                talkbank_model::RuleSelection::new(),
                talkbank_model::validation::AlignmentValidation::IncludeTierAlignment,
            ),
            name,
            errors,
        )?;
        Ok(AdmittedSourceChat {
            source: Cow::Borrowed(self.source),
            file,
        })
    }

    /// Sanitize this exact source parse only after its unedited serialization
    /// retains ordered, disjoint source-line witnesses (ignoring whitespace).
    /// This checks for source loss before intentional redaction, not CHAT validity
    /// or semantic equivalence. Recovery diagnostics are forwarded unchanged.
    /// Internal failures and unbuildable parses never produce sanitized output.
    pub fn sanitize(
        self,
        policy: &crate::redact::SanitizationPolicy,
        errors: &impl ErrorSink,
    ) -> Result<crate::redact::SanitizedDocument, crate::redact::SourceSanitizationError> {
        errors.report_all(self.product.diagnostics().to_vec());
        CompletedDiagnostics::admit(self.product.diagnostics().to_vec())?;
        match self.product {
            ParseProduct::Built { file, .. } => {
                super::Rewrite::of(&file, self.source)?;
                Ok(crate::redact::sanitize(file, policy)?)
            }
            ParseProduct::Unbuildable { .. } => {
                Err(crate::redact::SourceSanitizationError::Unbuildable)
            }
        }
    }

    /// Release the unchecked producer outcome, discarding source admission
    /// authority. Recovery remains available to callers that inspect it.
    pub fn into_product(self) -> ParseProduct {
        self.product
    }
}

/// Immutable source bytes coupled to the complete admission of their parse.
/// There is no constructor accepting a separately supplied model and text.
///
/// ```compile_fail,E0451
/// # fn forge(source: &str, file: talkbank_model::validation::ValidChatFile) {
/// let _ = talkbank_transform::AdmittedSourceChat { source: source.into(), file };
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct AdmittedSourceChat<'source> {
    source: Cow<'source, str>,
    file: ValidChatFile,
}

impl<'source> AdmittedSourceChat<'source> {
    /// Retain complete source admission from the same producer-selected plan.
    /// Replacement admission alone cannot enter this unchanged-output boundary.
    ///
    /// ```compile_fail,E0308
    /// # use talkbank_transform::AdmittedSourceChat;
    /// # fn preserve(replacement: talkbank_parser::AdmittedReplacement<'_>) {
    /// let _ = AdmittedSourceChat::from_preservation(replacement);
    /// # }
    /// ```
    pub fn from_preservation(admitted: talkbank_parser::AdmittedPreservation<'source>) -> Self {
        let (file, source) = admitted.into_parts();
        Self {
            source: Cow::Borrowed(source),
            file,
        }
    }

    /// Original bytes, not a serialization of the admitted model.
    pub fn source(&self) -> &str {
        self.source.as_ref()
    }

    /// The admitted model, without mutation authority.
    pub fn document(&self) -> &talkbank_model::ChatFile {
        self.file.document()
    }

    /// Own the source for storage or an asynchronous output boundary.
    pub fn into_owned(self) -> AdmittedSourceChat<'static> {
        AdmittedSourceChat {
            source: Cow::Owned(self.source.into_owned()),
            file: self.file,
        }
    }

    /// Consume the source binding and retain only model admission.
    pub fn into_valid_file(self) -> ValidChatFile {
        self.file
    }

    /// Consume the proof and return its original bytes, reusing owned storage.
    /// Callers needing write authority must retain the proof until that boundary.
    pub fn into_source(self) -> String {
        self.source.into_owned()
    }
}

/// Parse source once, retaining an opaque source/product pair for policy
/// selection before admission. This does not certify validity or recovery.
pub fn parse_source_with_parser<'source>(
    parser: &impl ChatParser,
    source: &'source str,
) -> ParsedSourceChat<'source> {
    ParsedSourceChat {
        source,
        product: ParseProduct::parse(parser, source),
    }
}

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
    parser: &impl ChatParser,
    content: &str,
    policy: ValidationPolicy,
    name: TranscriptName<'_>,
    errors: &impl ErrorSink,
) -> Result<ValidChatFile, ValidatedParseError> {
    admit_product(ParseProduct::parse(parser, content), policy, name, errors)
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
    use talkbank_model::{ErrorCollector, NullErrorSink, RuleSelection};
    use talkbank_parser::TreeSitterParser;
    use talkbank_parser_tests::test_error::ParseProductTestExt;

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
            let sanitizer_errors = ErrorCollector::new();
            let source_parse = ParsedSourceChat {
                source: SOURCE,
                product: product.clone(),
            };
            assert!(matches!(
                source_parse.sanitize(
                    &crate::redact::SanitizationPolicy::strict(),
                    &sanitizer_errors
                ),
                Err(crate::redact::SourceSanitizationError::Internal(_))
            ));
            assert_eq!(
                sanitizer_errors.into_vec()[0].code,
                talkbank_model::ErrorCode::InternalError
            );
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
