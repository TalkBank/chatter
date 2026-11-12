//! Bracketed-content recursion for the walk helpers.
//!
//! The four `walk_bracketed_*` recursors, the bracketed twins of the top-level
//! `walk_content`/`walk_words` walkers. Split out of `walk/mod.rs` to keep both
//! files browseable; the parent imports them by name.
//!
//! Gating is NOT here, for containers or for annotated words:
//! `helpers::descent` owns it for every traversal in the crate. The visited-item
//! enums ([`ContentItem`](super::ContentItem), [`WordItem`](super::WordItem), and
//! their `*Mut` twins) live in the parent module and are imported here.

// The sibling gating modules (`count.rs`, `overlap.rs`) carry this and these
// depend on exhaustiveness harder than either: a new container variant that
// lands in the wrong arm here silently stops eight walkers descending.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::alignment::helpers::domain::TierDomain;
use crate::model::BracketedItem;

use super::super::descent::{descend, descend_mut, excluded_by_annotations};
use super::{
    BoundedWordStep, ContentItem, ContentItemMut, LanguageScope, WordItem, WordItemMut,
    annotated_word_steps, container_steps, replaced_word_steps,
};

pub(super) fn walk_bracketed_content<'a>(
    items: &'a [BracketedItem],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(ContentItem<'a>),
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                f(ContentItem::Word(word));
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if !excluded_by_annotations(&annotated.scoped_annotations, domain) {
                    f(ContentItem::Word(&annotated.inner));
                }
            }
            BracketedItem::ReplacedWord(replaced) => {
                f(ContentItem::ReplacedWord(replaced));
            }
            BracketedItem::Separator(sep) => {
                f(ContentItem::Separator(sep));
            }
            BracketedItem::Event(event) => {
                f(ContentItem::Event(event));
            }
            BracketedItem::AnnotatedEvent(annotated) => {
                f(ContentItem::Event(&annotated.inner));
            }
            BracketedItem::Pause(pause) => {
                f(ContentItem::Pause(pause));
            }
            BracketedItem::Action(action) => {
                f(ContentItem::Action(action));
            }
            BracketedItem::AnnotatedAction(annotated) => {
                f(ContentItem::Action(&annotated.inner));
            }
            BracketedItem::OverlapPoint(op) => {
                f(ContentItem::OverlapPoint(op));
            }
            BracketedItem::InternalBullet(bullet) => {
                f(ContentItem::InternalBullet(bullet));
            }
            BracketedItem::Freecode(fc) => {
                f(ContentItem::Freecode(fc));
            }
            BracketedItem::OrphanAnnotation(raw) => f(ContentItem::OrphanAnnotation(raw)),
            BracketedItem::LongFeatureBegin(lfb) => {
                f(ContentItem::LongFeatureBegin(lfb));
            }
            BracketedItem::LongFeatureEnd(lfe) => {
                f(ContentItem::LongFeatureEnd(lfe));
            }
            BracketedItem::UnderlineBegin(marker) => {
                f(ContentItem::UnderlineBegin(marker));
            }
            BracketedItem::UnderlineEnd(marker) => {
                f(ContentItem::UnderlineEnd(marker));
            }
            BracketedItem::NonvocalBegin(nv) => {
                f(ContentItem::NonvocalBegin(nv));
            }
            BracketedItem::NonvocalEnd(nv) => {
                f(ContentItem::NonvocalEnd(nv));
            }
            BracketedItem::NonvocalSimple(nv) => {
                f(ContentItem::NonvocalSimple(nv));
            }
            BracketedItem::OtherSpokenEvent(ose) => {
                f(ContentItem::OtherSpokenEvent(ose));
            }
            // Containers: ONE arm, and `descent::descend` owns the rule.
            BracketedItem::Group(_)
            | BracketedItem::AnnotatedGroup(_)
            | BracketedItem::PhoGroup(_)
            | BracketedItem::SinGroup(_)
            | BracketedItem::Quotation(_)
            | BracketedItem::AnnotatedQuotation(_)
            | BracketedItem::Retrace(_)
            | BracketedItem::AnnotatedRetrace(_) => {
                if let Some(into) = descend(item.structure(), domain).entered() {
                    walk_bracketed_content(&into.content().content, domain, f);
                }
            }
        }
    }
}

pub(super) fn walk_bracketed_content_mut<'a>(
    items: &'a mut [BracketedItem],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(ContentItemMut<'a>),
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                f(ContentItemMut::Word(word));
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if !excluded_by_annotations(&annotated.scoped_annotations, domain) {
                    f(ContentItemMut::Word(&mut annotated.inner));
                }
            }
            BracketedItem::ReplacedWord(replaced) => {
                f(ContentItemMut::ReplacedWord(replaced));
            }
            BracketedItem::Separator(sep) => {
                f(ContentItemMut::Separator(sep));
            }
            BracketedItem::Event(event) => {
                f(ContentItemMut::Event(event));
            }
            BracketedItem::AnnotatedEvent(annotated) => {
                f(ContentItemMut::Event(&mut annotated.inner));
            }
            BracketedItem::Pause(pause) => {
                f(ContentItemMut::Pause(pause));
            }
            BracketedItem::Action(action) => {
                f(ContentItemMut::Action(action));
            }
            BracketedItem::AnnotatedAction(annotated) => {
                f(ContentItemMut::Action(&mut annotated.inner));
            }
            BracketedItem::OverlapPoint(op) => {
                f(ContentItemMut::OverlapPoint(op));
            }
            BracketedItem::InternalBullet(bullet) => {
                f(ContentItemMut::InternalBullet(bullet));
            }
            BracketedItem::Freecode(fc) => {
                f(ContentItemMut::Freecode(fc));
            }
            BracketedItem::OrphanAnnotation(raw) => f(ContentItemMut::OrphanAnnotation(raw)),
            BracketedItem::LongFeatureBegin(lfb) => {
                f(ContentItemMut::LongFeatureBegin(lfb));
            }
            BracketedItem::LongFeatureEnd(lfe) => {
                f(ContentItemMut::LongFeatureEnd(lfe));
            }
            BracketedItem::UnderlineBegin(marker) => {
                f(ContentItemMut::UnderlineBegin(marker));
            }
            BracketedItem::UnderlineEnd(marker) => {
                f(ContentItemMut::UnderlineEnd(marker));
            }
            BracketedItem::NonvocalBegin(nv) => {
                f(ContentItemMut::NonvocalBegin(nv));
            }
            BracketedItem::NonvocalEnd(nv) => {
                f(ContentItemMut::NonvocalEnd(nv));
            }
            BracketedItem::NonvocalSimple(nv) => {
                f(ContentItemMut::NonvocalSimple(nv));
            }
            BracketedItem::OtherSpokenEvent(ose) => {
                f(ContentItemMut::OtherSpokenEvent(ose));
            }
            // Containers: ONE gate per rule, not one per variant. These were
            // one arm per variant applying three rules, and they drifted: the two
            // `AnnotatedQuotation` arms shipped ungated on 2026-08-26 while
            // `count.rs` gated the same variant, so the walkers disagreed about
            // one node. `container_mut` hands over the kind, the annotations
            // and the content together, so the gate cannot be dropped by
            // copying the wrong neighbour.
            //
            // The annotations sit on the retrace WRAPPER, are not words, and
            // are not walked; only the retraced content is, which is why the
            // retrace rule reads the domain alone.
            BracketedItem::Group(_)
            | BracketedItem::AnnotatedGroup(_)
            | BracketedItem::PhoGroup(_)
            | BracketedItem::SinGroup(_)
            | BracketedItem::Quotation(_)
            | BracketedItem::AnnotatedQuotation(_)
            | BracketedItem::Retrace(_)
            | BracketedItem::AnnotatedRetrace(_) => {
                if let Some(content) = descend_mut(item.container_mut(), domain) {
                    walk_bracketed_content_mut(content.content.as_mut_slice(), domain, f);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Bracketed-level helpers for walk_words
// ---------------------------------------------------------------------------

pub(super) fn walk_bracketed_words<'a>(
    items: &'a [BracketedItem],
    domain: Option<TierDomain>,
    scope: LanguageScope<'a>,
    f: &mut impl FnMut(BoundedWordStep<'a>),
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                f(BoundedWordStep::Leaf(WordItem::Word(word), scope));
            }
            BracketedItem::AnnotatedWord(annotated) => {
                annotated_word_steps(
                    &annotated.inner,
                    &annotated.scoped_annotations,
                    domain,
                    scope,
                    f,
                );
            }
            BracketedItem::ReplacedWord(replaced) => {
                replaced_word_steps(replaced, scope, f);
            }
            BracketedItem::Separator(sep) => {
                f(BoundedWordStep::Leaf(WordItem::Separator(sep), scope));
            }
            // Containers: ONE arm, through the helper the top level uses.
            BracketedItem::Group(_)
            | BracketedItem::AnnotatedGroup(_)
            | BracketedItem::PhoGroup(_)
            | BracketedItem::SinGroup(_)
            | BracketedItem::Quotation(_)
            | BracketedItem::AnnotatedQuotation(_)
            | BracketedItem::Retrace(_)
            | BracketedItem::AnnotatedRetrace(_) => {
                container_steps(item.structure(), domain, scope, f);
            }
            // Non-word bracketed items: each stands between leaves.
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::OrphanAnnotation(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => f(BoundedWordStep::Boundary),
        }
    }
}

pub(super) fn walk_bracketed_words_mut<'a>(
    items: &'a mut [BracketedItem],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(WordItemMut<'a>),
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                f(WordItemMut::Word(word));
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if !excluded_by_annotations(&annotated.scoped_annotations, domain) {
                    let a = annotated.as_mut();
                    f(WordItemMut::Word(&mut a.inner));
                }
            }
            BracketedItem::ReplacedWord(replaced) => {
                f(WordItemMut::ReplacedWord(replaced));
            }
            BracketedItem::Separator(sep) => {
                f(WordItemMut::Separator(sep));
            }
            // Containers: one gate per RULE. See `walk_bracketed_content_mut`
            // for why these stopped being one arm per variant.
            BracketedItem::Group(_)
            | BracketedItem::AnnotatedGroup(_)
            | BracketedItem::PhoGroup(_)
            | BracketedItem::SinGroup(_)
            | BracketedItem::Quotation(_)
            | BracketedItem::AnnotatedQuotation(_)
            | BracketedItem::Retrace(_)
            | BracketedItem::AnnotatedRetrace(_) => {
                if let Some(content) = descend_mut(item.container_mut(), domain) {
                    walk_bracketed_words_mut(content.content.as_mut_slice(), domain, f);
                }
            }
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::OrphanAnnotation(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}
