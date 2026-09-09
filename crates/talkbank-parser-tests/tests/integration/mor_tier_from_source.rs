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

//! The `%mor` tier's chunk sequence, over a tier the parser built.
//!
//! # What this replaces
//!
//! Four tests in `talkbank-model`'s `model/dependent_tier/mor/tests.rs` built
//! `MorTier::new_mor(vec![its, cookie], Terminator::Period { span:
//! Span::DUMMY })` and asserted what `chunks()`, `chunk_at` and
//! `item_index_of_chunk` made of it. The tier here is parsed from
//! `pron|it~aux|be noun|cookie .`, the text those tests named in their docs.
//! The item-level tests (`Mor::new`, `to_chat_string`) stay there: they build
//! no tier and hold no fabricated span.

use talkbank_model::model::{MorChunkKind, MorTier};
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

/// The `%mor` tier of `it's cookie .`: a main, its post-clitic, a main, and
/// the terminator.
fn tier() -> Result<MorTier, TestError> {
    let utterance =
        SingleSpeaker::english("*CHI:\tit's cookie .\n%mor:\tpron|it~aux|be noun|cookie .")
            .utterance(&[])?;
    utterance
        .mor_tier()
        .cloned()
        .ok_or_else(|| TestError::Failure("the fixture built no %mor tier".into()))
}

/// Chunks across items, clitics and the trailing terminator: four.
#[test]
fn count_chunks_counts_items_clitics_and_the_terminator() -> Result<(), TestError> {
    assert_eq!(tier()?.count_chunks(), 4);
    Ok(())
}

/// The chunk sequence is main, post-clitic, main, terminator, in that order,
/// with the kinds and lemmas `%gra` alignment reads.
///
/// This is the primitive every downstream consumer routes through; a break
/// here mis-renders every `%gra` hover, edge and diagnostic.
#[test]
fn chunks_expand_items_with_post_clitics_then_terminator() -> Result<(), TestError> {
    let tier = tier()?;
    let chunks: Vec<_> = tier.chunks().collect();
    assert_eq!(chunks.len(), 4);
    assert_eq!(tier.count_chunks(), chunks.len());
    assert_eq!(chunks[0].kind(), MorChunkKind::Main);
    assert_eq!(chunks[0].lemma(), Some("it"));
    assert_eq!(chunks[1].kind(), MorChunkKind::PostClitic);
    assert_eq!(chunks[1].lemma(), Some("be"));
    assert_eq!(chunks[2].kind(), MorChunkKind::Main);
    assert_eq!(chunks[2].lemma(), Some("cookie"));
    assert_eq!(chunks[3].kind(), MorChunkKind::Terminator);
    assert_eq!(chunks[3].lemma(), None);
    assert_eq!(
        chunks[3].terminator().map(|t| t.to_string()),
        Some(".".to_string())
    );
    Ok(())
}

/// `chunk_at` indexes the same sequence, so a main and its post-clitic share
/// one host item, which is what one-pair-per-item alignment relies on.
#[test]
fn chunk_at_resolves_the_host_item_of_a_clitic() -> Result<(), TestError> {
    let tier = tier()?;
    let main = tier.chunk_at(0).expect("main chunk");
    let clitic = tier.chunk_at(1).expect("clitic chunk");
    let cookie = tier.chunk_at(2).expect("cookie chunk");
    assert!(std::ptr::eq(
        main.host_item().unwrap(),
        clitic.host_item().unwrap()
    ));
    assert!(!std::ptr::eq(
        main.host_item().unwrap(),
        cookie.host_item().unwrap()
    ));
    assert!(tier.chunk_at(3).unwrap().host_item().is_none());
    assert!(tier.chunk_at(4).is_none());
    Ok(())
}

/// `item_index_of_chunk` collapses a clitic onto its host, skips the
/// terminator, and refuses an out-of-range index: the mapping the LSP's
/// `%gra` highlight uses to land on the right word.
#[test]
fn item_index_of_chunk_collapses_a_clitic_onto_its_host() -> Result<(), TestError> {
    let tier = tier()?;
    assert_eq!(tier.item_index_of_chunk(0), Some(0));
    assert_eq!(tier.item_index_of_chunk(1), Some(0));
    assert_eq!(tier.item_index_of_chunk(2), Some(1));
    assert_eq!(tier.item_index_of_chunk(3), None);
    assert_eq!(tier.item_index_of_chunk(4), None);
    Ok(())
}
