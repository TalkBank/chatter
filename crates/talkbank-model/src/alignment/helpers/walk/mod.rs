//! Content tree walkers for traversing [`UtteranceContent`] and [`BracketedItem`].
//!
//! Centralizes the recursive traversal of [`UtteranceContent`] (24 variants) and
//! [`BracketedItem`] (22 variants) so callers provide only item-handling logic.
//! Domain-aware group gating (retrace skip for Mor, PhoGroup/SinGroup skip for
//! Pho/Sin) is handled once here.
//!
//! # Walkers
//!
//! - [`walk_content`] / [`walk_content_mut`], emit ALL non-container items
//! - [`walk_words`] / [`walk_words_mut`], convenience filter for word-like items only
//!
//! # Deprecated aliases
//!
//! [`walk_words`] / [`walk_words_mut`] delegate to [`walk_words`] / [`walk_words_mut`].
//! [`ContentLeaf`] / [`ContentLeafMut`] are type aliases for [`WordItem`] / [`WordItemMut`].

use crate::alignment::helpers::domain::TierDomain;
use crate::model::{
    Action, Bullet, CodeSwitchSpan, ContentAnnotation, Event, Freecode, LongFeatureBegin,
    LongFeatureEnd, NonvocalBegin, NonvocalEnd, NonvocalSimple, OtherSpokenEvent, OverlapPoint,
    Pause, ReplacedWord, Separator, UnderlineMarker, UtteranceContent, Word,
};

// The bracketed-content recursors and group-gating predicates live in a sibling
// submodule to keep this file browseable; the top-level walkers below call
// them by their bare names.
mod bracketed;
use bracketed::{
    should_skip_annotated_group, should_skip_pho_sin_group, walk_bracketed_content,
    walk_bracketed_content_mut, walk_bracketed_words, walk_bracketed_words_mut,
};

// ---------------------------------------------------------------------------
// ContentItem, every non-container item
// ---------------------------------------------------------------------------

/// Every non-container content item visited during in-order traversal.
/// Groups are descended into transparently. Annotated wrappers are
/// unwrapped to expose the inner item.
pub enum ContentItem<'a> {
    /// Plain word or inner word of an `AnnotatedWord`.
    Word(&'a Word),
    /// Replaced word (`word [: replacement]`).
    ReplacedWord(&'a ReplacedWord),
    /// Separator (comma, tag, vocative, etc.).
    Separator(&'a Separator),
    /// Sound event (`&=laughs`) or inner event of an `AnnotatedEvent`.
    Event(&'a Event),
    /// Pause (`(.)`, `(..)`, `(...)`, or timed).
    Pause(&'a Pause),
    /// Action (`&%action`) or inner action of an `AnnotatedAction`.
    Action(&'a Action),
    /// CA overlap boundary marker.
    OverlapPoint(&'a OverlapPoint),
    /// Other-speaker spoken event (`&*SPK:word`).
    OtherSpokenEvent(&'a OtherSpokenEvent),
    /// Freecode inline annotation (`[^ comment]`).
    Freecode(&'a Freecode),
    /// Internal timing bullet (mid-utterance media timestamp).
    InternalBullet(&'a Bullet),
    /// Long feature scope begin (`&{l=LABEL`).
    LongFeatureBegin(&'a LongFeatureBegin),
    /// Long feature scope end (`&}l=LABEL`).
    LongFeatureEnd(&'a LongFeatureEnd),
    /// Underline begin marker.
    UnderlineBegin(&'a UnderlineMarker),
    /// Underline end marker.
    UnderlineEnd(&'a UnderlineMarker),
    /// Nonvocal scope begin (`&{n=LABEL`).
    NonvocalBegin(&'a NonvocalBegin),
    /// Nonvocal scope end (`&}n=LABEL`).
    NonvocalEnd(&'a NonvocalEnd),
    /// Simple nonvocal marker (`&{n=LABEL}`).
    NonvocalSimple(&'a NonvocalSimple),
}
impl ContentItem<'_> {
    /// The SOURCE span of this item when it is a comma separator.
    ///
    /// `None` for a comma with no source position. Both callers exist to reason
    /// about byte offsets, and under the re2c front end every separator carries
    /// `Span::DUMMY` (`re2c/convert/items.rs` stamps it unconditionally), so a
    /// comma that is real but position-less must not read as a position. One
    /// caller guarded `!= Span::DUMMY` and the other did not, which meant `,,`
    /// reported E258 at byte 0 under that parser. Folding the guard in here
    /// makes the asymmetry unrepresentable rather than remembered.
    pub fn comma_span(&self) -> Option<crate::Span> {
        let Self::Separator(separator) = self else {
            return None;
        };
        separator
            .is_comma()
            .then(|| separator.span())
            .filter(|span| *span != crate::Span::DUMMY)
    }

    /// The SOURCE span of this item when it is a word-family item.
    ///
    /// One owner for a partition that `comma.rs::word_start` and
    /// `spacing.rs::word_end` each spelled out over all 17 variants, differing
    /// only in which end they read.
    pub fn word_span(&self) -> Option<crate::Span> {
        match self {
            Self::Word(word) => Some(word.span),
            // The ORIGINAL word's span, not the wrapper's: both callers
            // read `replaced.word.span`, because the editorial replacement is
            // not what occupies source position here.
            Self::ReplacedWord(replaced) => Some(replaced.word.span),
            Self::Separator(_)
            | Self::Event(_)
            | Self::Pause(_)
            | Self::Action(_)
            | Self::OverlapPoint(_)
            | Self::OtherSpokenEvent(_)
            | Self::Freecode(_)
            | Self::InternalBullet(_)
            | Self::LongFeatureBegin(_)
            | Self::LongFeatureEnd(_)
            | Self::UnderlineBegin(_)
            | Self::UnderlineEnd(_)
            | Self::NonvocalBegin(_)
            | Self::NonvocalEnd(_)
            | Self::NonvocalSimple(_) => None,
        }
    }
}

/// Mutable version of [`ContentItem`].
pub enum ContentItemMut<'a> {
    /// Mutable word reference.
    Word(&'a mut Word),
    /// Mutable replaced word reference.
    ReplacedWord(&'a mut ReplacedWord),
    /// Mutable separator reference.
    Separator(&'a mut Separator),
    /// Mutable event reference.
    Event(&'a mut Event),
    /// Mutable pause reference.
    Pause(&'a mut Pause),
    /// Mutable action reference.
    Action(&'a mut Action),
    /// Mutable overlap point reference.
    OverlapPoint(&'a mut OverlapPoint),
    /// Mutable other-speaker spoken event reference.
    OtherSpokenEvent(&'a mut OtherSpokenEvent),
    /// Mutable freecode reference.
    Freecode(&'a mut Freecode),
    /// Mutable internal bullet reference.
    InternalBullet(&'a mut Bullet),
    /// Mutable long feature begin reference.
    LongFeatureBegin(&'a mut LongFeatureBegin),
    /// Mutable long feature end reference.
    LongFeatureEnd(&'a mut LongFeatureEnd),
    /// Mutable underline begin reference.
    UnderlineBegin(&'a mut UnderlineMarker),
    /// Mutable underline end reference.
    UnderlineEnd(&'a mut UnderlineMarker),
    /// Mutable nonvocal begin reference.
    NonvocalBegin(&'a mut NonvocalBegin),
    /// Mutable nonvocal end reference.
    NonvocalEnd(&'a mut NonvocalEnd),
    /// Mutable nonvocal simple reference.
    NonvocalSimple(&'a mut NonvocalSimple),
}

// ---------------------------------------------------------------------------
// WordItem, word-like leaf items only
// ---------------------------------------------------------------------------

/// Word-like leaf item yielded by [`walk_words`].
///
/// A word-like content item visited during in-order traversal.
/// Groups are descended into transparently. AnnotatedWord is unwrapped.
pub enum WordItem<'a> {
    /// A word (bare or unwrapped from AnnotatedWord).
    Word(&'a Word),
    /// Replaced word (`word [: replacement]`).
    ReplacedWord(&'a ReplacedWord),
    /// Separator (comma, tag, vocative, etc.).
    Separator(&'a Separator),
}

/// Mutable version of [`WordItem`].
pub enum WordItemMut<'a> {
    /// Mutable word reference.
    Word(&'a mut Word),
    /// Mutable replaced word reference.
    ReplacedWord(&'a mut ReplacedWord),
    /// Mutable separator reference.
    Separator(&'a mut Separator),
}

// ---------------------------------------------------------------------------
// Deprecated aliases
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// walk_content, emit ALL non-container items
// ---------------------------------------------------------------------------

/// Walk utterance content and call `f` for every non-container item.
///
/// Groups are descended into transparently. Annotated wrappers are unwrapped
/// to expose the inner item. Domain gating applies as with [`walk_words`]:
/// `Some(Mor)` skips retrace/reformulation groups, `Some(Pho|Sin)` skips
/// PhoGroup/SinGroup.
pub fn walk_content<'a>(
    content: &'a [UtteranceContent],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(ContentItem<'a>),
) {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                f(ContentItem::Word(word));
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                // Single-word retraces (e.g. `cup [/]`) are AnnotatedWord with
                // retrace annotations, skip them in the Mor domain just like
                // multi-word AnnotatedGroup retraces.
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    f(ContentItem::Word(&annotated.inner));
                }
            }
            UtteranceContent::ReplacedWord(replaced) => {
                f(ContentItem::ReplacedWord(replaced));
            }
            UtteranceContent::Separator(sep) => {
                f(ContentItem::Separator(sep));
            }
            UtteranceContent::Event(event) => {
                f(ContentItem::Event(event));
            }
            UtteranceContent::AnnotatedEvent(annotated) => {
                f(ContentItem::Event(&annotated.inner));
            }
            UtteranceContent::Pause(pause) => {
                f(ContentItem::Pause(pause));
            }
            UtteranceContent::AnnotatedAction(annotated) => {
                f(ContentItem::Action(&annotated.inner));
            }
            UtteranceContent::Freecode(fc) => {
                f(ContentItem::Freecode(fc));
            }
            UtteranceContent::OverlapPoint(op) => {
                f(ContentItem::OverlapPoint(op));
            }
            UtteranceContent::InternalBullet(bullet) => {
                f(ContentItem::InternalBullet(bullet));
            }
            UtteranceContent::LongFeatureBegin(lfb) => {
                f(ContentItem::LongFeatureBegin(lfb));
            }
            UtteranceContent::LongFeatureEnd(lfe) => {
                f(ContentItem::LongFeatureEnd(lfe));
            }
            UtteranceContent::UnderlineBegin(marker) => {
                f(ContentItem::UnderlineBegin(marker));
            }
            UtteranceContent::UnderlineEnd(marker) => {
                f(ContentItem::UnderlineEnd(marker));
            }
            UtteranceContent::NonvocalBegin(nv) => {
                f(ContentItem::NonvocalBegin(nv));
            }
            UtteranceContent::NonvocalEnd(nv) => {
                f(ContentItem::NonvocalEnd(nv));
            }
            UtteranceContent::NonvocalSimple(nv) => {
                f(ContentItem::NonvocalSimple(nv));
            }
            UtteranceContent::OtherSpokenEvent(ose) => {
                f(ContentItem::OtherSpokenEvent(ose));
            }
            // Groups: descend into content
            UtteranceContent::Group(group) => {
                walk_bracketed_content(&group.content.content, domain, f);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_content(&annotated.inner.content.content, domain, f);
                }
            }
            UtteranceContent::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content(&pho.content.content, domain, f);
                }
            }
            UtteranceContent::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content(&sin.content.content, domain, f);
                }
            }
            UtteranceContent::Quotation(quot) => {
                walk_bracketed_content(&quot.content.content, domain, f);
            }
            UtteranceContent::Retrace(retrace) => {
                // Retrace content is excluded from %mor (not morphologically analyzed),
                // but included in %pho/%sin/%wor and for domain-unspecified walks.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content(&retrace.content.content, domain, f);
                }
            }
            UtteranceContent::AnnotatedRetrace(annotated) => {
                // Same rule as the bare form. The annotations sit on the
                // wrapper, are not words, and are not walked; only the retraced
                // content is.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content(&annotated.inner.content.content, domain, f);
                }
            }
        }
    }
}

/// Walk utterance content mutably and call `f` for every non-container item.
///
/// Same domain-aware gating as [`walk_content`].
pub fn walk_content_mut<'a>(
    content: &'a mut [UtteranceContent],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(ContentItemMut<'a>),
) {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                f(ContentItemMut::Word(word));
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    f(ContentItemMut::Word(&mut annotated.inner));
                }
            }
            UtteranceContent::ReplacedWord(replaced) => {
                f(ContentItemMut::ReplacedWord(replaced));
            }
            UtteranceContent::Separator(sep) => {
                f(ContentItemMut::Separator(sep));
            }
            UtteranceContent::Event(event) => {
                f(ContentItemMut::Event(event));
            }
            UtteranceContent::AnnotatedEvent(annotated) => {
                f(ContentItemMut::Event(&mut annotated.inner));
            }
            UtteranceContent::Pause(pause) => {
                f(ContentItemMut::Pause(pause));
            }
            UtteranceContent::AnnotatedAction(annotated) => {
                f(ContentItemMut::Action(&mut annotated.inner));
            }
            UtteranceContent::Freecode(fc) => {
                f(ContentItemMut::Freecode(fc));
            }
            UtteranceContent::OverlapPoint(op) => {
                f(ContentItemMut::OverlapPoint(op));
            }
            UtteranceContent::InternalBullet(bullet) => {
                f(ContentItemMut::InternalBullet(bullet));
            }
            UtteranceContent::LongFeatureBegin(lfb) => {
                f(ContentItemMut::LongFeatureBegin(lfb));
            }
            UtteranceContent::LongFeatureEnd(lfe) => {
                f(ContentItemMut::LongFeatureEnd(lfe));
            }
            UtteranceContent::UnderlineBegin(marker) => {
                f(ContentItemMut::UnderlineBegin(marker));
            }
            UtteranceContent::UnderlineEnd(marker) => {
                f(ContentItemMut::UnderlineEnd(marker));
            }
            UtteranceContent::NonvocalBegin(nv) => {
                f(ContentItemMut::NonvocalBegin(nv));
            }
            UtteranceContent::NonvocalEnd(nv) => {
                f(ContentItemMut::NonvocalEnd(nv));
            }
            UtteranceContent::NonvocalSimple(nv) => {
                f(ContentItemMut::NonvocalSimple(nv));
            }
            UtteranceContent::OtherSpokenEvent(ose) => {
                f(ContentItemMut::OtherSpokenEvent(ose));
            }
            // Groups: descend into content
            UtteranceContent::Group(group) => {
                walk_bracketed_content_mut(group.content.content.as_mut_slice(), domain, f);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_content_mut(
                        annotated.inner.content.content.as_mut_slice(),
                        domain,
                        f,
                    );
                }
            }
            UtteranceContent::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content_mut(pho.content.content.as_mut_slice(), domain, f);
                }
            }
            UtteranceContent::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content_mut(sin.content.content.as_mut_slice(), domain, f);
                }
            }
            UtteranceContent::Quotation(quot) => {
                walk_bracketed_content_mut(quot.content.content.as_mut_slice(), domain, f);
            }
            UtteranceContent::Retrace(retrace) => {
                // Retrace content is excluded from %mor (not morphologically analyzed),
                // but included in %pho/%sin/%wor and for domain-unspecified walks.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content_mut(retrace.content.content.as_mut_slice(), domain, f);
                }
            }
            UtteranceContent::AnnotatedRetrace(annotated) => {
                // Same rule as the bare form. The annotations sit on the
                // wrapper, are not words, and are not walked; only the retraced
                // content is.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content_mut(
                        annotated.inner.content.content.as_mut_slice(),
                        domain,
                        f,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// walk_words, word-like items only (replacement for walk_words)
// ---------------------------------------------------------------------------

/// The language scope a walked leaf sits in.
///
/// A word inside `<...> [@s]` takes the span's language exactly as if it
/// carried the `@s` suffix itself, so the resolver needs to know which scope
/// produced the leaf. Carrying that as a VALUE handed to the callback, rather
/// than as a flag the caller maintains across calls, is what keeps the rule out
/// of convention: there is no state to forget to clear when the walk leaves the
/// group.
///
/// Nesting resolves innermost-wins: a span inside a span replaces the outer
/// scope for its own contents, which is the only reading under which each word
/// has one answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageScope<'a> {
    /// No enclosing code-switch span. The word's own marker and the utterance
    /// language decide, as they always have.
    Utterance,

    /// Inside a `<...> [@s]` or `<...> [@s:lang]` span.
    CodeSwitch(&'a CodeSwitchSpan),
}

/// Walk utterance content and call `f` for each word-like leaf item, telling it
/// which [`LanguageScope`] the leaf was found in.
///
/// A convenience filter over the content walk that emits only words, replaced
/// words and separators.
///
/// When `domain` is `Some(Mor)`, annotated groups with retrace/reformulation
/// annotations are skipped. When `domain` is `Some(Pho)` or `Some(Sin)`,
/// PhoGroup and SinGroup are skipped (treated as atomic units by those
/// domains). When `domain` is `None`, all groups are recursed unconditionally.
///
/// This is the one descent for word-like leaves: [`walk_words`] wraps it and
/// discards the scope, so the two cannot disagree about which leaves exist. A
/// private second walk with its own leaf set is the bug this module's history
/// is made of.
///
/// An earlier version of this paragraph claimed
/// `model::content::main_tier::language_switch` still ran a walk of its own. It
/// did not; it called [`walk_words`], the scope-DISCARDING wrapper, which was a
/// different and worse problem: its predicate deletes per-word `@s` markers, and
/// without the scope it could not see that an enclosing span would inherit the
/// word. It now calls this function.
pub fn walk_words_scoped<'a>(
    content: &'a [UtteranceContent],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(WordItem<'a>, LanguageScope<'a>),
) {
    walk_words_in_scope(content, domain, LanguageScope::Utterance, f);
}

impl<'a> LanguageScope<'a> {
    /// The scope in force INSIDE a group carrying `annotations`.
    ///
    /// A `<...> [@s]` group opens a code-switch scope for its contents;
    /// anything else leaves the enclosing scope alone. Innermost wins, so a
    /// nested span replaces this one rather than combining with it.
    ///
    /// One owner on purpose. This was spelled out at both descent sites, in two
    /// files, so "innermost wins" was a rule two call sites had to keep
    /// agreeing on and a change to precedence was two edits that could silently
    /// disagree.
    ///
    /// At most one span per group is meaningful; a second is a validation
    /// finding rather than something to merge, so the first is taken. Nothing
    /// reports the second yet.
    pub(super) fn inside(self, annotations: &'a [ContentAnnotation]) -> Self {
        match Self::selected_by(annotations) {
            Some(span) => Self::CodeSwitch(span),
            None => self,
        }
    }

    /// The span governing this scope, if any.
    ///
    /// The bridge to [`GoverningMarker::of`], which takes the `Option` rather
    /// than this enum: the walk threads a scope, word validation stores one,
    /// and only this accessor converts between them.
    #[must_use]
    pub fn span(self) -> Option<&'a CodeSwitchSpan> {
        match self {
            Self::CodeSwitch(span) => Some(span),
            Self::Utterance => None,
        }
    }

    /// The span an annotation list opens, if any.
    ///
    /// The ONE answer to "does this annotation list open a code-switch scope",
    /// shared by the alignment walk (which threads a borrowed scope) and by
    /// word VALIDATION (which stores an owned span on its context). Those two
    /// carry the scope differently and must not decide it differently: a word
    /// whose metadata says the span governs it, but whose `E220`/`E763` checks
    /// ran against the tier language, is the exact disagreement this prevents.
    #[must_use]
    pub fn selected_by(annotations: &[ContentAnnotation]) -> Option<&CodeSwitchSpan> {
        annotations.iter().find_map(|annotation| match annotation {
            ContentAnnotation::CodeSwitch(span) => Some(span),
            _ => None,
        })
    }
}

/// Walk utterance content and call `f` for each word-like leaf item.
///
/// Thin wrapper over [`walk_words_scoped`] that discards the language scope, so
/// there is one descent and one leaf set. Callers that need to know whether a
/// word sits inside a `<...> [@s]` span use the scoped form.
pub fn walk_words<'a>(
    content: &'a [UtteranceContent],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(WordItem<'a>),
) {
    walk_words_scoped(content, domain, &mut |item, _scope| f(item));
}

fn walk_words_in_scope<'a>(
    content: &'a [UtteranceContent],
    domain: Option<TierDomain>,
    scope: LanguageScope<'a>,
    f: &mut impl FnMut(WordItem<'a>, LanguageScope<'a>),
) {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                f(WordItem::Word(word), scope);
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    // A scoped annotation may attach to ONE content item without
                    // angle brackets, so `hallo [@s]` governs its own word just
                    // as `<a b> [@s]` governs the words it encloses.
                    f(
                        WordItem::Word(&annotated.inner),
                        scope.inside(&annotated.scoped_annotations),
                    );
                }
            }
            UtteranceContent::ReplacedWord(replaced) => {
                f(WordItem::ReplacedWord(replaced), scope);
            }
            UtteranceContent::Separator(sep) => {
                f(WordItem::Separator(sep), scope);
            }
            UtteranceContent::Group(group) => {
                walk_bracketed_words(&group.content.content, domain, scope, f);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    let inner = scope.inside(&annotated.scoped_annotations);
                    walk_bracketed_words(&annotated.inner.content.content, domain, inner, f);
                }
            }
            UtteranceContent::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words(&pho.content.content, domain, scope, f);
                }
            }
            UtteranceContent::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words(&sin.content.content, domain, scope, f);
                }
            }
            UtteranceContent::Quotation(quot) => {
                walk_bracketed_words(&quot.content.content, domain, scope, f);
            }
            UtteranceContent::Retrace(retrace) => {
                // Retrace content is excluded from %mor (not morphologically analyzed),
                // but included in %pho/%sin/%wor and for domain-unspecified walks.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words(&retrace.content.content, domain, scope, f);
                }
            }
            UtteranceContent::AnnotatedRetrace(annotated) => {
                // Same rule as the bare form. The annotations sit on the
                // wrapper, are not words, and are not walked; only the retraced
                // content is.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words(&annotated.inner.content.content, domain, scope, f);
                }
            }
            // Non-word items: events, pauses, actions, overlap markers, bullets,
            // freecodes, long features, underline markers, nonvocal markers,
            // other spoken events, none produce alignable leaf items.
            UtteranceContent::Event(_)
            | UtteranceContent::AnnotatedEvent(_)
            | UtteranceContent::Pause(_)
            | UtteranceContent::AnnotatedAction(_)
            | UtteranceContent::Freecode(_)
            | UtteranceContent::OverlapPoint(_)
            | UtteranceContent::InternalBullet(_)
            | UtteranceContent::LongFeatureBegin(_)
            | UtteranceContent::LongFeatureEnd(_)
            | UtteranceContent::UnderlineBegin(_)
            | UtteranceContent::UnderlineEnd(_)
            | UtteranceContent::NonvocalBegin(_)
            | UtteranceContent::NonvocalEnd(_)
            | UtteranceContent::NonvocalSimple(_)
            | UtteranceContent::OtherSpokenEvent(_) => {}
        }
    }
}

/// Walk utterance content mutably and call `f` for each word-like leaf item.
///
/// Same domain-aware gating as [`walk_words`].
pub fn walk_words_mut<'a>(
    content: &'a mut [UtteranceContent],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(WordItemMut<'a>),
) {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                f(WordItemMut::Word(word));
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                // Split borrow: mut inner + shared annotations (disjoint fields).
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    let a = annotated.as_mut();
                    f(WordItemMut::Word(&mut a.inner));
                }
            }
            UtteranceContent::ReplacedWord(replaced) => {
                f(WordItemMut::ReplacedWord(replaced));
            }
            UtteranceContent::Separator(sep) => {
                f(WordItemMut::Separator(sep));
            }
            UtteranceContent::Group(group) => {
                walk_bracketed_words_mut(group.content.content.as_mut_slice(), domain, f);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_words_mut(
                        annotated.inner.content.content.as_mut_slice(),
                        domain,
                        f,
                    );
                }
            }
            UtteranceContent::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words_mut(pho.content.content.as_mut_slice(), domain, f);
                }
            }
            UtteranceContent::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words_mut(sin.content.content.as_mut_slice(), domain, f);
                }
            }
            UtteranceContent::Quotation(quot) => {
                walk_bracketed_words_mut(quot.content.content.as_mut_slice(), domain, f);
            }
            UtteranceContent::Retrace(retrace) => {
                // Retrace content is excluded from %mor (not morphologically analyzed),
                // but included in %pho/%sin/%wor and for domain-unspecified walks.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words_mut(retrace.content.content.as_mut_slice(), domain, f);
                }
            }
            UtteranceContent::AnnotatedRetrace(annotated) => {
                // Same rule as the bare form. The annotations sit on the
                // wrapper, are not words, and are not walked; only the retraced
                // content is.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words_mut(
                        annotated.inner.content.content.as_mut_slice(),
                        domain,
                        f,
                    );
                }
            }
            UtteranceContent::Event(_)
            | UtteranceContent::AnnotatedEvent(_)
            | UtteranceContent::Pause(_)
            | UtteranceContent::AnnotatedAction(_)
            | UtteranceContent::Freecode(_)
            | UtteranceContent::OverlapPoint(_)
            | UtteranceContent::InternalBullet(_)
            | UtteranceContent::LongFeatureBegin(_)
            | UtteranceContent::LongFeatureEnd(_)
            | UtteranceContent::UnderlineBegin(_)
            | UtteranceContent::UnderlineEnd(_)
            | UtteranceContent::NonvocalBegin(_)
            | UtteranceContent::NonvocalEnd(_)
            | UtteranceContent::NonvocalSimple(_)
            | UtteranceContent::OtherSpokenEvent(_) => {}
        }
    }
}

#[cfg(test)]
mod tests;
