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

//! What survives inside a bracketed construct: the contents of an angle
//! group, a quotation, a pho group and a sin group are the one `contents`
//! grammar rule, and every item that rule admits (a word, an overlap
//! marker, a separator, a nested group) is kept and written back byte for
//! byte.
//!
//! # Why this table exists
//!
//! Until 2026-09-08 the bracketed constructs walked their `contents`
//! through a hand-written dispatcher (`group/nested.rs`) that handed each
//! child to a second walk over the CHILD's children. An `overlap_point` is
//! a single token with no children, so `<hello ⌈ there ⌉> [/] hello there .`
//! parsed clean, validated clean, and wrote back as `<hello there> [/]
//! hello there .`: both overlap markers were dropped from the model, and no
//! test noticed because the construct tests assert a clean parse and the
//! spec examples never put an overlap marker inside a group. The typed
//! walker keeps them; these rows are the pin.
//!
//! Every fixture here is valid CHAT (`parsed(&[])`), so the byte-exact
//! roundtrip is the whole assertion: a construct that dropped an item would
//! write back shorter.

use talkbank_model::model::{BracketedItem, ChatFile, UtteranceContent, WriteChat};
use talkbank_parser_tests::from_source::{Media, SingleSpeaker};
use talkbank_parser_tests::test_error::TestError;

/// The file the parser builds for one valid utterance, and the source it
/// was built from.
fn parsed(utterance: &str) -> Result<(ChatFile, String), TestError> {
    let lines = format!("*CHI:\t{utterance}");
    let fixture = SingleSpeaker {
        languages: "eng",
        options: None,
        media: Media::Undeclared,
        lines: &lines,
    };
    Ok((fixture.parsed(&[])?, fixture.source()))
}

/// The items inside the first bracketed construct of the utterance.
fn first_bracketed_items(file: &ChatFile) -> Vec<BracketedItem> {
    let utterance = file.utterances().next().expect("one utterance");
    utterance
        .main
        .content
        .content
        .iter()
        .find_map(|item| match item {
            UtteranceContent::Retrace(retrace) => Some(retrace.content.content.to_vec()),
            UtteranceContent::Group(group) => Some(group.content.content.to_vec()),
            UtteranceContent::Quotation(quotation) => Some(quotation.content.content.to_vec()),
            UtteranceContent::PhoGroup(pho) => Some(pho.content.content.to_vec()),
            UtteranceContent::SinGroup(sin) => Some(sin.content.content.to_vec()),
            _ => None,
        })
        .expect("a bracketed construct")
}

/// Each row: the utterance, and the kinds of item its first construct holds.
const ROWS: &[(&str, &[&str])] = &[
    (
        "<hello \u{2308} there \u{2309}> [/] hello there .",
        &["word", "overlap_point", "word", "overlap_point"],
    ),
    (
        "\u{201c}hello \u{2308} there \u{2309}\u{201d} .",
        &["word", "overlap_point", "word", "overlap_point"],
    ),
    (
        "\u{2039}hello \u{2308} there \u{2309}\u{203a} .",
        &["word", "overlap_point", "word", "overlap_point"],
    ),
    (
        "\u{3014}hello \u{2308} there \u{2309}\u{3015} .",
        &["word", "overlap_point", "word", "overlap_point"],
    ),
    (
        "<hello , there> [/] hello there .",
        &["word", "separator", "word"],
    ),
    (
        "<hello <there> [/] there> [//] hello there .",
        &["word", "retrace", "word"],
    ),
];

/// The kind of one bracketed item, for the row table.
fn kind(item: &BracketedItem) -> &'static str {
    match item {
        BracketedItem::Word(_) => "word",
        BracketedItem::OverlapPoint(_) => "overlap_point",
        BracketedItem::Separator(_) => "separator",
        BracketedItem::Retrace(_) => "retrace",
        _ => "other",
    }
}

/// Every item the grammar admits inside a construct is in the model, in
/// order, and the file writes back byte for byte.
#[test]
fn bracketed_contents_keep_every_item_and_roundtrip() -> Result<(), TestError> {
    for (utterance, expected) in ROWS {
        let (file, source) = parsed(utterance)?;
        let items = first_bracketed_items(&file);
        let kinds: Vec<&str> = items.iter().map(kind).collect();
        assert_eq!(
            &kinds, expected,
            "items inside the construct of {utterance:?}"
        );
        assert_eq!(
            file.to_chat_string(),
            source,
            "byte-exact roundtrip of {utterance:?}"
        );
    }
    Ok(())
}
