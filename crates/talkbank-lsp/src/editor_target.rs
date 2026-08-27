//! Which utterance content the editor targets, and where it is.
//!
//! # One owner, after five
//!
//! `fn content_span(&UtteranceContent) -> Option<Span>` existed five times,
//! byte-identical and private, across the hover, highlight, goto and sidecar
//! paths. Every copy ended in `_ => None`, so when `UtteranceContent::Action`
//! was added on 2026-08-26 for bare `0`, all five silently began answering
//! `None` for it and the commonest item in a daylong corpus became
//! untargetable, with a compile error at none of them.
//!
//! # Two questions, and only one of them is the LSP's
//!
//! WHERE an item is belongs to the model, and is answered by
//! [`WordRef::span`](talkbank_model::model::WordRef::span) and
//! [`GroupRef::span`](talkbank_model::model::GroupRef::span). WHETHER the
//! editor targets it is this crate's policy, and is the only part written
//! here. Dispatching on [`ContentStructure`] rather than on the 28
//! `UtteranceContent` variants is what keeps them apart: a new variant is
//! classified once, in the model's `structure()`, instead of once there and
//! once here where the two could disagree.
//!
//! The first version of this module DID list all 28, on the stated grounds
//! that a model-side accessor was "blocked because `LeafRef` does not carry
//! its node". That was wrong twice over: this consumer answers `None` for
//! every leaf and so never needs a leaf's span, and the real gap was elsewhere
//! entirely, in `PhoGroup` and `SinGroup` having no span field at all.
//!
//! # What is excluded, and which kind of exclusion each is
//!
//! - **Phonological and sign groups**: the model records no span for them.
//!   A FACT, and `GroupRef::span` reports it.
//! - **Quotations**: they have spans; not targeting them is a POLICY, and it
//!   is inherited from the five copies rather than adjudicated here.
//! - **Retraces and every leaf**: also policy, also inherited. Retraces are
//!   the exclusion most worth revisiting, because the model's `%mor` counting
//!   descends INTO them, so a `%mor` item aligned to a retraced word currently
//!   has no goto target.

use talkbank_model::Span;
use talkbank_model::alignment::{TierDomain, count_tier_positions_until};
use talkbank_model::model::{ContentStructure, GroupKind, UtteranceContent};

/// The span an editor feature should target for `content`, if any.
#[inline]
#[must_use]
pub(crate) fn editor_target_span(content: &UtteranceContent) -> Option<Span> {
    match content.structure() {
        ContentStructure::Word(word) => Some(word.span()),
        ContentStructure::Group(group) => match group.kind() {
            // The bracketed span, so a hover covers the whole group.
            GroupKind::Angle => group.span(),
            GroupKind::Quotation | GroupKind::Pho | GroupKind::Sin => None,
        },
        ContentStructure::Retrace(_) | ContentStructure::Leaf(_) => None,
    }
}

/// Whether `offset` falls inside `span`, INCLUSIVE of both ends.
///
/// Existed five times in this crate, byte-identical, plus once more as a
/// closure. The model has `span_contains_half_open`, which is EXCLUSIVE at the
/// end and treats a dummy span as containing nothing; the two are not
/// interchangeable, and an editor wants the inclusive form so a cursor resting
/// on the closing character still targets the item. Kept separate deliberately
/// rather than merged with the model's.
#[inline]
#[must_use]
pub(crate) fn span_contains(span: Span, offset: u32) -> bool {
    offset >= span.start && offset <= span.end
}

/// The index of the targetable item under `offset`, if any.
///
/// The VERB, and the reason it lives beside the primitive: folding
/// `editor_target_span` alone would have left this duplicated one level up,
/// which is the trap the house rules name as "better primitives left public
/// just move the duplication to the next caller". It was byte-identical in the
/// hover and highlight paths.
#[must_use]
pub(crate) fn find_content_index_at_offset(
    content: &[UtteranceContent],
    offset: u32,
) -> Option<usize> {
    content
        .iter()
        .enumerate()
        .find_map(|(index, item)| match editor_target_span(item) {
            Some(span) if span_contains(span, offset) => Some(index),
            Some(_) | None => None,
        })
}

/// The `%mor` alignment index of the item at `index`, if it has one.
///
/// # Why this is one function and not two
///
/// Both callers used to ask two questions: `count_alignable_before` for the
/// index, and `is_alignable_content` for whether the item had one. That is a
/// bool beside the number it qualifies, and the pair can disagree. One caller
/// even computed the index BEFORE checking, so a non-aligned item briefly had
/// an alignment position that meant nothing; it was discarded by the early
/// return, but nothing in the types said it had to be.
///
/// An `Option` says it instead: there is no index for an item that contributes
/// none, and no way to hold one. That also removes a traversal, since the two
/// questions were differencing counts over the same content.
///
/// The POLICY of which items contribute is the model's, not this crate's; this
/// only reads `count_tier_positions_until`.
#[must_use]
pub(crate) fn mor_alignment_index(content: &[UtteranceContent], index: usize) -> Option<usize> {
    let before = count_tier_positions_until(content, index, TierDomain::Mor);
    let after = count_tier_positions_until(content, index + 1, TierDomain::Mor);
    (after > before).then_some(before)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use talkbank_model::model::{
        Action, BracketedContent, BracketedItem, Group, Pause, PauseDuration, PhoGroup, Quotation,
        Word,
    };

    fn one_word() -> BracketedContent {
        BracketedContent::new(vec![BracketedItem::Word(Box::new(Word::simple("hello")))])
    }

    /// What the editor targets, per structural category.
    ///
    /// SURVIVES a type, as POLICY: which items get hover, goto and highlight
    /// is a choice with real alternatives. The structural dispatch stops a new
    /// `UtteranceContent` variant changing any of these silently, but it
    /// cannot pin the CHOICES, so they are asserted here.
    ///
    /// The bare-action row is why this test exists. All five copies this
    /// module replaced answered `None` for it the day
    /// `UtteranceContent::Action` was added, and nothing failed, because this
    /// crate had no test over the question at all.
    #[test]
    fn the_editor_targets_words_and_angle_groups_and_nothing_else() {
        assert!(
            editor_target_span(&UtteranceContent::Word(Box::new(Word::simple("hello")))).is_some(),
            "a bare word is the primary target"
        );
        assert!(
            editor_target_span(&UtteranceContent::Group(Group::new(one_word()))).is_some(),
            "an angle group is targeted as a whole"
        );
        assert_eq!(
            editor_target_span(&UtteranceContent::Action(Action::new())),
            None,
            "a bare action is policy-excluded, not accidentally missed"
        );
        assert_eq!(
            editor_target_span(&UtteranceContent::Quotation(Quotation::new(one_word()))),
            None,
            "a quotation HAS a span; excluding it is an inherited policy"
        );
        assert_eq!(
            editor_target_span(&UtteranceContent::PhoGroup(PhoGroup::new(one_word()))),
            None,
            "a phonological group has no span in the model at all"
        );
    }

    /// Which items contribute a `%mor` alignment position, and which do not.
    ///
    /// Written 2026-08-26 because `cargo mutants` reported both of this
    /// function's mutants MISSED: replacing its body with `true`, and widening
    /// its `>` to `>=`, each left the whole suite green. It had been moved into
    /// this module from two byte-identical copies without anything pinning what
    /// it answers, which is the exact gap that shipped a regression earlier the
    /// same day.
    ///
    /// SURVIVES a type, as a MEASUREMENT: it reads the model's `%mor` counting,
    /// and what that counting includes is the model's policy, not this crate's.
    #[test]
    fn only_alignable_items_have_a_mor_index() {
        let content = vec![
            UtteranceContent::Word(Box::new(Word::simple("hello"))),
            UtteranceContent::Pause(Pause::new(PauseDuration::Short)),
        ];
        assert!(
            mor_alignment_index(&content, 0).is_some(),
            "a word contributes a %mor position"
        );
        // Kills both mutants: a body of `true` fails here, and `>=` would read
        // an unchanged count as a contribution.
        assert!(
            mor_alignment_index(&content, 1).is_none(),
            "a pause contributes none, so there is no index to hold"
        );
    }

    /// `span_contains` is inclusive at BOTH ends, unlike the model's half-open
    /// `span_contains_half_open`.
    ///
    /// SURVIVES a type: a boundary convention two functions disagree about on
    /// purpose. A cursor resting on the closing character still targets the
    /// item, which is what an editor wants.
    #[test]
    fn span_contains_includes_both_ends() {
        let span = Span { start: 10, end: 20 };
        assert!(span_contains(span, 10), "start is inside");
        assert!(span_contains(span, 20), "end is inside, unlike the model's");
        assert!(!span_contains(span, 9));
        assert!(!span_contains(span, 21));
    }
}
