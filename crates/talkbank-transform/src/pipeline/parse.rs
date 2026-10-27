//! Parse/validate pipeline entry points for CHAT content.
//!
//! This module provides pipeline functions that compose parsing and validation.
//! Most callers should use `parse_and_validate()` or `parse_and_validate_streaming()`.
//! For batch workflows where parser construction overhead matters, use the
//! `_with_parser` variants that accept a caller-provided `ChatParser`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ChatFile;
use talkbank_model::ParseValidateOptions;
use talkbank_model::validation::ValidationPolicy;
use talkbank_model::{ChatParser, CompletedDiagnostics, ParseOutcome, ParseProduct};
use talkbank_model::{ErrorSink, FragmentRangeError, FragmentSource, NullErrorSink, ParseErrors};
use talkbank_parser::TreeSitterParser;

use super::error::PipelineError;
use talkbank_model::model::TranscriptName;

/// Parse CHAT content and optionally validate.
///
/// This is the core pipeline function that:
/// 1. Creates a TreeSitterParser
/// 2. Parses CHAT content to ChatFile
/// 3. Optionally validates the data model
/// 4. Optionally validates tier alignment
///
/// # Arguments
///
/// * `content` - The CHAT file content as a string
/// * `options` - Parsing and validation options
///
/// # Returns
///
/// * `Ok(ChatFile)` - Successfully parsed (and validated if requested)
/// * `Err(PipelineError)` - Parse or validation errors
///
/// # Example
///
/// ```no_run
/// use talkbank_transform::{parse_and_validate, PipelineError};
/// use talkbank_model::ParseValidateOptions;
///
/// # fn parse_example() -> Result<(), PipelineError> {
/// let content = "*CHI:\thello world .";
/// let options = ParseValidateOptions::default().with_validation();
/// let chat_file = parse_and_validate(content, options)?;
/// # let _ = chat_file;
/// # Ok(())
/// # }
/// ```
pub fn parse_and_validate(
    content: &str,
    options: ParseValidateOptions,
) -> Result<ChatFile, PipelineError> {
    let parser =
        TreeSitterParser::new().map_err(|e| PipelineError::ParserCreation(format!("{e}")))?;
    parse_and_validate_with_parser(&parser, content, options)
}

/// Parse CHAT content and optionally validate using a caller-provided ChatParser.
///
/// This avoids per-call parser construction, which is useful for batch workflows.
pub fn parse_and_validate_with_parser(
    parser: &impl ChatParser,
    content: &str,
    options: ParseValidateOptions,
) -> Result<ChatFile, PipelineError> {
    parse_and_validate_named(parser, content, options, TranscriptName::Anonymous)
}

/// Parse and validate content whose transcript name is known.
///
/// The name decides whether the rules that compare the transcript against its
/// own file name run, E531 above all. The content-only entry points above pass
/// [`TranscriptName::Anonymous`], which is correct for a string that came from
/// nowhere in particular; [`super::io::parse_file_and_validate`] reads from
/// disk and passes the real one.
///
/// This exists because `parse_and_validate*` used to pass `None` and carry a
/// FOLLOW-UP comment saying E531 therefore did not run for `to-json` or any
/// other pipeline consumer. The comment stood in place of the fix for as long
/// as it existed, which is the failure mode a hazard note always has.
pub fn parse_and_validate_named(
    parser: &impl ChatParser,
    content: &str,
    options: ParseValidateOptions,
    name: TranscriptName<'_>,
) -> Result<ChatFile, PipelineError> {
    if let Some(policy) = options.validation_policy() {
        return required_validation(parser, content, policy, name, &NullErrorSink);
    }
    CompletedParse::try_from(ParseProduct::parse(parser, content))?.into_error_free()
}

/// Parse CHAT content and optionally validate with streaming error reporting.
///
/// Parse-only recovery forwards diagnostics as the parser emits them, while
/// retaining a copy to detect internal failures even if the sink discards them.
/// Required-validation calls collect parse diagnostics before admission and
/// then report validation findings. This API does not provide cancellation or
/// a bounded-memory guarantee.
///
/// # Arguments
///
/// * `content` - The CHAT file content as a string
/// * `options` - Parsing and validation options
/// * `errors` - ErrorSink that receives errors as they're discovered
///
/// # Returns
///
/// * `Ok(ChatFile)` - Recovered model when validation is not required
/// * `Err(PipelineError)` - Source admission or required validation failed
///
/// # Example
///
/// ```no_run
/// use talkbank_transform::parse_and_validate_streaming;
/// use talkbank_model::ParseValidateOptions;
/// use talkbank_model::ErrorCollector;
///
/// let content = "*CHI:\thello world .";
/// let options = ParseValidateOptions::default().with_validation();
/// let errors = ErrorCollector::new();
/// let chat_file = parse_and_validate_streaming(content, options, &errors);
/// // Errors are in the sink; source admission and required validation can fail.
/// ```
pub fn parse_and_validate_streaming(
    content: &str,
    options: ParseValidateOptions,
    errors: &impl ErrorSink,
) -> Result<ChatFile, PipelineError> {
    let parser =
        TreeSitterParser::new().map_err(|e| PipelineError::ParserCreation(format!("{e}")))?;
    parse_and_validate_streaming_with_parser(&parser, content, options, errors)
}

/// Streaming variant that reuses a caller-provided parser instance.
pub fn parse_and_validate_streaming_with_parser(
    parser: &impl ChatParser,
    content: &str,
    options: ParseValidateOptions,
    errors: &impl ErrorSink,
) -> Result<ChatFile, PipelineError> {
    parse_and_validate_streaming_named(parser, content, options, errors, TranscriptName::Anonymous)
}

/// Streaming variant that names the transcript after the file it came from.
///
/// The convenience the CLI wants: it already holds the path, and should not
/// have to construct a parser just to say what the transcript is called.
pub fn parse_and_validate_streaming_for_path(
    path: &std::path::Path,
    content: &str,
    options: ParseValidateOptions,
    errors: &impl ErrorSink,
) -> Result<ChatFile, PipelineError> {
    let stored = crate::paths::StoredTranscript::resolve(path)?;
    let parser =
        TreeSitterParser::new().map_err(|e| PipelineError::ParserCreation(format!("{e}")))?;
    parse_and_validate_streaming_named(&parser, content, options, errors, stored.name())
}

/// Streaming variant for a transcript whose name is known.
///
/// See [`parse_and_validate_named`] for why the name is a parameter rather
/// than an `Option` filled in with `None`.
pub fn parse_and_validate_streaming_named(
    parser: &impl ChatParser,
    content: &str,
    options: ParseValidateOptions,
    errors: &impl ErrorSink,
    name: TranscriptName<'_>,
) -> Result<ChatFile, PipelineError> {
    if let Some(policy) = options.validation_policy() {
        return required_validation(parser, content, policy, name, errors);
    }
    let source =
        FragmentSource::new(content, 0).map_err(|error| report_source_rejection(error, errors))?;
    CompletedParse::try_from(ParseProduct::parse_streaming(
        parser,
        source.input(),
        errors,
    ))?
    .into_recovered()
}

/// A complete producer attempt, with internal failures excluded at its only
/// construction edge. Recovery and error-free parsing consume different policy
/// transitions; neither transition claims model validity.
struct CompletedParse {
    outcome: ParseOutcome<ChatFile>,
    diagnostics: CompletedDiagnostics,
}

impl TryFrom<ParseProduct> for CompletedParse {
    type Error = PipelineError;

    fn try_from(product: ParseProduct) -> Result<Self, Self::Error> {
        match product {
            ParseProduct::Built { file, diagnostics } => Ok(Self {
                outcome: ParseOutcome::Parsed(file),
                diagnostics: CompletedDiagnostics::admit(diagnostics)
                    .map_err(PipelineError::InternalFailure)?,
            }),
            ParseProduct::Unbuildable { diagnostics } => Ok(Self {
                outcome: ParseOutcome::Rejected,
                diagnostics: CompletedDiagnostics::admit(diagnostics)
                    .map_err(PipelineError::InternalFailure)?,
            }),
        }
    }
}

impl CompletedParse {
    fn into_error_free(self) -> Result<ChatFile, PipelineError> {
        let diagnostics = self.diagnostics.into_diagnostics();
        if diagnostics
            .iter()
            .any(|error| error.severity == talkbank_model::Severity::Error)
        {
            return Err(PipelineError::Parse(ParseErrors::from(diagnostics)));
        }
        match self.outcome {
            ParseOutcome::Parsed(file) => Ok(file),
            ParseOutcome::Rejected => Err(PipelineError::Parse(ParseErrors::from(diagnostics))),
        }
    }

    fn into_recovered(self) -> Result<ChatFile, PipelineError> {
        match self.outcome {
            ParseOutcome::Parsed(file) => Ok(file),
            ParseOutcome::Rejected => Err(PipelineError::Parse(ParseErrors::from(
                self.diagnostics.into_diagnostics(),
            ))),
        }
    }
}

/// Preserve the admission owner's diagnostic in both the stream and return value.
fn report_source_rejection(error: FragmentRangeError, errors: &impl ErrorSink) -> PipelineError {
    let diagnostic = error.into_diagnostic();
    errors.report(diagnostic.clone());
    PipelineError::Parse(ParseErrors::from(vec![diagnostic]))
}

/// Compatibility APIs explicitly discard the accepted phase before returning a
/// mutable model. Required parsing and validation still have one proof producer.
fn required_validation(
    parser: &impl ChatParser,
    content: &str,
    policy: ValidationPolicy,
    name: TranscriptName<'_>,
    errors: &impl ErrorSink,
) -> Result<ChatFile, PipelineError> {
    super::validated::parse_validated_with_parser(parser, content, policy, name, errors)
        .map(|accepted| accepted.into_unchecked())
        .map_err(PipelineError::from)
}

#[cfg(test)]
mod tests {
    use super::{PipelineError, parse_and_validate, parse_and_validate_with_parser};
    use talkbank_model::ErrorCode;
    use talkbank_model::ParseValidateOptions;
    use talkbank_parser::TreeSitterParser;
    use talkbank_parser_tests::test_error::ParseProductTestExt;

    #[test]
    fn optional_completion_refuses_internal_failure_in_either_product_shape() {
        use talkbank_model::{
            DiagnosticLocation, ErrorContext, ParseError, ParseProduct, Severity, Span,
        };
        let source = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
        let file = TreeSitterParser::new()
            .expect("grammar")
            .parse_chat_file(source)
            .expect_built();
        let failure = ParseError::new(
            ErrorCode::InternalError,
            Severity::Warning,
            DiagnosticLocation::from_offsets(0, 1),
            ErrorContext::new(source, Span::new(0, 1), source),
            "injected producer fault",
        );
        for product in [
            ParseProduct::Built {
                file,
                diagnostics: vec![failure.clone()],
            },
            ParseProduct::Unbuildable {
                diagnostics: vec![failure],
            },
        ] {
            assert!(matches!(
                super::CompletedParse::try_from(product),
                Err(PipelineError::InternalFailure(_))
            ));
        }
        assert!(matches!(
            super::CompletedParse::try_from(ParseProduct::Unbuildable {
                diagnostics: vec![]
            })
            .expect("no internal failure")
            .into_recovered(),
            Err(PipelineError::Parse(_))
        ));
    }

    #[test]
    fn source_rejection_is_reported_once_as_a_parse_failure() {
        let errors = talkbank_model::ErrorCollector::new();
        let rejection = talkbank_model::FragmentRangeError::check(0, u32::MAX as usize + 1)
            .expect_err("oversized source");
        let PipelineError::Parse(returned) = super::report_source_rejection(rejection, &errors)
        else {
            panic!("source rejection is not parser creation failure");
        };
        let streamed = errors.into_vec();
        assert_eq!(streamed.len(), 1);
        assert_eq!(returned.errors.len(), 1);
        assert_eq!(streamed[0].code, ErrorCode::ParseFailed);
        assert_eq!(
            streamed[0].location,
            talkbank_model::DiagnosticLocation::Unlocated
        );
        assert_eq!(streamed[0].message, returned.errors[0].message);
    }

    #[test]
    fn required_streaming_validation_rejects_errors_with_null_sink() {
        for source in [
            include_str!(
                "../../../../tests/error_corpus/parse_errors/E316_invalid_main_tier_syntax.cha"
            ),
            include_str!(
                "../../../../tests/error_corpus/validation_errors/E552_unlinked_with_wor_timing.cha"
            ),
        ] {
            let result = super::parse_and_validate_streaming(
                source,
                ParseValidateOptions::default().with_alignment(),
                &talkbank_model::NullErrorSink,
            );
            assert!(
                result.is_err(),
                "required validation returned an unchecked model"
            );
        }
    }

    #[test]
    fn test_span_preserved_through_pipeline() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world\n@End\n";

        let options = ParseValidateOptions::default().with_validation();

        match parse_and_validate(content, options) {
            Ok(chat_file) => {
                // Check that main tier has proper span
                let utterance = match chat_file.utterances().next() {
                    Some(utterance) => utterance,
                    None => {
                        return Err(PipelineError::ParserCreation(
                            "Missing utterance in parsed file".to_string(),
                        ));
                    }
                };
                let main_tier = &utterance.main;

                println!(
                    "Main tier span: {}..{}",
                    main_tier.location.span().expect("located").start(),
                    main_tier.location.span().expect("located").end()
                );
                assert_ne!(
                    main_tier.location.span().expect("located").start(),
                    0,
                    "Span should not be 0..0"
                );
                assert_ne!(
                    main_tier.location.span().expect("located").end(),
                    0,
                    "Span should not be 0..0"
                );
            }
            Err(PipelineError::Validation(errors)) => {
                // Should have validation errors (missing terminator)
                println!("Got validation errors (expected):");
                for error in &errors {
                    println!(
                        "  Error: {} at span {}..{}",
                        error.message,
                        error.location.span().expect("located diagnostic").start(),
                        error.location.span().expect("located diagnostic").end()
                    );

                    if error.code == ErrorCode::MissingSpeaker {
                        assert_ne!(
                            error.location.span().expect("located diagnostic").start(),
                            0,
                            "E304 error span should not be 0..0"
                        );
                        assert_ne!(
                            error.location.span().expect("located diagnostic").end(),
                            0,
                            "E304 error span should not be 0..0"
                        );
                    }
                }
            }
            Err(e) => return Err(e),
        }

        Ok(())
    }

    #[test]
    fn test_parse_and_validate_simple() {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();
        let result = parse_and_validate(content, options);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_and_validate_with_validation() -> Result<(), PipelineError> {
        // Use minimal valid CHAT file (validation may require certain headers)
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default().with_validation();
        let result = parse_and_validate(content, options);
        // Validation may find missing required elements, so we just check it doesn't panic
        match result {
            Ok(_) => {}                             // Validation passed
            Err(PipelineError::Validation(_)) => {} // Validation failed as expected for minimal file
            Err(e) => return Err(e),
        }
        Ok(())
    }

    #[test]
    fn test_with_explicit_parser() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();

        let parser = TreeSitterParser::new()
            .map_err(|err| PipelineError::ParserCreation(format!("{:?}", err)))?;
        let result = parse_and_validate_with_parser(&parser, content, options);

        assert!(result.is_ok());

        Ok(())
    }
}
