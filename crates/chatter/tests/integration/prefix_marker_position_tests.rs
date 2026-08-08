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

//! Where the prefix marker `#` may appear in a main-tier word, end to end
//! through the CLI.
//!
//! Two independent rules are exercised here, and keeping them apart is the
//! point of the file:
//!
//! 1. **Position, language-independent (E762).** The marker attaches to the
//!    END of the prefix it marks, as in the Hebrew `ha# kelev` ("the dog").
//!    So a word that is nothing but `#`, or that opens with `#`, cannot be
//!    that construct in any language. Neither shape is attested anywhere in
//!    the corpora.
//!
//! 2. **Language, position-independent (E763).** A legally-positioned marker
//!    is only meaningful in a language that writes prefixes as separate
//!    orthographic units, which in the corpora means Hebrew and Arabic. The
//!    gate is on the WORD's resolved language, exactly as the digits rule
//!    (E220) works, never on the file's `@Languages` header: a word carrying
//!    its own `@s:` marker carries its own language with it.
//!
//! Grounding (typed survey over every `#`-bearing corpus file, 2026-07-26):
//! word-final 26,811 Arabic + 8,041 Hebrew + 14 strays in seven other
//! languages; word-internal 35,802, all Hebrew (BermanLong); word-initial 0;
//! standalone 0.

use talkbank_parser_tests::test_error::TestError;

/// Build a one-utterance file with the given `@Languages` header and words.
///
/// A helper rather than a table of literals because these tests vary exactly
/// two things, the declared language and the main-tier text, and spelling out
/// eleven near-identical headers would bury that.
///
/// `languages` is written exactly as CHAT requires it, comma-SPACE separated
/// (`"heb, eng"`). Writing it any other way produces a malformed header, and
/// a malformed header makes every "expect invalid" assertion in this file
/// pass for the wrong reason.
fn chat_file(languages: &str, words: &str) -> String {
    format!(
        "@UTF8\n\
         @Begin\n\
         @Languages:\t{languages}\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\t{languages}|corpus|CHI|3;00.00|male|||Target_Child|||\n\
         *CHI:\t{words} .\n\
         @End\n",
    )
}

// ---------------------------------------------------------------------------
// Rule 1: position, language-independent
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_a_standalone_prefix_marker_as_a_word() -> Result<(), TestError> {
    crate::common::assert_validation(
        "standalone.cha",
        &chat_file("eng", "the # dog"),
        crate::common::Verdict::Rejected(talkbank_model::ErrorCode::PrefixMarkerIllegalPosition),
    )
}

#[test]
fn validate_rejects_a_word_opening_with_the_prefix_marker() -> Result<(), TestError> {
    crate::common::assert_validation(
        "initial.cha",
        &chat_file("eng", "the #dog"),
        crate::common::Verdict::Rejected(talkbank_model::ErrorCode::PrefixMarkerIllegalPosition),
    )
}

/// The positional rule holds even in the languages that use the marker.
///
/// Without this the rule could be implemented as a language gate and still
/// pass the two tests above, which would leave the actual invariant unstated.
#[test]
fn validate_rejects_a_standalone_prefix_marker_in_hebrew_too() -> Result<(), TestError> {
    crate::common::assert_validation(
        "standalone_heb.cha",
        &chat_file("heb", "ha# # kelev"),
        crate::common::Verdict::Rejected(talkbank_model::ErrorCode::PrefixMarkerIllegalPosition),
    )
}

// ---------------------------------------------------------------------------
// Rule 2: language, position-independent
// ---------------------------------------------------------------------------

#[test]
fn validate_accepts_a_word_final_prefix_marker_in_hebrew() -> Result<(), TestError> {
    crate::common::assert_validation(
        "heb.cha",
        &chat_file("heb", "ha# kelev"),
        crate::common::Verdict::Valid,
    )
}

#[test]
fn validate_accepts_a_word_final_prefix_marker_in_arabic() -> Result<(), TestError> {
    crate::common::assert_validation(
        "ara.cha",
        &chat_file("ara", "l# walad"),
        crate::common::Verdict::Valid,
    )
}

#[test]
fn validate_rejects_a_word_final_prefix_marker_in_english() -> Result<(), TestError> {
    crate::common::assert_validation(
        "eng.cha",
        &chat_file("eng", "sun# dog"),
        crate::common::Verdict::Rejected(talkbank_model::ErrorCode::PrefixMarkerLanguageNotAllowed),
    )
}

/// The gate reads the WORD's language, not the file header.
///
/// This is the whole distinction: an English-headed file may legitimately
/// contain a Hebrew word, and that word brings its own rules with it.
#[test]
fn validate_accepts_a_hebrew_marked_word_inside_an_english_file() -> Result<(), TestError> {
    crate::common::assert_validation(
        "switch.cha",
        &chat_file("eng, heb", "the ha#@s:heb kelev@s:heb"),
        crate::common::Verdict::Valid,
    )
}

/// The converse of the previous test, and a real corpus case.
///
/// A Hebrew-script word tagged as an English code switch is a data error
/// (maintainer ruling, 2026-07-26, over three such annotations in
/// BermanLong), and the rule is expected to flag it rather than to carve out
/// an exception for `@s`-marked words.
#[test]
fn validate_rejects_a_marker_word_tagged_as_a_switch_to_a_language_without_the_marker()
-> Result<(), TestError> {
    crate::common::assert_validation(
        "mistagged.cha",
        &chat_file("heb, eng", "ha#@s:eng kelev"),
        crate::common::Verdict::Rejected(talkbank_model::ErrorCode::PrefixMarkerLanguageNotAllowed),
    )
}

/// Word-INTERNAL markers remain legal in Hebrew.
///
/// BermanLong writes 35,802 of them as glued forms. Rejecting internal
/// markers globally is a separate, deferred change that is blocked on
/// normalizing that corpus; until then this test is the guard against
/// implementing it by accident.
#[test]
fn validate_accepts_a_word_internal_prefix_marker_in_hebrew() -> Result<(), TestError> {
    crate::common::assert_validation(
        "internal_heb.cha",
        &chat_file("heb", "mi#ha#shuk"),
        crate::common::Verdict::Valid,
    )
}

/// A word-internal marker still fails in a language that does not use it.
///
/// The language gate is about the marker's presence in the word, exactly as
/// the digits rule is; position is rule 1's concern, not rule 2's.
#[test]
fn validate_rejects_a_word_internal_prefix_marker_in_english() -> Result<(), TestError> {
    crate::common::assert_validation(
        "internal_eng.cha",
        &chat_file("eng", "sun#shine"),
        crate::common::Verdict::Rejected(talkbank_model::ErrorCode::PrefixMarkerLanguageNotAllowed),
    )
}
