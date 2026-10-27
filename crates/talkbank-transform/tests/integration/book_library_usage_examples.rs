// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Compile + run the code examples shown in
//! `book/src/chatter/integrating/library-usage.md`.
//!
//! The book includes the anchored source below in `rust,ignore` fences because
//! mdbook test can't disambiguate among the workspace's many compiled
//! `libtalkbank_transform-HASH.rlib` candidates in the shared
//! `target/debug/deps/` directory (it lacks Cargo's per-dep `--extern`
//! machinery). Cargo compiles and runs this single source owner with the
//! correct extern wiring; the book does not carry manually mirrored bodies.
//!
//! Keep the book's surrounding explanations consistent when changing these
//! examples. Each test is a library recipe, not an application entrypoint.

// ANCHOR: book_imports
use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{ChatParser, ErrorCollector, RuleSelection};
use talkbank_model::{DependentTier, ErrorSink, ParseError, ParseValidateOptions, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;
use talkbank_transform::json::to_json_pretty_validated;
use talkbank_transform::{
    PipelineError, ValidatedParseError, parse_and_validate, parse_and_validate_with_parser,
    parse_source_with_parser, parse_validated_with_parser,
};
// ANCHOR_END: book_imports

// ANCHOR: book_samples
const SAMPLE_MINIMAL: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n";

const SAMPLE_WITH_MOR: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|||||Target_Child|||
*CHI:\thello world .
%mor:\tco|hello n|world .
@End
";
// ANCHOR_END: book_samples

/// Book example: "Parsing and Validating a CHAT File" (simplest entry).
// ANCHOR: book_parse
#[test]
fn book_parsing_and_validating_simplest() -> Result<(), PipelineError> {
    let source = SAMPLE_MINIMAL.to_owned();
    let options = ParseValidateOptions::default().with_validation();
    let chat_file = parse_and_validate(&source, options)?;

    // These are observable recipe results, not a new validity proof.
    let mut saw_speaker = false;
    for utt in chat_file.utterances() {
        let speaker = format!("{}", utt.main.speaker);
        println!("Speaker: {speaker}");
        assert!(!speaker.is_empty(), "speaker code should render non-empty");
        saw_speaker = true;
    }
    assert!(
        saw_speaker,
        "minimal sample should contain at least one utterance"
    );
    Ok(())
}
// ANCHOR_END: book_parse

/// Book example: "Parsing and Validating a CHAT File" (batch with reused parser).
// ANCHOR: book_reuse_parser
#[test]
fn book_parsing_and_validating_with_parser() -> Result<(), Box<dyn std::error::Error>> {
    fn batch(parser: &impl ChatParser) -> Result<(), PipelineError> {
        let options = ParseValidateOptions::default().with_validation();
        for source in [SAMPLE_MINIMAL, SAMPLE_WITH_MOR] {
            let chat_file = parse_and_validate_with_parser(parser, source, options.clone())?;
            assert!(chat_file.utterances().next().is_some());
        }
        Ok(())
    }
    batch(&TreeSitterParser::new()?)?;
    batch(&Re2cParser::new())?;
    Ok(())
}
// ANCHOR_END: book_reuse_parser

/// Book example: "Working with the Model", top-level participants,
/// dependent-tier iteration, MorTier::items() accessor.
// ANCHOR: book_model
#[test]
fn book_working_with_the_model() -> Result<(), PipelineError> {
    let chat_file = parse_and_validate(
        SAMPLE_WITH_MOR,
        ParseValidateOptions::default().with_validation(),
    )?;

    let _participants = chat_file.all_participants();
    assert!(
        !chat_file.all_participants().is_empty(),
        "sample should yield at least one participant"
    );

    let mut saw_mor_item = false;
    for utt in chat_file.utterances() {
        for entry in &utt.dependent_tiers {
            if let DependentTier::Mor(mor_tier) = &entry.tier {
                for item in mor_tier.items() {
                    let _pos = format!("{}", item.main.pos);
                    let _lemma = format!("{}", item.main.lemma);
                    saw_mor_item = true;
                }
            }
        }
    }
    assert!(saw_mor_item, "SAMPLE_WITH_MOR should expose mor items");
    Ok(())
}
// ANCHOR_END: book_model

/// Book example: "Serializing to CHAT", `WriteChat::to_chat_string()`
/// convenience + the streaming `write_chat(&mut output)` form.
// ANCHOR: book_chat
#[test]
fn book_serializing_to_chat() -> Result<(), Box<dyn std::error::Error>> {
    let chat_file = parse_and_validate(
        SAMPLE_MINIMAL,
        ParseValidateOptions::default().with_validation(),
    )?;

    let chat_text = chat_file.to_chat_string();
    assert!(chat_text.starts_with("@UTF8"));

    let mut output = String::new();
    chat_file.write_chat(&mut output)?;
    assert!(output.starts_with("@UTF8"));
    Ok(())
}
// ANCHOR_END: book_chat

/// Book example: "Serializing to JSON" via the schema-validated helper.
// ANCHOR: book_json
#[test]
fn book_serializing_to_json() -> Result<(), Box<dyn std::error::Error>> {
    let chat_file = parse_and_validate(
        SAMPLE_MINIMAL,
        ParseValidateOptions::default().with_validation(),
    )?;
    let json = to_json_pretty_validated(&chat_file)?;
    assert!(json.contains("\"speaker\""));
    Ok(())
}
// ANCHOR_END: book_json

/// Book example: "Custom Error Handling", implementing the
/// `ErrorSink` trait against the published `ParseError` shape.
///
/// SURVIVES: wire format, in the documentation sense. The assertion is that it
/// COMPILES: a reader copying this out of the book gets code that builds
/// against the current public API. A change to `ErrorSink`'s shape breaks this
/// file, which is the point, and no runtime assertion would add to it.
// ANCHOR: book_sink
#[test]
fn book_custom_error_handling() {
    struct MyErrorHandler;

    impl ErrorSink for MyErrorHandler {
        fn report(&self, _error: ParseError) {
            // This interface-only recipe deliberately discards the diagnostic.
        }
    }

    let _: &dyn ErrorSink = &MyErrorHandler;
}
// ANCHOR_END: book_sink

/// Both backends produce the same retained admission capability.
// ANCHOR: book_peer_admission
#[test]
fn book_checked_admission_with_either_backend() -> Result<(), Box<dyn std::error::Error>> {
    fn check(parser: &impl ChatParser) -> Result<(), ValidatedParseError> {
        let errors = ErrorCollector::new();
        let admitted = parse_validated_with_parser(
            parser,
            SAMPLE_MINIMAL,
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            TranscriptName::Anonymous,
            &errors,
        )?;
        assert_eq!(admitted.document().utterances().count(), 1);
        // Keep this capability while validity is required. Editing requires
        // consuming it with into_unchecked() and validating again.
        for diagnostic in errors.into_vec() {
            eprintln!("{:?}", diagnostic);
        }
        Ok(())
    }
    check(&TreeSitterParser::new()?)?;
    check(&Re2cParser::new())?;
    Ok(())
}
// ANCHOR_END: book_peer_admission

/// Source admission preserves bytes; malformed source stays explicit refusal.
// ANCHOR: book_source_admission
#[test]
fn book_source_admission_and_refusal() -> Result<(), Box<dyn std::error::Error>> {
    fn check(parser: &impl ChatParser) -> Result<(), ValidatedParseError> {
        let diagnostics = ErrorCollector::new();
        let admitted = parse_source_with_parser(parser, SAMPLE_MINIMAL)
            .admit(TranscriptName::Anonymous, &diagnostics)?;
        assert_eq!(admitted.source(), SAMPLE_MINIMAL);

        let malformed = SAMPLE_MINIMAL.replace("*CHI:", "CHI:");
        let rejected_diagnostics = ErrorCollector::new();
        let outcome = parse_source_with_parser(parser, &malformed)
            .admit(TranscriptName::Anonymous, &rejected_diagnostics);
        match outcome {
            Err(ValidatedParseError::Parse(product)) => {
                assert!(product.has_error_diagnostics());
                assert_eq!(product.diagnostics(), rejected_diagnostics.into_vec());
            }
            Err(other) => return Err(other),
            Ok(_) => panic!("malformed source must not gain admission"),
        }
        Ok(())
    }
    check(&TreeSitterParser::new()?)?;
    check(&Re2cParser::new())?;
    Ok(())
}
// ANCHOR_END: book_source_admission
