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

//! Main-tier to `%pho` and `%mod` alignment, over tiers the parser built.
//!
//! # What this replaces
//!
//! The three inline tests of `talkbank-model`'s `alignment/pho.rs` built a
//! `MainTier` of `Word::new_unchecked` items beside `Terminator::Period {
//! span: Span::DUMMY }` and a `PhoTier::from_tokens(..)`. Here each case is
//! the two tiers as written; `%mod` is the SAME tier type with
//! `PhoTierType::Mod`, and the point of the third test is that it reports
//! the `%mod` codes and not the `%pho` ones.

use talkbank_model::alignment::{PhoAlignment, align_main_to_pho};
use talkbank_model::model::PhoTierType;
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

fn tier_name(tier_type: PhoTierType) -> &'static str {
    match tier_type {
        PhoTierType::Pho => "pho",
        PhoTierType::Mod => "mod",
    }
}

/// Parse a main tier with one `%pho` or `%mod` tier, checked to report
/// exactly `codes`, and align them.
fn aligned(
    tier_type: PhoTierType,
    main: &str,
    tokens: &str,
    codes: &[&str],
) -> Result<PhoAlignment, TestError> {
    let lines = format!("*CHI:\t{main}\n%{}:\t{tokens}", tier_name(tier_type));
    let utterance = SingleSpeaker::english(&lines).utterance(codes)?;
    let tier = match tier_type {
        PhoTierType::Pho => utterance.pho_tier(),
        PhoTierType::Mod => utterance.mod_tier(),
    }
    .ok_or_else(|| {
        TestError::Failure(format!(
            "the fixture built no %{} tier",
            tier_name(tier_type)
        ))
    })?;
    Ok(align_main_to_pho(&utterance.main, tier))
}

fn codes(alignment: &PhoAlignment) -> Vec<&str> {
    alignment.errors.iter().map(|e| e.code.as_str()).collect()
}

/// One token per word: complete pairs, no diagnostic, for either tier type.
#[test]
fn one_token_per_word_aligns_cleanly_for_pho_and_mod() -> Result<(), TestError> {
    for tier_type in [PhoTierType::Pho, PhoTierType::Mod] {
        let alignment = aligned(tier_type, "one two .", "w\u{28c}n tu\u{2d0}", &[])?;
        assert_eq!(alignment.pairs.len(), 2);
        assert!(alignment.pairs.iter().all(|p| p.is_complete()));
        assert!(alignment.errors.is_empty(), "{:?}", codes(&alignment));
    }
    Ok(())
}

/// A `%pho` tier reports the `%pho` codes: E714 when short, E715 when long.
#[test]
fn pho_tier_reports_e714_and_e715() -> Result<(), TestError> {
    let alignment = aligned(PhoTierType::Pho, "one two .", "w\u{28c}n", &["E714"])?;
    assert_eq!(codes(&alignment), ["E714"]);
    assert_eq!(
        alignment.pairs.len(),
        2,
        "one complete pair and one placeholder"
    );
    let alignment = aligned(PhoTierType::Pho, "one .", "w\u{28c}n tu\u{2d0}", &["E715"])?;
    assert_eq!(codes(&alignment), ["E715"]);
    Ok(())
}

/// A `%mod` tier reports the `%mod` codes, E733 when short and E734 when
/// long, and names `%mod`. Until 2026-09-03 the trait route hardcoded the
/// `%pho` codes, which is why `compute.rs` kept its own copy of the
/// algorithm with the codes passed as parameters.
#[test]
fn mod_tier_reports_e733_and_e734_not_the_pho_codes() -> Result<(), TestError> {
    let alignment = aligned(PhoTierType::Mod, "one two .", "w\u{28c}n", &["E733"])?;
    assert_eq!(codes(&alignment), ["E733"]);
    assert!(
        alignment.errors[0].message.contains("%mod"),
        "{}",
        alignment.errors[0].message
    );
    let alignment = aligned(PhoTierType::Mod, "one .", "w\u{28c}n tu\u{2d0}", &["E734"])?;
    assert_eq!(codes(&alignment), ["E734"]);
    Ok(())
}
