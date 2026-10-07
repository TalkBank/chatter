//! Required-validation entry point. Recovery remains available through ParseProduct.

use std::borrow::Cow;
use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{ValidChatFile, ValidationFailure, ValidationPolicy};
use talkbank_model::{CompletedDiagnostics, ErrorSink, InternalFailure};
use talkbank_parser::{ParseProduct, TreeSitterParser};

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
    parser: &TreeSitterParser,
    source: &'source str,
) -> ParsedSourceChat<'source> {
    ParsedSourceChat {
        source,
        product: parser.parse_chat_file(source),
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
    parser: &TreeSitterParser,
    content: &str,
    policy: ValidationPolicy,
    name: TranscriptName<'_>,
    errors: &impl ErrorSink,
) -> Result<ValidChatFile, ValidatedParseError> {
    admit_product(parser.parse_chat_file(content), policy, name, errors)
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
    fn producer_preservation_handoff_keeps_source_bytes_and_model_without_reparsing() {
        let source = SOURCE.replace('\n', "\r\n");
        let parser = TreeSitterParser::new().expect("grammar loads");
        let admitted = parser
            .admit_planned_tiers(&source, TranscriptName::Anonymous, |_| None)
            .expect("complete source is valid");
        let talkbank_parser::AdmittedDisposition::Preserved(preserved) =
            admitted.into_disposition()
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
    fn source_admission_preserves_exact_bytes_and_can_own_them() {
        let source = SOURCE.replace("@End", "@Comment:\tkept   \n@End");
        let parsed = parse_source_with_parser(&TreeSitterParser::new().unwrap(), &source);
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
        let parsed = parse_source_with_parser(&TreeSitterParser::new().unwrap(), &source);
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
