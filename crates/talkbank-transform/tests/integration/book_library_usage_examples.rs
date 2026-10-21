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
use talkbank_model::{DependentTier, ErrorSink, ParseError, ParseValidateOptions, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::json::to_json_pretty_validated;
use talkbank_transform::{PipelineError, parse_and_validate, parse_and_validate_with_parser};
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
    let parser = TreeSitterParser::new()?;
    let options = ParseValidateOptions::default().with_validation();
    let chat_files: Vec<&str> = vec![SAMPLE_MINIMAL, SAMPLE_WITH_MOR];

    for source in &chat_files {
        let chat_file = parse_and_validate_with_parser(&parser, source, options.clone())?;
        let _ = chat_file;
    }
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

    let _participants = &chat_file.participants;
    assert!(
        !chat_file.participants.is_empty(),
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
