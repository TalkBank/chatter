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

//! `MainTier` methods, called on a main tier the parser built.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `model/content/main_tier/tests.rs` built a `MainTier`
//! by hand for twelve tests; the ten testing
//! `find_context_dependent_ca_omission_span`,
//! `whole_utterance_language_switch_target` and the `@s` half of `validate`
//! moved here.
//! The hand-built tier stated the input twice: `Word::new_unchecked("&~dang3",
//! "dang3").with_category(WordCategory::Nonword)` says what the text is AND
//! what the parser would make of it, and nothing checks the second against
//! the first. Here each case is the utterance as written, and the tier is
//! whatever the parser builds from it.
//!
//! # Why it moved crates, stated accurately
//!
//! A `#[cfg(test)] mod` in `src/` cannot parse (the lib-test target is a
//! second instantiation of the crate); an integration test under `tests/`
//! with a dev-dependency cycle could, as `talkbank-derive` does. This crate is
//! the cheaper home: it already depends on the tree-sitter parser.
//!
//! # These are METHOD RESULTS, not diagnostics
//!
//! So there is no single table with a code column. Each test asks one public
//! method one question and checks its answer, with the language context the
//! original supplied. Spans are compared to the parsed word's own span rather
//! than to a number the test wrote.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{BracketedItem, LanguageCode, MainTier, UtteranceContent, Word};
use talkbank_model::validation::{Validate, ValidationContext};
use talkbank_parser_tests::from_source::{Media, SingleSpeaker};
use talkbank_parser_tests::test_error::TestError;

/// The main tier the parser builds for one utterance, after the whole
/// fixture has been checked to report exactly `expected` (`&[]`: valid CHAT).
///
/// Some fixtures here are invalid ON PURPOSE (`hola@s amiga@s .` exists to be
/// E255), and each such row names its code, so a fixture cannot be invalid
/// by accident: the review of the first draft found exactly that shape.
fn main_tier(utterance: &str, languages: &str, expected: &[&str]) -> Result<MainTier, TestError> {
    let lines = format!("*CHI:\t{utterance}");
    SingleSpeaker {
        languages,
        options: None,
        media: Media::Undeclared,
        lines: &lines,
    }
    .main_tier(expected)
}

/// The same for an utterance carrying a bullet, which needs the `@Media`
/// header its timestamp indexes (E752 without one, since 2026-09-08).
fn timed_main_tier(utterance: &str) -> Result<MainTier, TestError> {
    let lines = format!("*CHI:\t{utterance}");
    SingleSpeaker {
        languages: "eng",
        options: None,
        media: Media::Declared,
        lines: &lines,
    }
    .main_tier(&[])
}

/// The same, in CA mode: `(word)` is an omission only under `@Options: CA`.
fn main_tier_ca(utterance: &str, expected: &[&str]) -> Result<MainTier, TestError> {
    let lines = format!("*CHI:\t{utterance}");
    SingleSpeaker {
        languages: "eng",
        options: Some("CA"),
        media: Media::Undeclared,
        lines: &lines,
    }
    .main_tier(expected)
}

fn codes(langs: &[&str]) -> Vec<LanguageCode> {
    langs
        .iter()
        .map(|code| LanguageCode::new(code).expect("test literal is non-empty"))
        .collect()
}

// ── generate_wor_tier ───────────────────────────────────────────────────
//
// What the migration found here. The originals set `word.inline_bullet` on
// main-tier words and asserted `generate_wor_tier` copied it. No main-tier
// PARSE ever sets that field: on a main tier a bullet after a word is a
// separate `internal_bullet` ITEM. The field is set by the two `%wor` tier
// parsers, by JSON, by `Word::with_inline_bullet`, and by a consumer that
// times a main tier in memory before projecting it, which is how forced
// alignment writes every `%wor` tier. So the copy branch is a library
// contract no CHAT text reaches, and the originals stay in `talkbank-model`
// as its only pin, saying so. These assert what a PARSED main tier yields.
// The method's real-fixture coverage lives in
// `talkbank-parser/tests/integration/wor_alignment_regression.rs`.

/// A parsed main tier's words come out flat, and timing after a word is not
/// on the word.
///
/// The second assertion PINS CURRENT BEHAVIOUR rather than a ruled design: a
/// bullet the transcript places after a word is an `internal_bullet` item, so
/// the generated `%wor` carries none of it. That is information loss on the
/// roundtripped representation (a `%wor` written from timed words and read
/// back does not rebuild timed words), and nobody has adjudicated whether
/// `generate_wor_tier` should read the adjacent item. If that is changed, this
/// assertion is the one to delete, not to satisfy.
///
/// Also found while writing the fixture: until 2026-09-08 a bullet INSIDE
/// the main tier was timing evidence for neither E752 nor E544, so this
/// fixture was valid with no `@Media` and invalid with one. The maintainer
/// ruled that a timestamp is a timestamp wherever it sits (CLAN CHECK 112
/// agrees), so the two timed fixtures here declare the media they index.
#[test]
fn wor_tier_from_a_parsed_main_tier_has_flat_untimed_words() -> Result<(), TestError> {
    let main = timed_main_tier("hello \u{15}100_200\u{15} world .")?;
    let wor = main.generate_wor_tier();
    let words: Vec<&Word> = wor.words().collect();
    let texts: Vec<&str> = words.iter().map(|w| w.cleaned_text()).collect();
    assert_eq!(texts, ["hello", "world"]);
    assert!(
        words.iter().all(|w| w.inline_bullet.is_none()),
        "a main-tier parse puts timing in an `internal_bullet` item, never on the word"
    );
    Ok(())
}

/// A word inside an angle group is extracted, and nothing is duplicated.
///
/// The original assembled a bare `Group`, which no parser produces (`<hello>
/// hello .` is E316). `<...> [!]` is the parser-producible angle group and
/// takes the walker's `GroupRef::Angle` arm; `<...> [/]` would be a retrace
/// and take a different arm. The count is asserted, not just membership: a
/// walker that skipped the group would still contain `world`.
#[test]
fn wor_tier_extracts_words_from_groups() -> Result<(), TestError> {
    let main = timed_main_tier("<hello \u{15}50_150\u{15}> [!] world .")?;
    let texts: Vec<String> = main
        .generate_wor_tier()
        .words()
        .map(|w| w.cleaned_text().to_owned())
        .collect();
    assert_eq!(texts, ["hello", "world"]);
    Ok(())
}

// ── find_context_dependent_ca_omission_span ─────────────────────────────

/// A CA omission inside a group is found, at the omission's OWN span.
///
/// The original compared against `Span::from_usize(12, 18)`, a number it had
/// also written. Here the expected span is read off the parsed word.
#[test]
fn ca_omission_span_is_found_inside_a_group() -> Result<(), TestError> {
    let main = main_tier_ca("<(word)> [!] word .", &[])?;
    let omission = main
        .content
        .content
        .iter()
        .find_map(|item| match item {
            // `<...> [!]` is a group WITH a scoped annotation, a distinct
            // variant from the bare `Group` the original assembled; both hold
            // the same content and the walker descends into either.
            UtteranceContent::AnnotatedGroup(annotated) => annotated
                .inner
                .content
                .content
                .iter()
                .find_map(|i| match i {
                    BracketedItem::Word(word) => Some(word.span),
                    _ => None,
                }),
            _ => None,
        })
        .ok_or_else(|| TestError::Failure("the group built no word".into()))?;
    assert_eq!(
        main.find_context_dependent_ca_omission_span(),
        Some(omission)
    );
    Ok(())
}

/// A shortening-only word inside a replacement is found, at its own span.
#[test]
fn ca_omission_span_is_found_in_a_replacement_shortening() -> Result<(), TestError> {
    let main = main_tier("hello [: (lo)] .", "eng", &["E209"])?;
    let shortening = main
        .content
        .content
        .iter()
        .find_map(|item| match item {
            UtteranceContent::ReplacedWord(replaced) => {
                replaced.replacement.words.first().map(|w| w.span)
            }
            _ => None,
        })
        .ok_or_else(|| TestError::Failure("the replacement built no word".into()))?;
    assert_eq!(
        main.find_context_dependent_ca_omission_span(),
        Some(shortening)
    );
    Ok(())
}

// ── validate, the @s half ───────────────────────────────────────────────

fn validated(main: &MainTier, default: &str, declared: &[&str]) -> Vec<String> {
    let context = ValidationContext::new()
        .with_default_language(LanguageCode::new(default).expect("test literal is non-empty"))
        .with_declared_languages(codes(declared));
    let errors = ErrorCollector::new();
    main.validate(&context, &errors);
    errors
        .into_vec()
        .iter()
        .map(|e| e.code.as_str().to_owned())
        .collect()
}

/// An utterance whose every word carries `@s` is a whole-utterance switch and
/// must be written with a precode instead: E255.
#[test]
fn an_all_at_s_utterance_is_a_whole_utterance_switch() -> Result<(), TestError> {
    let main = main_tier("hola@s amiga@s .", "eng, spa", &["E255"])?;
    let got = validated(&main, "eng", &["eng", "spa"]);
    assert!(got.iter().any(|c| c == "E255"), "expected E255 in {got:?}");
    Ok(())
}

/// One untagged lexical word keeps the utterance on the word-level path.
#[test]
fn a_mixed_utterance_is_not_a_whole_utterance_switch() -> Result<(), TestError> {
    let main = main_tier("hola@s friend .", "eng, spa", &[])?;
    let got = validated(&main, "eng", &["eng", "spa"]);
    assert!(!got.iter().any(|c| c == "E255"), "E255 in {got:?}");
    Ok(())
}

// ── whole_utterance_language_switch_target ──────────────────────────────
//
// The predicate behind `chatter debug fix-s` and E255. Bug history
// (2026-05-06): it collected words through the MOR-domain walker, which
// skipped fillers, so `ballet@s , &~dang3 &~dang1 .` read as monolingual
// and was rewritten to `[- eng] ...`, putting E220 on the Cantonese tone
// fillers. The fix walks every word-bearing item; each case below is one
// filler shape that must refuse the rewrite unless it carries an explicit
// language.

fn target(main: &MainTier, default: Option<&str>, declared: &[&str]) -> Option<String> {
    let default = default.map(|d| LanguageCode::new(d).expect("test literal is non-empty"));
    main.whole_utterance_language_switch_target(default.as_ref(), &codes(declared))
        .map(|t| t.language().as_str().to_owned())
}

/// All-`@s`, default `yue`: `@s` resolves to the OTHER declared language.
#[test]
fn uniform_at_s_resolves_to_the_other_language() -> Result<(), TestError> {
    let main = main_tier("ballet@s hello@s .", "yue, eng", &["E255"])?;
    assert_eq!(
        target(&main, Some("yue"), &["yue", "eng"]).as_deref(),
        Some("eng")
    );
    Ok(())
}

/// The AliciaCan shape: a nonword filler with no language marker refuses.
#[test]
fn a_nonword_filler_without_a_marker_refuses() -> Result<(), TestError> {
    let main = main_tier("ballet@s &~dang3 .", "yue, eng", &[])?;
    assert_eq!(target(&main, None, &["yue", "eng"]), None);
    Ok(())
}

/// An `&-` filler with no language marker refuses.
#[test]
fn a_filler_without_a_marker_refuses() -> Result<(), TestError> {
    let main = main_tier("dile@s &-um a@s .", "eng, spa", &[])?;
    assert_eq!(target(&main, None, &["eng", "spa"]), None);
    Ok(())
}

/// A phonological fragment with no language marker refuses.
#[test]
fn a_phonological_fragment_without_a_marker_refuses() -> Result<(), TestError> {
    let main = main_tier("hola@s &+fr .", "eng, spa", &[])?;
    assert_eq!(target(&main, None, &["eng", "spa"]), None);
    Ok(())
}

/// A nonword filler INSIDE a retrace still counts: retracted content was
/// uttered.
#[test]
fn a_retraced_nonword_without_a_marker_refuses() -> Result<(), TestError> {
    let main = main_tier("<&~s> [///] el@s viernes@s .", "eng, spa", &[])?;
    assert_eq!(target(&main, Some("eng"), &["eng", "spa"]), None);
    Ok(())
}

/// A filler carrying an explicit matching `@s:LANG` does not block the
/// rewrite.
#[test]
fn a_filler_with_an_explicit_matching_language_accepts() -> Result<(), TestError> {
    let main = main_tier("hello@s:eng &-um@s:eng .", "yue, eng", &["E255"])?;
    assert_eq!(target(&main, None, &["yue", "eng"]).as_deref(), Some("eng"));
    Ok(())
}
