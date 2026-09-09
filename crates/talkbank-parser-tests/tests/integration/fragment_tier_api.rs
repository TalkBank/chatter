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

//! `TreeSitterParser::parse_tiers`, the dependent-tier fragment entry point.
//!
//! # Why this exists
//!
//! A whole-suite branch-coverage run on 2026-09-08 found `parse_tiers` with
//! ZERO covered regions, 67 of them. It is not dead: the public method has one
//! implementation and the implementation has one caller. It is UNTESTED, and
//! the reason is a trap worth naming.
//!
//! Its only invocations anywhere are two DOCTESTS, and `cargo llvm-cov --tests`
//! does not run doctests. A doctest is a real test and it is invisible to the
//! coverage instrument, so a function exercised only by one reads exactly like a
//! function nothing calls. The apparent third caller,
//! `raw_user_tier_characterization.rs`, is a test-local helper of the same name
//! that goes through `parse_chat_file_streaming` instead.
//!
//! This is the fragment API the LSP uses to parse one tier line out of context,
//! so it is worth more than a doctest: the CLI cannot reach it, which means no
//! `.cha` fixture anywhere in the corpus exercises it either.

use talkbank_model::model::DependentTier;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// A well-formed `%mor` line parses to a `%mor` tier.
///
/// SURVIVES a type change, and says which category: this is a FRAGMENT
/// BOUNDARY, where a caller hands over one line of CHAT text out of context.
/// No signature describes which tier a given string denotes.
#[test]
fn a_mor_line_parses_as_a_mor_tier() -> Result<(), TestError> {
    let parser = TreeSitterParser::new()?;
    let tier = parser
        .parse_tiers("%mor:\tpro|I v|want n|cookie-PL .")
        .expect("a well-formed %mor line is a dependent tier");
    assert!(
        matches!(tier, DependentTier::Mor(_)),
        "a `%mor:` line must lower to the Mor variant, not {tier:?}"
    );
    Ok(())
}

/// Each dependent-tier kind the fragment API admits reaches its own variant.
///
/// One case per kind rather than one representative: the entry point dispatches
/// on the tier label, so a single `%mor` case would leave every other arm of
/// that dispatch unexercised while reading as though the entry point worked.
#[test]
fn each_tier_kind_reaches_its_own_variant() -> Result<(), TestError> {
    let parser = TreeSitterParser::new()?;
    let mut wrong = Vec::new();

    for (line, expected) in [
        ("%mor:\tpro|I v|want n|cookie-PL .", "Mor"),
        ("%gra:\t1|2|NSUBJ 2|0|ROOT 3|2|PUNCT", "Gra"),
        ("%pho:\thɛloʊ", "Pho"),
        ("%com:\ta comment", "Com"),
        ("%act:\tpointing", "Act"),
    ] {
        match parser.parse_tiers(line) {
            Ok(tier) => {
                let got = format!("{tier:?}");
                if !got.starts_with(expected) {
                    wrong.push(format!(
                        "{line:?} -> {} (expected {expected})",
                        &got[..got.len().min(30)]
                    ));
                }
            }
            Err(err) => wrong.push(format!("{line:?} -> rejected: {err:?}")),
        }
    }

    assert!(
        wrong.is_empty(),
        "fragment tier dispatch:\n  {}",
        wrong.join("\n  ")
    );
    Ok(())
}

/// An unknown tier LABEL is preserved; input that is not a tier is refused.
///
/// The pair to the tests above, and the half a fragment API most needs: an
/// entry point that accepts anything cannot tell a caller it was handed the
/// wrong line.
///
/// # What this test learned rather than asserted
///
/// It first claimed `%bogus:\tcontent` would be refused, and that was MY belief
/// about CHAT rather than chatter's rule. Measured, it lowers to
/// `Unsupported(UserDefinedDependentTier { label: "bogus" })`, which is the
/// maximal-preservation policy working: a tier whose label chatter does not
/// know is KEPT, with its label and content, rather than discarded. Discarding
/// it would lose transcript content to a vocabulary gap, and a roundtrip could
/// not put it back.
///
/// So the rule this pins is the real one, and the boundary is sharper than
/// "unknown is rejected": a `%label:\t...` SHAPE is admitted whatever the
/// label, and anything that is not that shape is refused.
#[test]
fn an_unknown_tier_label_is_preserved_and_a_non_tier_is_refused() -> Result<(), TestError> {
    let parser = TreeSitterParser::new()?;

    let unknown = parser
        .parse_tiers("%bogus:\tcontent")
        .expect("an unknown tier label is preserved, not rejected");
    assert!(
        matches!(unknown, DependentTier::Unsupported(_)),
        "an unrecognised label must be kept as Unsupported so a roundtrip can \
         put it back, not silently dropped: {unknown:?}"
    );

    let mut accepted = Vec::new();
    for line in ["not a tier at all", "*CHI:\thello .", ""] {
        if let Ok(tier) = parser.parse_tiers(line) {
            let got = format!("{tier:?}");
            accepted.push(format!("{line:?} -> {}", &got[..got.len().min(70)]));
        }
    }
    assert!(
        accepted.is_empty(),
        "input with no `%label:` shape at all must be refused: {}",
        accepted.join(", ")
    );
    Ok(())
}
