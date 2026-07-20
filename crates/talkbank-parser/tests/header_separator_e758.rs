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

//! Pins that `Line::Header` captures the `header_sep`'s illegal
//! trailing-space span (E758 provenance) via `TierSeparator`, mirroring the
//! dependent-tier analog (`dependent_tier_separator` in
//! `dependent_tier_dispatch/helpers.rs`).
//!
//! This is a parser-model test only: it asserts what
//! `TreeSitterParser::parse_chat_file_streaming` records on `Line::Header`,
//! not what `chatter validate` reports (the E758-from-separator validation
//! rewrite is a later task per
//! `docs/superpowers/plans/2026-07-18-tier-separator-e758.md` Task 5).

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line, TierSeparator};
use talkbank_parser::TreeSitterParser;

/// A minimal valid CHAT file whose `@Comment` header has a CLEAN separator:
/// exactly one tab between the colon and the content, no trailing spaces.
const COMMENT_HEADER_CLEAN: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Comment:\tclean comment
*CHI:\thello .
@End
";

/// The same file, but the `@Comment` header's separator has one illegal
/// space after the required tab (`@Comment:<TAB><SPACE>text`). The grammar's
/// `header_sep` rule (`seq(colon, tab, optional(sep_trailing_space))`) grabs
/// that space into a `sep_trailing_space` node rather than the content, so
/// the header still parses successfully; only the separator's provenance
/// records the illegal space.
const COMMENT_HEADER_TRAILING_SPACE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Comment:\t trailing space comment
*CHI:\thello .
@End
";

/// Parse `input` and return the `separator` of its (single) `Header::Comment`
/// line, panicking if the fixture does not contain exactly one.
fn comment_header_separator(input: &str) -> TierSeparator {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);

    let mut found = None;
    for line in &chat.lines.0 {
        if let Line::Header {
            header, separator, ..
        } = line
            && matches!(header.as_ref(), Header::Comment { .. })
        {
            assert!(
                found.is_none(),
                "fixture must contain exactly one @Comment header"
            );
            found = Some(*separator);
        }
    }
    found.expect("fixture must contain an @Comment header")
}

/// A clean `@Comment:<TAB>text` separator has no trailing-space span.
#[test]
fn header_separator_clean_has_no_trailing_space() {
    let separator = comment_header_separator(COMMENT_HEADER_CLEAN);
    assert!(
        separator.trailing_space().is_none(),
        "clean header_sep must not carry a trailing-space span, got {separator:?}"
    );
}

/// A `@Comment:<TAB><SPACE>text` separator records the illegal space's span.
#[test]
fn header_separator_trailing_space_is_captured() {
    let separator = comment_header_separator(COMMENT_HEADER_TRAILING_SPACE);
    let span = separator
        .trailing_space()
        .expect("header_sep with an illegal trailing space must record its span");

    // The trailing-space span must be non-empty (it covers the one illegal
    // space byte) and must point at the source text that is actually a
    // space, not the tab or the following content.
    assert!(
        span.start < span.end,
        "trailing-space span must be non-empty, got {span:?}"
    );
    let illegal_bytes =
        &COMMENT_HEADER_TRAILING_SPACE.as_bytes()[span.start as usize..span.end as usize];
    assert_eq!(
        illegal_bytes, b" ",
        "trailing-space span must cover exactly the illegal space byte(s)"
    );
}
