//! Parse/validate pipeline entry points for CHAT content.
//!
//! This module provides pipeline functions that compose parsing and validation.
//! Most callers should use `parse_and_validate()` or `parse_and_validate_streaming()`.
//! For batch workflows where parser construction overhead matters, use the
//! `_with_parser` variants that accept a caller-provided `TreeSitterParser`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ChatFile;
use talkbank_model::ParseOutcome;
use talkbank_model::ParseValidateOptions;
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{
    ErrorCode, ErrorCollector, ErrorSink, NullErrorSink, ParseError, ParseErrors, Severity,
};
use talkbank_parser::TreeSitterParser;

use super::error::PipelineError;
use talkbank_model::model::TranscriptName;

/// The rule set a `ParseValidateOptions` asks for.
///
/// One owner for the mapping, so the two pipeline entry points below cannot
/// drift into running different rules for the same options.
fn rule_selection(strict_linkers: bool) -> talkbank_model::RuleSelection {
    let rules = talkbank_model::RuleSelection::new();
    if strict_linkers {
        rules.with_strict_linkers()
    } else {
        rules
    }
}

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

/// Parse CHAT content and optionally validate using a caller-provided TreeSitterParser.
///
/// This avoids per-call parser construction, which is useful for batch workflows.
pub fn parse_and_validate_with_parser(
    parser: &TreeSitterParser,
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
    parser: &TreeSitterParser,
    content: &str,
    options: ParseValidateOptions,
    name: TranscriptName<'_>,
) -> Result<ChatFile, PipelineError> {
    if options.validate || options.alignment {
        return required_validation(parser, content, options, name, &NullErrorSink);
    }
    let parse_errors = ErrorCollector::new();

    let chat_file_outcome = parser.parse_chat_file_fragment(content, 0, &parse_errors);

    let parse_error_vec = parse_errors.into_vec();
    let actual_errors: Vec<_> = parse_error_vec
        .iter()
        .filter(|e| e.severity == Severity::Error)
        .cloned()
        .collect();

    if !actual_errors.is_empty() {
        return Err(PipelineError::Parse(ParseErrors {
            errors: parse_error_vec,
        }));
    }

    let chat_file = match chat_file_outcome {
        ParseOutcome::Parsed(chat_file) => chat_file,
        ParseOutcome::Rejected => {
            return Err(PipelineError::ParserCreation(
                "Parser rejected input without reporting errors".to_string(),
            ));
        }
    };

    Ok(chat_file)
}

/// Parse CHAT content and optionally validate with streaming error reporting.
///
/// This is the streaming variant that accepts an ErrorSink for real-time error reporting.
/// Errors are reported immediately as they're discovered, enabling:
/// - Real-time error display in interactive environments
/// - Early cancellation (user can Ctrl+C after seeing first errors)
/// - Memory efficiency (no need to accumulate all errors)
///
/// # Arguments
///
/// * `content` - The CHAT file content as a string
/// * `options` - Parsing and validation options
/// * `errors` - ErrorSink that receives errors as they're discovered
///
/// # Returns
///
/// * `ChatFile` - Always returns a ChatFile (even if there were errors)
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
/// // Errors are in the sink, file is always returned for recovery
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
    parser: &TreeSitterParser,
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
    let parser =
        TreeSitterParser::new().map_err(|e| PipelineError::ParserCreation(format!("{e}")))?;
    parse_and_validate_streaming_named(
        &parser,
        content,
        options,
        errors,
        TranscriptName::for_path(path),
    )
}

/// Streaming variant for a transcript whose name is known.
///
/// See [`parse_and_validate_named`] for why the name is a parameter rather
/// than an `Option` filled in with `None`.
pub fn parse_and_validate_streaming_named(
    parser: &TreeSitterParser,
    content: &str,
    options: ParseValidateOptions,
    errors: &impl ErrorSink,
    name: TranscriptName<'_>,
) -> Result<ChatFile, PipelineError> {
    if options.validate || options.alignment {
        return required_validation(parser, content, options, name, errors);
    }
    let chat_file_outcome = parser.parse_chat_file_fragment(content, 0, errors);

    let chat_file = match chat_file_outcome {
        ParseOutcome::Parsed(chat_file) => chat_file,
        ParseOutcome::Rejected => {
            let parse_error = ParseError::build(ErrorCode::ParseFailed)
                .message("Parser rejected input without reporting errors")
                .finish()
                .map_err(|err| PipelineError::ParserCreation(err.to_string()))?;
            errors.report(parse_error);
            ChatFile::new(vec![])
        }
    };

    Ok(chat_file)
}

/// Compatibility APIs explicitly discard the accepted phase before returning a
/// mutable model. Required parsing and validation still have one proof producer.
fn required_validation(
    parser: &TreeSitterParser,
    content: &str,
    options: ParseValidateOptions,
    name: TranscriptName<'_>,
    errors: &impl ErrorSink,
) -> Result<ChatFile, PipelineError> {
    let alignment = if options.alignment {
        AlignmentValidation::IncludeTierAlignment
    } else {
        AlignmentValidation::Structure
    };
    super::validated::parse_validated_with_parser(
        parser,
        content,
        ValidationPolicy::new(rule_selection(options.strict_linkers), alignment),
        name,
        errors,
    )
    .map(|accepted| accepted.into_unchecked())
    .map_err(|error| match error {
        super::validated::ValidatedParseError::Parse(product) => {
            PipelineError::Parse(ParseErrors::from(product.diagnostics().to_vec()))
        }
        super::validated::ValidatedParseError::Validation(failure) => {
            if failure.has_incomplete_parse() {
                PipelineError::IncompleteValidation(Box::new(failure))
            } else {
                PipelineError::Validation(failure.diagnostics().to_vec())
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{PipelineError, parse_and_validate, parse_and_validate_with_parser};
    use talkbank_model::ErrorCode;
    use talkbank_model::ParseValidateOptions;
    use talkbank_parser::TreeSitterParser;

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
                    main_tier.span.start, main_tier.span.end
                );
                assert_ne!(main_tier.span.start, 0, "Span should not be 0..0");
                assert_ne!(main_tier.span.end, 0, "Span should not be 0..0");
            }
            Err(PipelineError::Validation(errors)) => {
                // Should have validation errors (missing terminator)
                println!("Got validation errors (expected):");
                for error in &errors {
                    println!(
                        "  Error: {} at span {}..{}",
                        error.message, error.location.span.start, error.location.span.end
                    );

                    if error.code == ErrorCode::MissingSpeaker {
                        assert_ne!(
                            error.location.span.start, 0,
                            "E304 error span should not be 0..0"
                        );
                        assert_ne!(
                            error.location.span.end, 0,
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
