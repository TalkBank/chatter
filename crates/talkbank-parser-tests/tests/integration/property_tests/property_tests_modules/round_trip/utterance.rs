//! The utterance and file BUILDER API writes the CHAT it means.
//!
//! WIRE-FORMAT tests, and hand-built on purpose: a downstream that builds
//! utterances from its own data needs the writer pinned against the builder
//! route, `Utterance::new` followed by `with_gra`/`with_com` and
//! `ChatFile::new`, which the parsers do not take (they attach dependent
//! tiers through `add_dependent_tier` and build the file through
//! `ChatFile::with_line_map`); `MainTier::new` and the bare `Utterance::new`
//! are the parsers' own calls and every parsed fixture exercises them. The words are
//! proven, not fabricated (`Word::new` over compile-time non-empty
//! literals, since 2026-09-09); the terminator spans are `Span::DUMMY`
//! because a hand-built terminator has no source, and the writer reads no
//! span.

use talkbank_model::Span;
use talkbank_model::model::{
    BulletContent, ChatFile, ComTier, GraTier, GrammaticalRelation, Header, LanguageCode, Line,
    MainTier, Terminator, Utterance, UtteranceContent, Word, WordText,
};
use talkbank_model::non_empty_literal;

/// Verifies a minimal utterance serializes to expected CHAT main-tier text.
#[test]
fn utterance_round_trip_simple() {
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(Word::new(
            non_empty_literal!("hello"),
            WordText::from(non_empty_literal!("hello")),
        )))],
        Terminator::Period { span: Span::DUMMY },
    );
    let utterance = Utterance::new(main);
    let output = utterance.to_chat();
    assert!(
        output.contains("*CHI:\thello ."),
        "Expected main tier in output: {}",
        output
    );
}

/// Verifies utterances with attached dependent tiers serialize all tiers correctly.
#[test]
fn utterance_round_trip_with_dependent_tiers() {
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(Word::new(
            non_empty_literal!("hello"),
            WordText::from(non_empty_literal!("hello")),
        )))],
        Terminator::Period { span: Span::DUMMY },
    );

    let utterance = Utterance::new(main)
        .with_gra(GraTier::new_gra(vec![GrammaticalRelation::new(
            1, 0, "ROOT",
        )]))
        .with_com(ComTier::new(BulletContent::from_text("comment text")));

    let output = utterance.to_chat();
    assert!(output.contains("*CHI:\thello ."), "Expected main tier");
    assert!(output.contains("%gra:\t1|0|ROOT"), "Expected gra tier");
    assert!(output.contains("%com:\tcomment text"), "Expected com tier");
}

/// Verifies a minimal `ChatFile` with headers and one utterance round-trips.
#[test]
fn chat_file_round_trip() {
    let lines = vec![
        Line::header(Header::Utf8),
        Line::header(Header::Languages {
            codes: vec![LanguageCode::new("eng").expect("test literal is non-empty")].into(),
        }),
        Line::utterance(Utterance::new(MainTier::new(
            "CHI",
            vec![UtteranceContent::Word(Box::new(Word::new(
                non_empty_literal!("hello"),
                WordText::from(non_empty_literal!("hello")),
            )))],
            Terminator::Period { span: Span::DUMMY },
        ))),
    ];

    let file = ChatFile::new(lines);
    let output = file.to_chat();

    assert!(output.contains("@UTF8"), "Expected UTF8 header");
    assert!(
        output.contains("@Languages:\teng"),
        "Expected Languages header"
    );
    assert!(output.contains("*CHI:\thello ."), "Expected utterance");
}
