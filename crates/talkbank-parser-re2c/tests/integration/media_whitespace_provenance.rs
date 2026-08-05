// Test code: the panic-family clippy lints are relaxed by policy.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Both parser front ends must record whitespace before the `@Media` comma.
//!
//! E767 is a VALIDATION rule reading `MediaHeader::whitespace_before_comma`,
//! so it only fires for a front end that actually sets the field. When E767 was
//! introduced it was emitted from the tree-sitter lowering instead, and this
//! parser silently did not report it.
//!
//! Nothing else would have caught that: the equivalence oracle compares the
//! parsed MODELS via `semantic_eq`, never the two parsers' diagnostics, and the
//! provenance field is `#[semantic_eq(skip)]` besides. This test is the gate.

use talkbank_model::model::Header;
use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome};
use talkbank_parser_re2c::Re2cParser;

const WITH_SPACE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n@Media:\tsimple_name , audio\n*CHI:\thello .\n@End\n";

const WITHOUT_SPACE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n@Media:\tsimple_name, audio\n*CHI:\thello .\n@End\n";

fn media_whitespace_recorded(source: &str) -> bool {
    let parser = Re2cParser::new();
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(file) = parser.parse_chat_file(source, 0, &errors) else {
        panic!("the probe source must parse");
    };
    file.lines
        .as_slice()
        .iter()
        .filter_map(|line| match line {
            talkbank_model::model::Line::Header { header, .. } => Some(header),
            _ => None,
        })
        .any(|header| match header.as_ref() {
            Header::Media(media) => media.whitespace_before_comma.is_some(),
            _ => false,
        })
}

/// Surviving category: behaviour a signature cannot describe. Nothing in
/// `MediaHeader` says which parsers populate which of its fields.
#[test]
fn re2c_records_whitespace_before_the_media_comma() {
    assert!(
        media_whitespace_recorded(WITH_SPACE),
        "the re2c front end must record the space so E767 fires for it too"
    );
}

/// The negative case, so the check cannot pass by always reporting.
#[test]
fn re2c_records_nothing_when_the_comma_is_adjacent() {
    assert!(!media_whitespace_recorded(WITHOUT_SPACE));
}
