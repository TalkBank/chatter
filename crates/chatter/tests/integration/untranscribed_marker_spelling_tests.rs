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

//! How the three untranscribed markers may be SPELLED, end to end through the
//! CLI.
//!
//! CHAT has exactly three untranscribed-material markers, `xxx`, `yyy` and
//! `www`, and E241 exists to reject a token that is plainly one of them written
//! wrongly. Until 2026-08-15 which wrong spellings it caught was a hand-written
//! table, and the table disagreed with itself: `xx` and `yy` were caught, `ww`
//! was not, and of the miscased full forms only the all-caps ones were. Nobody
//! had written down a reason, because there was none.
//!
//! Brian MacWhinney ruled on 2026-08-15, asked directly: `ww` is not legal,
//! `www` is canonical, and `yy` against `yyy` likewise, "for consistency".
//! `docs/decisions/2026-08-15-brian-ruling-ww-and-yy.md` in the private
//! workspace records the ruling and its limits.
//!
//! The rule this file pins is therefore about the VOCABULARY, not about three
//! separate tokens: a shortened or miscased spelling of any marker is rejected.
//!
//! The IMPLEMENTATION derives that set from the marker list, so a fourth marker
//! cannot be forgotten there. These tests do not: the arrays below are written
//! by hand, and a fourth marker WOULD be forgotten here exactly as `ww` was.
//! `UntranscribedStatus::ALL` is private, deliberately, so nothing in this
//! workspace can iterate the vocabulary from outside the model. Widening it to
//! close that gap is a decision about the model's public surface, not about
//! these tests, and it has not been taken.
//!
//! # The exemption, which is the other half
//!
//! `&+ww` is a phonological FRAGMENT that happens to be spelled `ww`. It is not
//! a marker and must stay valid. The rule used to read a word's CLEANED text,
//! which has already discarded the `&+` prefix, so the fragment and the marker
//! arrived at the check indistinguishable. That is the defect shape the CHAT
//! doctrine names: meaning recovered from serialised text instead of from the
//! typed model.

use talkbank_model::ErrorCode;
use talkbank_parser_tests::test_error::TestError;

use crate::common::{Verdict, assert_validation};

/// Build a one-utterance English file whose only variable is the main-tier text.
fn chat_file(words: &str) -> String {
    format!(
        "@UTF8\n\
         @Begin\n\
         @Languages:\teng\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|3;00.00|male|||Target_Child|||\n\
         *CHI:\tI saw {words} there .\n\
         @End\n",
    )
}

// ---------------------------------------------------------------------------
// The canonical spellings stay valid. Without these, "reject everything" passes.
// ---------------------------------------------------------------------------

#[test]
fn validate_accepts_the_three_canonical_markers() -> Result<(), TestError> {
    for marker in ["xxx", "yyy", "www"] {
        assert_validation(
            &format!("canonical_{marker}.cha"),
            &chat_file(marker),
            Verdict::Valid,
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shortened forms: the ruling. `xx` and `yy` already failed here; `ww` is new.
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_every_shortened_marker() -> Result<(), TestError> {
    for short in ["xx", "yy", "ww"] {
        assert_validation(
            &format!("short_{short}.cha"),
            &chat_file(short),
            Verdict::Rejected(ErrorCode::IllegalUntranscribed),
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Miscased full forms. Only the all-caps ones were caught; the mixed were not.
// Reported by an external integrator (rustling integration notes, 2026-07-31).
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_every_miscased_marker() -> Result<(), TestError> {
    for miscased in ["XXX", "YYY", "WWW", "Xxx", "Yyy", "Www", "xXx"] {
        assert_validation(
            &format!("miscased_{miscased}.cha"),
            &chat_file(miscased),
            Verdict::Rejected(ErrorCode::IllegalUntranscribed),
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The exemption: a prefixed form is not a marker, whatever its letters spell.
// ---------------------------------------------------------------------------

/// `&+ww` is a phonological fragment, not a mistyped `www`.
///
/// 95 of the ~730 corpus occurrences of the token `ww` are this shape. Shipping
/// the widening without this exemption would hand the corpus authority 95 wrong
/// errors on the first run.
#[test]
fn validate_accepts_a_phonological_fragment_spelled_like_a_marker() -> Result<(), TestError> {
    assert_validation("fragment_ww.cha", &chat_file("&+ww"), Verdict::Valid)
}

/// The exemption is about the CATEGORY, not about one prefix, so it is stated
/// over every prefixed form rather than over the one that occurs in the corpus.
/// Without this, an implementation that special-cases `&+` passes and leaves the
/// invariant unwritten.
#[test]
fn validate_accepts_other_prefixed_forms_spelled_like_a_marker() -> Result<(), TestError> {
    for prefixed in ["&+xx", "&-ww", "&~ww"] {
        assert_validation(
            &format!(
                "prefixed_{}.cha",
                prefixed.replace(['&', '+', '-', '~'], "_")
            ),
            &chat_file(prefixed),
            Verdict::Valid,
        )?;
    }
    Ok(())
}
