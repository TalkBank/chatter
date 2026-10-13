//! The CA-omission canonicalization, owned once.
//!
//! Under `@Options: CA`, a word written entirely in parentheses is a CA
//! OMISSION, which is a different thing from a CHAT shortening: `(be)cause` is
//! a partly-unpronounced word, while a standalone `(ja)` is the CA notation for
//! a token the transcriber heard but is not asserting was fully articulated.
//! The model records that as [`WordCategory::CAOmission`] with the parenthesized
//! material as ordinary [`WordContent::Text`], so downstream rules see a word
//! with content rather than a word made only of unspoken material.
//!
//! # Why this lives in the model and not in a parser
//!
//! Both parser backends performed this rewrite, in two hand-written copies, and
//! the copies had drifted on BOTH of their conditions:
//!
//! | | which words qualify | which category |
//! |---|---|---|
//! | tree-sitter | one `Shortening` plus any number of non-lexical markers | `None` or already `CAOmission` |
//! | re2c | `content.len() == 1`, nothing else allowed | `None` only |
//!
//! So `⌊(ja)`, an overlap marker followed by a parenthesized word and utterly
//! ordinary in CA corpora, normalized under tree-sitter and did not under re2c.
//! The consequence was a spurious E209 "word has no spoken content" on 246
//! occurrences in a 3,000-file corpus sample, reported by
//! `chatter validate --parser re2c` against files the default backend reads as
//! clean.
//!
//! One rule, one owner, so the two backends cannot disagree about it again.

use crate::alignment::helpers::{WordItemMut, walk_words_mut};
use crate::model::{Word, WordCategory, WordContent, WordShortening, WordText};

/// One qualifying shortening in its original word. Only `inspect` constructs
/// this association; a second shortening or other lexical content refuses it.
struct CaOmission<'word> {
    index: usize,
    shortening: &'word WordShortening,
}

impl<'word> CaOmission<'word> {
    fn inspect(word: &'word Word) -> Option<Self> {
        if matches!(word.category.as_ref(), Some(category) if *category != WordCategory::CAOmission)
        {
            return None;
        }
        let mut found = None;
        for (index, item) in word.content().iter().enumerate() {
            match item {
                WordContent::Shortening(shortening) => {
                    if found.is_some() {
                        return None;
                    }
                    found = Some(Self { index, shortening });
                }
                WordContent::Text(_)
                | WordContent::Phonetic(_)
                | WordContent::CompoundMarker(_) => return None,
                WordContent::OverlapPoint(_)
                | WordContent::CAElement(_)
                | WordContent::CADelimiter(_)
                | WordContent::StressMarker(_)
                | WordContent::Lengthening(_)
                | WordContent::SyllablePause(_)
                | WordContent::UnderlineBegin(_)
                | WordContent::UnderlineEnd(_)
                | WordContent::CliticBoundary(_) => {}
            }
        }
        found
    }
}

/// Whether fragment interpretation needs the CA option for this word.
/// This uses the normalization owner's shape admission, including non-lexical
/// markers, rather than a second one-item approximation in the main tier.
pub fn requires_ca_omission_context(word: &Word) -> bool {
    matches!(word.category, Some(WordCategory::CAOmission)) || CaOmission::inspect(word).is_some()
}

/// Rewrite a standalone CA-omission shortening into text, in place.
///
/// A no-op unless the word qualifies: exactly one [`WordContent::Shortening`],
/// no lexical content beside it, and a category that is either absent or
/// already [`WordCategory::CAOmission`]. Mixed content such as `(be)cause` is
/// a genuine shortening and is left exactly as it is.
///
/// Callers apply this only in CA mode; the function itself expresses which
/// SHAPES qualify, not the policy about when to ask.
pub fn normalize_ca_omission_word(word: &mut Word) {
    let Some(candidate) = CaOmission::inspect(word) else {
        return;
    };
    let index = candidate.index;
    let content = WordContent::Text(WordText::from(candidate.shortening.clone()));
    word.replace_content_at(index, content);
    word.category = Some(WordCategory::CAOmission);
}

/// Apply [`normalize_ca_omission_word`] to every word in a file's utterances.
///
/// # Why the WALK lives here too, and not in each parser
///
/// An earlier pass gave the word-level rule one owner and left the traversal
/// that finds words hand-rolled in both parser backends. That was the half that
/// was actually diverging: re2c's walk skipped `PhoGroup` and `SinGroup` at the
/// content level and `AnnotatedGroup`, `PhoGroup`, `SinGroup` and `Quotation`
/// inside brackets, both behind a `_ => {}`, and it normalized only the left
/// side of a replacement while the tree-sitter copy did both. So `‹(ja)›` and
/// `<(ja)> [//]` canonicalized under one backend and not the other, which is
/// the same divergence class the word-level unification was meant to close,
/// one container deeper.
///
/// [`walk_words_mut`] already owns "which content contains words", exhaustively
/// and with no catch-all, so a container added to the model reaches this
/// normalization for free.
pub fn normalize_ca_omissions_in_lines(lines: &mut [crate::model::Line]) {
    for line in lines {
        if let crate::model::Line::Utterance(utterance) = line {
            normalize_ca_omissions_in_main_tier(&mut utterance.main);
        }
    }
}

/// The same normalization for a single main tier, as the fragment APIs need.
pub fn normalize_ca_omissions_in_main_tier(main: &mut crate::model::MainTier) {
    walk_words_mut(main.content.content.as_mut_slice(), None, &mut |item| {
        match item {
            WordItemMut::Word(word) => normalize_ca_omission_word(word),
            WordItemMut::ReplacedWord(replaced) => {
                normalize_ca_omission_word(&mut replaced.word);
                for word in &mut replaced.replacement.words {
                    normalize_ca_omission_word(word);
                }
            }
            // A separator carries no word to normalize.
            WordItemMut::Separator(_) => {}
        }
    });
}
