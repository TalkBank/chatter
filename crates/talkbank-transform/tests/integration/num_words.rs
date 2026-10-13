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

//! Boundary test for the extracted number-normalization utility.
//!
//! Any CHAT generator (the MICASE/SBCSAE converters, ASR pipelines, external
//! tools) that emits digit-bearing content needs to spell digits out so the
//! output satisfies E220. This is the general core of what was the batchalign
//! ASR post-processor's `expand_number`, now a chatter utility.

use talkbank_transform::num_words::expand_number;

#[test]
fn expands_digits_per_language() {
    // Cardinal expansion across the table languages.
    assert_eq!(expand_number("5", "eng"), "five");
    assert_eq!(expand_number("99", "eng"), "ninety-nine");
    assert_eq!(expand_number("5", "spa"), "cinco");

    // CJK via num2chinese.
    assert_eq!(expand_number("42", "zho"), "四十二");

    // Non-digit content passes through unchanged.
    assert_eq!(expand_number("hello", "eng"), "hello");

    // Currency and English ordinals (the converters feed English text that can
    // carry these forms).
    assert!(expand_number("$5", "eng").contains("dollars"));
    assert_eq!(expand_number("21st", "eng"), "twenty-first");

    // An unknown language leaves the digit unchanged (honest passthrough; the
    // validator's E220 catches it if that language forbids digits).
    assert_eq!(expand_number("42", "xxx"), "42");
}

/// Unsupported composition must not partially rewrite a caller's token.
#[test]
fn table_only_languages_preserve_unsupported_tokens() {
    assert_eq!(expand_number("10000", "fra"), "dix mille");
    for language in ["fra", "deu", "xxx"] {
        for token in [
            "100000",
            "$100000",
            "100000€",
            "5-100000",
            "5—100000",
            "100000-year-old",
        ] {
            assert_eq!(expand_number(token, language), token, "{language}: {token}");
        }
    }
    assert_eq!(expand_number("5-6", "fra"), "cinq six");
    assert_eq!(expand_number("$5", "fra"), "cinq dollars");
}

/// Authored spellings; suffix recognition normalizes compatibility inputs.
#[test]
fn dutch_ordinals_render_authored_words_at_the_public_boundary() {
    for (rank, expected) in [
        (1, "eerste"),
        (2, "tweede"),
        (3, "derde"),
        (8, "achtste"),
        (12, "twaalfde"),
        (18, "achttiende"),
        (19, "negentiende"),
        (20, "twintigste"),
        (22, "tweeëntwintigste"),
        (29, "negenentwintigste"),
        (33, "drieëndertigste"),
        (100, "honderdste"),
        (101, "honderdeerste"),
        (102, "honderdtweede"),
        (108, "honderdachtste"),
        (117, "honderdzeventiende"),
        (200, "tweehonderdste"),
        (999, "negenhonderdnegenennegentigste"),
        (1000, "duizendste"),
    ] {
        for suffix in ["e", "de", "ste", "ᵉ"] {
            let token = format!("{rank}{suffix}");
            for lang in ["nld", "NLD", "Nld"] {
                assert_eq!(expand_number(&token, lang), expected, "{lang}: {token}");
            }
        }
        assert_eq!(expand_number(expected, "nld"), expected);
    }
    assert_eq!(expand_number("00029e", "nld"), "negenentwintigste");
}

/// Rejection preserves the complete spelling; it is not CHAT admission.
#[test]
fn dutch_ordinal_recognition_is_whole_token_ascii_and_bounded() {
    for token in [
        "0e",
        "0de",
        "1001e",
        "65536e",
        "999999999999999999999999999999e",
        "abc29e",
        "29extra",
        "29steabc",
        "29°",
        "29.e",
        "29e.",
        "-29e",
        "3.14e",
        "29e-dag",
        "29E",
        "29DE",
        "29STE",
        "２９e",
        "٢٩e",
        " 29e",
        "29e ",
    ] {
        assert_eq!(expand_number(token, "nld"), token, "{token}");
    }
    for lang in ["eng", "por", "fra", "spa", "deu", "xxx"] {
        for suffix in ["e", "de", "ste", "ᵉ"] {
            let token = format!("29{suffix}");
            assert_eq!(expand_number(&token, lang), token, "{lang}: {token}");
        }
    }
}

/// Every supported rank is rendered; existing cardinal/currency policy stays.
#[test]
fn dutch_ordinal_domain_is_complete_without_changing_cardinals() {
    for rank in 1..=1000 {
        let canonical = expand_number(&format!("{rank}e"), "nld");
        assert!(
            canonical.chars().all(char::is_alphabetic),
            "{rank}: {canonical}"
        );
        for suffix in ["de", "ste", "ᵉ"] {
            assert_eq!(expand_number(&format!("{rank}{suffix}"), "nld"), canonical);
        }
    }
    for (token, expected) in [
        ("22", "tweeëntwintig"),
        ("100", "honderd"),
        ("1000", "duizend"),
        ("101", "101"),
        ("21-22", "eenentwintig tweeëntwintig"),
        ("$22", "tweeëntwintig dollars"),
        ("22-dagen", "tweeëntwintig-dagen"),
    ] {
        assert_eq!(expand_number(token, "nld"), expected);
    }
}
