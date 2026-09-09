//! Word-level validation for states NO PARSE PRODUCES.
//!
//! # Why the rest of this file left
//!
//! It held thirty-odd tests that built a `Word` by hand and asserted a rule
//! against it: a raw text `"+word"` beside a content list
//! `[CompoundMarker, Text("word")]`, with nothing forcing the two to agree.
//! That is the input stated twice, and the coverage it produced was
//! fabrication-backed: the validator ran, the regions counted as covered, and
//! no CHAT text was ever read. Those rules now run over words the PARSER built,
//! in `talkbank-parser-tests`. Not because a test here cannot parse: an
//! integration test under `tests/` with a dev-dependency cycle can, as
//! `talkbank-derive` does. A `#[cfg(test)] mod` in `src/` cannot, because the
//! lib-test target is a second instantiation of this crate, and that is where
//! these were.
//!
//! Migrating them found two things no hand-built version could:
//!
//! - `+word` does not parse as a word under the CANONICAL parser, by either
//!   route, so there E232 fires only on a `Word` a test made; the re2c backend
//!   builds it and reports E232. That matches what `UNDEMONSTRATED` records,
//!   and is now measured rather than remembered. E232's message is pinned in
//!   `talkbank-parser-tests` through the re2c backend over a whole file, the
//!   one route from CHAT text to it; the `insta` snapshot over a hand-built
//!   `Word` that used to pin it here is gone.
//! - `:test` does not reach E246. A leading `:` is a separator glued to what
//!   follows, which is E765, as `spec/errors/E246.md` records in its first
//!   example since 2026-09-05. The deleted test asserted the older reading,
//!   and could keep doing so because it built a `Word` carrying a leading
//!   `Lengthening` that no parse produces.
//! - `Word::simple("un+do")` carries no `CompoundMarker`, so the deleted
//!   `test_valid_compound` never reached the check it was named for.
//!
//! # What is left, and why it belongs here
//!
//! Two rules guard `Word` values that no parser can build, so no parse-backed
//! test can reach them and there is nothing to migrate:
//!
//! - E243: word TEXT carrying a space, tab, newline or bullet byte. Whitespace
//!   is what separates words, so a parsed `Word` never contains it; this rule
//!   exists for the `%wor` export path and for deserialization.
//! - E251/E253: a `Word` with an EMPTY content list. A parse always produces
//!   content; the reachable route is `serde`, which is how the E251 test builds
//!   its fixture.
//!
//! These stay hand-built ON PURPOSE, and that is the difference between the
//! fabrication that left and the fabrication that remains: the state under test
//! is genuinely unreachable from CHAT, so building it is the only way to reach
//! the rule at all.
//!
//! CHAT references:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Compounds>
//! - <https://talkbank.org/0info/manuals/CHAT.html#PrimaryStress_Element>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Lengthening_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#WordInternalPause_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>

use crate::model::{LanguageCode, Word, WordContent, WordContents, WordText};
use crate::non_empty_literal;
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCollector, ParseError};
use serde_json;

/// Validates one word under a caller-controlled language/CA context.
///
/// Tests use this helper to keep fixtures focused on token structure instead of
/// repeating `ValidationContext` and sink wiring.
fn run_word_validation(
    word: &Word,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
    ca_mode: bool,
) -> Vec<ParseError> {
    let context = ValidationContext::new()
        .with_declared_languages(declared_languages.to_vec())
        .with_ca_mode(ca_mode)
        .with_tier_language(tier_language.cloned());
    let errors = ErrorCollector::new();
    word.validate(&context, &errors);
    errors.into_vec()
}

/// Flags trailing whitespace contamination in word tokens.
///
/// This case mirrors historical `%wor` export issues where trailing spaces were
/// accidentally serialized into token text.
#[test]
fn test_e243_word_with_trailing_space() {
    // This catches the %wor tier bug where words include trailing spaces
    let word = Word::simple("hello ");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E243"),
        "Expected E243 error for word with trailing space, got: {:#?}",
        errors
    );
}

/// Flags leading whitespace contamination in word tokens.
///
/// Leading spaces should consistently emit `E243` rather than silently passing.
#[test]
fn test_e243_word_with_leading_space() {
    let word = Word::simple(" hello");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E243"),
        "Expected E243 error for word with leading space, got: {:#?}",
        errors
    );
}

/// Rejects stray media bullet bytes inside lexical word text.
///
/// The U+0015 marker is structural metadata, so textual inclusion should report
/// `E243` with a bullet-specific message.
#[test]
fn test_e243_word_with_bullet_marker() {
    // Bullet marker U+0015 (byte 0x15) should never be in word text
    let word_with_bullet = "hello\x15".to_string();
    let word = Word::new_unchecked(&word_with_bullet, &word_with_bullet);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors
            .iter()
            .any(|e| e.code.as_str() == "E243" && e.message.contains("bullet")),
        "Expected E243 error for word with bullet marker, got: {:#?}",
        errors
    );
}

/// Rejects tab characters embedded in a word token.
///
/// Tabs are treated as formatting contamination and should emit `E243`.
#[test]
fn test_e243_word_with_tab() {
    let word = Word::simple("hello\tworld");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E243"),
        "Expected E243 error for word with tab character, got: {:#?}",
        errors
    );
}

/// Rejects newline characters embedded in a word token.
///
/// Newlines must not survive tokenization and therefore trigger `E243`.
#[test]
fn test_e243_word_with_newline() {
    let word = Word::simple("hello\nworld");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E243"),
        "Expected E243 error for word with newline, got: {:#?}",
        errors
    );
}

/// Confirms clean lexical tokens do not spuriously emit formatting errors.
///
/// A plain token should pass with no `E243`.
#[test]
fn test_e243_clean_word_no_error() {
    let word = Word::simple("hello");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E243"),
        "Clean word should not trigger E243"
    );
}

/// Handles mixed contamination (`space + bullet`) in one token robustly.
///
/// This regression fixture checks that at least one `E243` is raised for the
/// real-world `%wor` corruption pattern.
#[test]
fn test_e243_word_with_space_and_bullet() {
    // This is the exact pattern from %wor tier bug: "word \x15"
    let word_with_space_and_bullet = "hello \x15".to_string();
    let word = Word::new_unchecked(&word_with_space_and_bullet, &word_with_space_and_bullet);
    let errors = run_word_validation(&word, None, &[], false);

    // Should get at least one E243 error (maybe two - one for space, one for bullet)
    let e243_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code.as_str() == "E243")
        .collect();
    assert!(
        !e243_errors.is_empty(),
        "Expected E243 error for word with space and bullet marker, got: {:#?}",
        errors
    );
}

// =============================================================================
// Prosodic Marker Validation Tests (E244-E247, E250, E251)
// =============================================================================

/// Rejects words whose parsed content list is unexpectedly empty.
///
/// `E253` protects downstream logic that assumes each word has at least one
/// semantic element.
#[test]
fn test_e253_empty_word_content_list() {
    let word = Word::simple("test").with_content(WordContents::default());
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E253"),
        "Expected E253 error for empty word content list, got: {:#?}",
        errors
    );
}

/// Confirms non-empty content vectors pass the `E253` structural check.
///
/// A normal `Word` should preserve at least one element in `content`.
#[test]
fn test_e253_non_empty_word_content_list() {
    let word = Word::simple("test");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E253"),
        "Non-empty word content list should not trigger E253"
    );
}

/// Deserialization admits empty text; validation must reject all three textual
/// content variants, while accepting their non-empty counterparts.
#[test]
fn test_e251_deserialized_word_content() -> Result<(), serde_json::Error> {
    for kind in ["text", "phonetic", "shortening"] {
        for text in ["", "test"] {
            let content: WordContent = serde_json::from_value(serde_json::json!({
                "type": kind,
                "content": text,
            }))?;
            let errors = ErrorCollector::new();
            content.validate(&ValidationContext::new(), &errors);
            let codes: Vec<_> = errors.into_vec().into_iter().map(|e| e.code).collect();
            let expected = if text.is_empty() {
                vec![crate::ErrorCode::EmptyWordContentText]
            } else {
                vec![]
            };
            assert_eq!(codes, expected, "{kind} content {text:?}");
        }
    }
    Ok(())
}

/// Accepts `WordText` elements with non-empty lexical content.
///
/// This positive case prevents false-positive `E251` reports.
#[test]
fn test_e251_valid_non_empty_text() {
    // Word with non-empty Text content
    let word = Word::simple("test").with_content(vec![WordContent::Text(WordText::from(
        non_empty_literal!("test"),
    ))]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E251"),
        "Non-empty word content text should not trigger E251"
    );
}
