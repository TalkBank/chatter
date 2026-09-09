//! `generate_wor_tier`'s timing-copy branch, which no main-tier PARSE reaches.
//!
//! # Why ten tests left
//!
//! They built a `MainTier` by hand for `find_context_dependent_ca_omission_span`,
//! `whole_utterance_language_switch_target` and the `@s` half of `validate`:
//! `Word::new_unchecked("&~dang3", "dang3").with_category(WordCategory::Nonword)`
//! states what the text is AND what the parser would make of it, and nothing
//! checks the second against the first. All ten are now
//! `talkbank-parser-tests/tests/integration/main_tier_methods_from_source.rs`,
//! where each is the utterance as written and the tier is what the parser
//! builds. A `#[cfg(test)] mod` here cannot parse (second instantiation of the
//! crate); an integration test under `tests/` could, and that crate is the
//! cheaper home.
//!
//! # Why these two stay, and what they are evidence of
//!
//! Both set `word.inline_bullet` on a main-tier word and assert
//! `generate_wor_tier` copies it. Measured 2026-09-08: no main-tier PARSE
//! ever sets that field. On a main tier a bullet after a word is a separate
//! `internal_bullet` ITEM; the field is set by the two `%wor` tier parsers, by
//! JSON deserialization, by `Word::with_inline_bullet`, and by any consumer
//! that builds a main tier in memory. The last is the one that matters: a
//! forced-alignment consumer times each main-tier word in memory and then
//! calls `generate_wor_tier` on that tier, so the copy branch is the route by
//! which every machine-produced `%wor` timing is written. No CHAT text
//! reaches it, which is why these two build their input by hand ON PURPOSE,
//! and they are chatter's only pin on that contract. The method's parse-backed
//! coverage is in `talkbank-parser/tests/integration/wor_alignment_regression.rs`.

use crate::Span;
use crate::model::{
    BracketedContent, BracketedItem, Bullet, Group, MainTier, Terminator, UtteranceContent, Word,
};

/// Generates wor tier produces flat words with timing.
#[test]
fn generate_wor_tier_produces_flat_words_with_timing() -> Result<(), String> {
    let mut timed = Word::simple("hello");
    timed.inline_bullet = Some(Bullet::new(100, 200));
    let plain = Word::simple("world");

    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(timed.clone())),
            UtteranceContent::Word(Box::new(plain.clone())),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let wor = main.generate_wor_tier();
    let words: Vec<&Word> = wor.words().collect();
    assert_eq!(words.len(), 2);

    assert_eq!(words[0].cleaned_text(), "hello");
    match &words[0].inline_bullet {
        Some(b) => {
            assert_eq!(b.timing.start_ms, 100);
            assert_eq!(b.timing.end_ms, 200);
        }
        None => return Err("expected inline_bullet on first word".into()),
    }

    assert_eq!(words[1].cleaned_text(), "world");
    assert!(words[1].inline_bullet.is_none());
    Ok(())
}

/// Generates wor tier extracts words from groups.
#[test]
fn generate_wor_tier_extracts_words_from_groups() -> Result<(), String> {
    let mut timed = Word::simple("hello");
    timed.inline_bullet = Some(Bullet::new(50, 150));

    let group = Group::new(BracketedContent::new(vec![BracketedItem::Word(Box::new(
        timed.clone(),
    ))]));

    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Group(group)],
        Terminator::Period { span: Span::DUMMY },
    );

    let wor = main.generate_wor_tier();
    let words: Vec<&Word> = wor.words().collect();
    assert_eq!(words.len(), 1);

    assert_eq!(words[0].cleaned_text(), "hello");
    match &words[0].inline_bullet {
        Some(b) => {
            assert_eq!(b.timing.start_ms, 50);
            assert_eq!(b.timing.end_ms, 150);
        }
        None => return Err("expected inline_bullet on grouped word".into()),
    }
    Ok(())
}
