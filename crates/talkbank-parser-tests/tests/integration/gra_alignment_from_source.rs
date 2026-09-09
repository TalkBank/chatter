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

//! `%mor` to `%gra` alignment, over tiers the parser built.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `alignment/gra/tests.rs` built each `MorTier` from
//! `Mor::new(MorWord::new(PosCategory::new("pron"), "I"))` beside a
//! `Terminator::Period { span: Span::DUMMY }`, and each `GraTier` from
//! `GrammaticalRelation::new(1, 2, "SUBJ")`. Here each case is the two tiers
//! as written under a main tier, and the tiers are whatever the parser
//! builds.
//!
//! # What moving them found
//!
//! `SUBJ`, which the originals wrote, is not a Universal Dependencies
//! relation: it is E761 (`OBL` and `IOBJ`, which they also wrote, are). The
//! relation label is irrelevant to alignment, which counts, so that row uses
//! `NSUBJ` and every fixture is valid CHAT. And a `%gra` tier of
//! one `PUNCT` relation with no `ROOT` is E722, so the terminator-only row
//! says so; the alignment it asserts is unchanged.

use talkbank_model::alignment::{GraAlignment, align_mor_to_gra};
use talkbank_model::model::Utterance;
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

/// Parse a main tier with its `%mor` and `%gra`, checked to report exactly
/// `codes`, and align the two dependent tiers.
fn aligned(
    main: &str,
    mor: &str,
    gra: &str,
    codes: &[&str],
) -> Result<(Utterance, GraAlignment), TestError> {
    let lines = format!("*CHI:\t{main}\n%mor:\t{mor}\n%gra:\t{gra}");
    let utterance = SingleSpeaker::english(&lines).utterance(codes)?;
    let mor = utterance
        .mor_tier()
        .ok_or_else(|| TestError::Failure("the fixture built no %mor tier".into()))?;
    let gra = utterance
        .gra_tier()
        .ok_or_else(|| TestError::Failure("the fixture built no %gra tier".into()))?;
    let alignment = align_mor_to_gra(mor, gra);
    Ok((utterance, alignment))
}

/// Equal chunk and relation counts align without error, the terminator's
/// `PUNCT` relation included.
#[test]
fn perfect_match_aligns_every_chunk() -> Result<(), TestError> {
    let (utterance, alignment) = aligned(
        "I go home .",
        "pron|I verb|go noun|home .",
        "1|2|NSUBJ 2|0|ROOT 3|2|OBL 4|2|PUNCT",
        &[],
    )?;
    assert_eq!(alignment.pairs.len(), 4);
    assert!(alignment.is_error_free());
    assert_eq!(utterance.mor_tier().expect("mor").count_chunks(), 4);
    Ok(())
}

/// A post-clitic is its own chunk, and `%gra` supplies a relation for it.
#[test]
fn a_post_clitic_is_a_chunk_of_its_own() -> Result<(), TestError> {
    let (utterance, alignment) = aligned(
        "it's .",
        "pron|it~aux|be .",
        "1|2|EXPL 2|0|ROOT 3|2|PUNCT",
        &[],
    )?;
    // main + 1 post-clitic + terminator
    assert_eq!(utterance.mor_tier().expect("mor").count_chunks(), 3);
    assert_eq!(alignment.pairs.len(), 3);
    assert!(alignment.is_error_free());
    Ok(())
}

/// Extra `%mor` chunks get placeholder `%gra` entries and one E720.
#[test]
fn a_longer_mor_tier_gets_placeholders() -> Result<(), TestError> {
    let (_, alignment) = aligned("a b c .", "verb|a verb|b verb|c .", "1|0|ROOT", &["E720"])?;
    // 1 valid + 3 placeholders
    assert_eq!(alignment.pairs.len(), 4);
    assert!(!alignment.is_error_free());
    assert_eq!(alignment.errors.len(), 1);
    assert_eq!(alignment.errors[0].code.as_str(), "E720");
    assert!(alignment.pairs[0].is_complete());
    assert!(alignment.pairs[1].is_placeholder());
    assert!(alignment.pairs[2].is_placeholder());
    assert!(alignment.pairs[3].is_placeholder());
    Ok(())
}

/// Extra `%gra` relations get placeholder `%mor` entries and one E720.
#[test]
fn a_longer_gra_tier_gets_placeholders() -> Result<(), TestError> {
    let (_, alignment) = aligned("go .", "verb|go .", "1|0|ROOT 2|1|PUNCT 3|1|OBJ", &["E720"])?;
    // 2 valid + 1 placeholder
    assert_eq!(alignment.pairs.len(), 3);
    assert!(!alignment.is_error_free());
    assert_eq!(alignment.errors.len(), 1);
    assert_eq!(alignment.errors[0].code.as_str(), "E720");
    assert!(alignment.pairs[0].is_complete());
    assert!(alignment.pairs[1].is_complete());
    assert!(alignment.pairs[2].is_placeholder());
    Ok(())
}

/// A terminator-only `%mor` tier is one chunk, aligned to the one `PUNCT`
/// relation; that `%gra` has no `ROOT` (E722) is a fact about the fixture,
/// not about the alignment.
#[test]
fn a_terminator_only_mor_tier_is_one_pair() -> Result<(), TestError> {
    let (_, alignment) = aligned("&=laughs .", ".", "1|0|PUNCT", &["E722"])?;
    assert_eq!(alignment.pairs.len(), 1);
    assert!(alignment.is_error_free());
    Ok(())
}

/// The `%mor`-longer diagnostic renders both tiers in columns and marks the
/// missing `%gra` entries.
#[test]
fn the_mor_longer_diagnostic_shows_both_columns() -> Result<(), TestError> {
    let (_, alignment) = aligned(
        "I'll give you .",
        "pron|I~aux|will verb|give pron|you .",
        "1|3|NSUBJ 2|3|AUX 3|0|ROOT",
        &["E720"],
    )?;
    assert!(!alignment.is_error_free());
    let msg = &alignment.errors[0].message;
    assert!(
        msg.contains("%mor chunks"),
        "should have %mor header: {msg}"
    );
    assert!(
        msg.contains("%gra relations"),
        "should have %gra header: {msg}"
    );
    assert!(msg.contains("pron|I"), "should show pron|I chunk: {msg}");
    assert!(
        msg.contains("aux|will"),
        "should show aux|will clitic: {msg}"
    );
    assert!(msg.contains("1|3|NSUBJ"), "should show gra relation: {msg}");
    assert!(
        msg.contains("\u{2296}"),
        "should mark missing gra entries: {msg}"
    );
    Ok(())
}

/// The `%gra`-longer diagnostic shows the chunk and marks the extra
/// relations.
#[test]
fn the_gra_longer_diagnostic_marks_the_extras() -> Result<(), TestError> {
    let (_, alignment) = aligned("go .", "verb|go .", "1|0|ROOT 2|1|PUNCT 3|1|OBJ", &["E720"])?;
    assert!(!alignment.is_error_free());
    let msg = &alignment.errors[0].message;
    assert!(msg.contains("verb|go"), "should show mor chunk: {msg}");
    assert!(msg.contains("3|1|OBJ"), "should show extra gra: {msg}");
    assert!(
        msg.contains("\u{2295}"),
        "should mark extra gra entries: {msg}"
    );
    Ok(())
}
