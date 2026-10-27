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

//! Golden tests for main tier parsing.
//!
//! Test inputs from this repo:
//! - `spec/constructs/main_tier/`
//! - `corpus/reference/content/`
//! - `grammar/test/corpus/main_tier/`

use talkbank_model::{Terminator, WordCategory};
use talkbank_parser_re2c::ast::*;
use talkbank_parser_re2c::parser;
use talkbank_parser_re2c::token::Token;

// ── Simple utterances ───────────────────────────────────────────

#[test]
fn simple_utterance() {
    let mt = parser::parse_main_tier("*CHI:\thello .\n").unwrap();
    assert!(matches!(mt.speaker, Token::Speaker("CHI")));
    assert_eq!(mt.tier_body.contents.len(), 1);
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::Period { .. })
    ));
}

#[test]
fn multi_word() {
    let mt = parser::parse_main_tier("*CHI:\thello world .\n").unwrap();
    assert_eq!(mt.tier_body.contents.len(), 2);
}

// ── Terminators ─────────────────────────────────────────────────

#[test]
fn terminator_period() {
    let mt = parser::parse_main_tier("*CHI:\tI see a cat .\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::Period { .. })
    ));
}

#[test]
fn terminator_question() {
    let mt = parser::parse_main_tier("*MOT:\twhere is the cat ?\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::Question { .. })
    ));
}

#[test]
fn terminator_exclamation() {
    let mt = parser::parse_main_tier("*CHI:\twow !\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::Exclamation { .. })
    ));
}

#[test]
fn terminator_trailing_off() {
    let mt = parser::parse_main_tier("*CHI:\tI was going to the +...\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::TrailingOff { .. })
    ));
}

#[test]
fn terminator_interruption() {
    let mt = parser::parse_main_tier("*MOT:\twhat do they do with +/.\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::Interruption { .. })
    ));
}

#[test]
fn terminator_self_interruption() {
    let mt = parser::parse_main_tier("*MOT:\tand then she wouldn't +//.\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::SelfInterruption { .. })
    ));
}

#[test]
fn terminator_break_for_coding() {
    let mt = parser::parse_main_tier("*EXP:\tfoo bar +.\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::BreakForCoding { .. })
    ));
}

#[test]
fn terminator_interrupted_question() {
    let mt = parser::parse_main_tier("*MOT:\twhat would they have in a +/?\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::InterruptedQuestion { .. })
    ));
}

#[test]
fn terminator_self_interrupted_question() {
    let mt = parser::parse_main_tier("*MOT:\twhat were you going to say about +//?\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::SelfInterruptedQuestion { .. })
    ));
}

#[test]
fn terminator_trailing_off_question() {
    let mt = parser::parse_main_tier("*MOT:\tanything to belong to the club or +..?\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::TrailingOffQuestion { .. })
    ));
}

#[test]
fn terminator_broken_question() {
    let mt = parser::parse_main_tier("*MOT:\tbut what if they +!?\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::BrokenQuestion { .. })
    ));
}

#[test]
fn terminator_quoted_new_line() {
    let mt = parser::parse_main_tier("*CHI:\tthe bear said +\"/.\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::QuotedNewLine { .. })
    ));
}

#[test]
fn terminator_quoted_period_simple() {
    let mt = parser::parse_main_tier("*CHI:\tthe bear said +\".\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::QuotedPeriodSimple { .. })
    ));
}

#[test]
fn terminator_optional() {
    // Terminator is optional per grammar.js
    let mt = parser::parse_main_tier("*SPK:\trising to high \u{21D7}\n").unwrap();
    assert!(!mt.tier_body.contents.is_empty());
}

// ── Word features ───────────────────────────────────────────────

#[test]
fn compound_word() {
    let mt = parser::parse_main_tier("*CHI:\tI want ice+cream .\n").unwrap();
    let words: Vec<_> = mt
        .tier_body
        .contents
        .iter()
        .filter_map(|c| match c {
            ContentItem::Word(w) => Some(w),
            _ => None,
        })
        .collect();
    assert_eq!(words.len(), 3);
    assert!(
        words[2]
            .body()
            .iter()
            .any(|b| matches!(b, WordBodyItem::CompoundMarker(_)))
    );
}

#[test]
fn word_with_form_marker() {
    let mt = parser::parse_main_tier("*MOT:\tmama@f is here .\n").unwrap();
    let words: Vec<_> = mt
        .tier_body
        .contents
        .iter()
        .filter_map(|c| match c {
            ContentItem::Word(w) => Some(w),
            _ => None,
        })
        .collect();
    assert!(
        words[0].form_marker().is_some(),
        "expected form marker on first word"
    );
}

#[test]
fn word_with_lengthening() {
    let mt = parser::parse_main_tier("*CHI:\tno:: .\n").unwrap();
    let words: Vec<_> = mt
        .tier_body
        .contents
        .iter()
        .filter_map(|c| match c {
            ContentItem::Word(w) => Some(w),
            _ => None,
        })
        .collect();
    assert!(
        words[0]
            .body()
            .iter()
            .any(|b| matches!(b, WordBodyItem::Lengthening(run) if run.count().get() == 2))
    );
}

#[test]
fn filler_word() {
    let mt = parser::parse_main_tier("*CHI:\t&-um I want cookies .\n").unwrap();
    let words: Vec<_> = mt
        .tier_body
        .contents
        .iter()
        .filter_map(|c| match c {
            ContentItem::Word(w) => Some(w),
            _ => None,
        })
        .collect();
    assert!(
        matches!(words[0].category(), Some(WordCategory::Filler)),
        "expected filler category on first word"
    );
}

#[test]
fn event_happening() {
    let mt = parser::parse_main_tier("*CHI:\t&=laughs .\n").unwrap();
    let has_event = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, ContentItem::Event(..)));
    assert!(has_event, "got {:?}", mt.tier_body.contents);
}

// ── Annotations ─────────────────────────────────────────────────

#[test]
fn retracing_annotation() {
    let mt = parser::parse_main_tier("*CHI:\tthe the [/] dog .\n").unwrap();
    let has_retrace = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, ContentItem::Retrace(_)));
    assert!(has_retrace, "got {:?}", mt.tier_body.contents);
}

#[test]
fn explanation_annotation() {
    let mt = parser::parse_main_tier("*CHI:\tit [= the cookie] .\n").unwrap();
    let has = mt.tier_body.contents.iter().any(|c| match c {
        ContentItem::Word(w) => w.annotations().iter().any(|a| {
            matches!(
                a.annotation(),
                talkbank_model::ContentAnnotation::Explanation(_)
            )
        }),
        _ => false,
    });
    assert!(has, "got {:?}", mt.tier_body.contents);
}

#[test]
fn pause_in_utterance() {
    let mt = parser::parse_main_tier("*CHI:\tI (.) want cookies .\n").unwrap();
    let has_pause = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, ContentItem::Pause(_)));
    assert!(has_pause, "got {:?}", mt.tier_body.contents);
}

// ── Linkers ─────────────────────────────────────────────────────

#[test]
fn linker_admission_preserves_reference_kinds_order_and_source_locations() {
    use talkbank_model::{ChatParser, ErrorCollector, WriteChat};
    let parser = talkbank_parser_re2c::Re2cParser::new();
    let mut spellings = std::collections::BTreeSet::new();
    for source in [
        include_str!("../../../../corpus/reference/content/linkers.cha"),
        include_str!("../../../../corpus/reference/content/linkers-multiple.cha"),
        include_str!("../../../../corpus/reference/ca/intonation.cha"),
    ] {
        for origin in [0, 37] {
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(source, origin, &errors)
                .into_option()
                .unwrap();
            assert!(!errors.has_errors(), "{:?}", errors.to_vec());
            for utterance in file
                .utterances()
                .filter(|utterance| !utterance.main.content.linkers.is_empty())
            {
                let main = &utterance.main;
                let span = main.location.span().unwrap();
                let input = &source[span.start() - origin..span.end() - origin];
                let errors = ErrorCollector::new();
                let fragment = parser
                    .parse_main_tier(input, span.start(), &errors)
                    .into_option()
                    .unwrap();
                assert!(!errors.has_errors(), "{:?}", errors.to_vec());
                assert_eq!(fragment.content.linkers, main.content.linkers);
                for linker in &main.content.linkers {
                    let span = linker.location.span().unwrap();
                    let spelling = linker.to_chat_string();
                    assert_eq!(
                        &source[span.start() - origin..span.end() - origin],
                        spelling
                    );
                    spellings.insert(spelling);
                }
            }
        }
    }
    assert_eq!(
        spellings,
        ["+<", "++", "+^", "+\"", "+,", "+≈", "+≋"]
            .into_iter()
            .map(String::from)
            .collect()
    );
}

// ── Postcodes ───────────────────────────────────────────────────

#[test]
fn postcodes_after_terminator() {
    let mt = parser::parse_main_tier("*CHI:\thello . [+ bch] [+ foo]\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Terminator::Period { .. })
    ));
    assert_eq!(
        mt.tier_body.postcodes.len(),
        2,
        "got {:?}",
        mt.tier_body.postcodes
    );
}

// ── Groups ──────────────────────────────────────────────────────

#[test]
fn angle_bracket_group() {
    // <I want> [//] is a group retrace (complete), not a plain group
    let mt = parser::parse_main_tier("*CHI:\t<I want> [//] I need cookies .\n").unwrap();
    let has_retrace = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, ContentItem::Retrace(r) if r.is_group()));
    assert!(has_retrace, "got {:?}", mt.tier_body.contents);
}
