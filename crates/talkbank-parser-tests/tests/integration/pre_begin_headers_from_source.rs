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
//! What a pre-`@Begin` header with NO value lowers to.
//!
//! `@Window`, `@Color words` and `@Font` are `seq(prefix, header_sep,
//! free_text, newline)`, and a line with nothing after the tab leaves the
//! `free_text` position present with zero-width text and a MISSING
//! placeholder elsewhere on the line, E342; the header it lowers to is
//! unknown, E525 at validation (both measured through the parsed verdict
//! these rows carry). Until 2026-09-09 the pre-begin handler read that position by
//! index, the placeholder's empty text decoded, and the file carried a
//! `Header::Window { geometry: "" }` nobody wrote and no diagnostic named:
//! a fabricated value. The typed position is read as the recovery state it
//! is, and the header lowers as `Header::Unknown`, which is what these rows
//! pin. `@PID` took the same step a day earlier and is pinned alongside.

use talkbank_model::model::{Header, Line};
use talkbank_parser_tests::from_source::{Rules, parsed_document};
use talkbank_parser_tests::test_error::TestError;

/// The one header line the fixture carries between `@UTF8` and `@Begin`.
fn pre_begin_header(line: &str) -> Result<Header, TestError> {
    let source = format!(
        "@UTF8\n{line}\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n"
    );
    let file = parsed_document(&source, &["E342", "E525"], Rules::Default)?;
    file.lines
        .into_iter()
        .filter_map(|entry| match entry {
            Line::Header { header, .. } => Some(*header),
            _ => None,
        })
        .nth(1)
        .ok_or_else(|| TestError::Failure(format!("{line:?} built no second header")))
}

/// A pre-begin header with no value is a recovered, unknown header, never a
/// header carrying an empty value.
#[test]
fn a_pre_begin_header_with_no_value_lowers_as_unknown() -> Result<(), TestError> {
    let mut wrong = Vec::new();
    for line in ["@Window:\t", "@Color words:\t", "@Font:\t", "@PID:\t"] {
        match pre_begin_header(line)? {
            Header::Unknown { .. } => {}
            other => wrong.push(format!("{line:?} lowered as {other:?}")),
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    Ok(())
}
