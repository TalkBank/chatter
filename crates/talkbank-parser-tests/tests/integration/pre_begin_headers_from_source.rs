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
//! `@Window`, `@Color words`, `@Font` and `@PID` are `seq(prefix,
//! header_sep, optional(free_text), newline)`: a line with nothing after the
//! tab parses cleanly and lowers to the one empty-content state, refused by
//! validation as E553 alone (measured through the parsed verdict these rows
//! carry). Until 2026-09-09 the pre-begin handler read the text position by
//! index and the file carried a `Header::Window { geometry: "" }` nobody
//! wrote; until 2026-10-31 the required position made recovery insert a
//! MISSING placeholder (E342) and the header lowered as unknown (E525).

use talkbank_model::model::{Header, HeaderKind, Line, PlainKind, TextKind};
use talkbank_parser_tests::from_source::{Rules, parsed_document};
use talkbank_parser_tests::test_error::TestError;

/// The one header line the fixture carries between `@UTF8` and `@Begin`.
fn pre_begin_header(line: &str) -> Result<Header, TestError> {
    let source = format!(
        "@UTF8\n{line}\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n"
    );
    let file = parsed_document(&source, &["E553"], Rules::Default)?;
    file.lines
        .into_iter()
        .filter_map(|entry| match entry {
            Line::Header { header, .. } => Some(*header),
            _ => None,
        })
        .nth(1)
        .ok_or_else(|| TestError::Failure(format!("{line:?} built no second header")))
}

/// A pre-begin header with no value is empty content, never a header
/// carrying an empty value.
#[test]
fn a_pre_begin_header_with_no_value_is_empty_content() -> Result<(), TestError> {
    let mut wrong = Vec::new();
    for (line, kind) in [
        ("@Window:\t", TextKind::Window),
        ("@Color words:\t", TextKind::ColorWords),
        ("@Font:\t", TextKind::Font),
        ("@PID:\t", TextKind::Pid),
    ] {
        match pre_begin_header(line)? {
            Header::EmptyContent(found)
                if found.kind() == HeaderKind::Plain(PlainKind::Text(kind)) => {}
            other => wrong.push(format!("{line:?} lowered as {other:?}")),
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    Ok(())
}
