//! Bracketed-content recursion for the walk helpers.
//!
//! The four `walk_bracketed_*` recursors (the bracketed twins of the top-level
//! `walk_content`/`walk_words` walkers) plus the two `should_skip_*` group-gating
//! predicates. Extracted verbatim from `walk/mod.rs`; the top-level walkers call
//! these through the `use bracketed::*;` re-export in the parent. The visited-item
//! enums ([`ContentItem`](super::ContentItem), [`WordItem`](super::WordItem), and
//! their `*Mut` twins) live in the parent module and are imported here.

use crate::alignment::helpers::{domain::TierDomain, rules::should_skip_group};
use crate::model::{BracketedItem, ContentAnnotation};

use super::{ContentItem, ContentItemMut, WordItem, WordItemMut};

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
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
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
            // Groups: descend into content
            BracketedItem::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_content(&annotated.inner.content.content, domain, f);
                }
            }
            BracketedItem::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content(&pho.content.content, domain, f);
                }
            }
            BracketedItem::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content(&sin.content.content, domain, f);
                }
            }
            BracketedItem::Quotation(quot) => {
                walk_bracketed_content(&quot.content.content, domain, f);
            }
            BracketedItem::Retrace(retrace) => {
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content(&retrace.content.content, domain, f);
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
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
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
            // Groups: descend into content
            BracketedItem::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_content_mut(&mut annotated.inner.content.content, domain, f);
                }
            }
            BracketedItem::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content_mut(&mut pho.content.content, domain, f);
                }
            }
            BracketedItem::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content_mut(&mut sin.content.content, domain, f);
                }
            }
            BracketedItem::Quotation(quot) => {
                walk_bracketed_content_mut(&mut quot.content.content, domain, f);
            }
            BracketedItem::Retrace(retrace) => {
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content_mut(&mut retrace.content.content, domain, f);
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
    f: &mut impl FnMut(WordItem<'a>),
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                f(WordItem::Word(word));
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    f(WordItem::Word(&annotated.inner));
                }
            }
            BracketedItem::ReplacedWord(replaced) => {
                f(WordItem::ReplacedWord(replaced));
            }
            BracketedItem::Separator(sep) => {
                f(WordItem::Separator(sep));
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_words(&annotated.inner.content.content, domain, f);
                }
            }
            BracketedItem::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words(&pho.content.content, domain, f);
                }
            }
            BracketedItem::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words(&sin.content.content, domain, f);
                }
            }
            BracketedItem::Quotation(quot) => {
                walk_bracketed_words(&quot.content.content, domain, f);
            }
            BracketedItem::Retrace(retrace) => {
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words(&retrace.content.content, domain, f);
                }
            }
            // Non-word bracketed items.
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::InternalBullet(_)
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
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
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
            BracketedItem::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_words_mut(&mut annotated.inner.content.content, domain, f);
                }
            }
            BracketedItem::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words_mut(&mut pho.content.content, domain, f);
                }
            }
            BracketedItem::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words_mut(&mut sin.content.content, domain, f);
                }
            }
            BracketedItem::Quotation(quot) => {
                walk_bracketed_words_mut(&mut quot.content.content, domain, f);
            }
            BracketedItem::Retrace(retrace) => {
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words_mut(&mut retrace.content.content, domain, f);
                }
            }
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::InternalBullet(_)
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

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Returns `true` when an annotated word/group should be skipped for the given domain.
///
/// Checks for alignment-ignore annotations (currently `[e]` exclude marker).
/// Retrace skipping is handled by the `Retrace` content variant directly.
pub(super) fn should_skip_annotated_group(
    annotations: &[ContentAnnotation],
    domain: Option<TierDomain>,
) -> bool {
    match domain {
        Some(d) => should_skip_group(annotations, d),
        None => false,
    }
}

/// Returns `true` when PhoGroup/SinGroup should be skipped.
///
/// Pho and Sin domains treat these as atomic units rather than recursing
/// into their word content.
pub(super) fn should_skip_pho_sin_group(domain: Option<TierDomain>) -> bool {
    matches!(domain, Some(TierDomain::Pho | TierDomain::Sin))
}
