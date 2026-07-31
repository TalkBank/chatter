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

//! Tests for `ChatFile::utterance_containing`.
//!
//! This lives here rather than in `talkbank-model` because the model crate
//! deliberately has no dev-dependency on `talkbank-parser` (parser-on-model
//! would invert the workspace's layering), and this crate exists precisely
//! to hold tests that need both a real parse and the typed model.
//!
//! Uses `TreeSitterParser`'s inherent whole-file `parse_chat_file(&str)`
//! (see `wor_terminator_alignment.rs` for the same idiom) rather than the
//! `ChatParser` trait method of the same name: the trait method additionally
//! takes an offset and an `ErrorSink`, and an inherent method of the same
//! name always shadows a trait method in Rust's method resolution, so the
//! two cannot be told apart by argument list on a bare `parser.parse_chat_file(...)`
//! dot-call.

use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Builds the parser used for containment assertions.
fn make_parser() -> Result<TreeSitterParser, TestError> {
    TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))
}

/// Half-open containment: an offset exactly at one utterance's end
/// belongs to the NEXT utterance, never to both.
#[test]
fn utterance_containment_is_half_open_at_the_boundary() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n*CHI:\thi .\n*MOT:\tbye .\n@End\n";
    let parser = make_parser()?;
    let file = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(source))?;

    let first = file
        .utterances()
        .next()
        .ok_or_else(|| TestError::Failure("no utterances".to_string()))?;
    let end = first.main.span.end;

    let at_end = file
        .utterance_containing(end)
        .ok_or_else(|| TestError::Failure("nothing at boundary".to_string()))?;
    assert_ne!(
        at_end.main.span, first.main.span,
        "offset at the first utterance's exclusive end must not resolve to it"
    );
    Ok(())
}

/// An offset strictly inside an utterance's main tier resolves to it.
#[test]
fn utterance_containing_finds_an_interior_offset() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n*CHI:\thi .\n@End\n";
    let parser = make_parser()?;
    let file = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(source))?;

    let first = file
        .utterances()
        .next()
        .ok_or_else(|| TestError::Failure("no utterances".to_string()))?;
    let interior = first.main.span.start + 1;
    assert!(file.utterance_containing(interior).is_some());
    Ok(())
}

/// An offset with no covering utterance (past end of file) resolves to
/// nothing, never to a dummy-spanned utterance.
#[test]
fn utterance_containing_returns_none_past_the_last_utterance() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n*CHI:\thi .\n@End\n";
    let parser = make_parser()?;
    let file = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(source))?;

    assert!(file.utterance_containing(source.len() as u32).is_none());
    Ok(())
}
