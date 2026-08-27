//! Word validation that reaches every depth of an utterance's content.
//!
//! # The defect this replaces
//!
//! Main-tier validation used to iterate content items FLATLY, matching only
//! `Word`, `AnnotatedWord`, and `ReplacedWord`, with a `_ => {}` catch-all
//! that silently swallowed every container. A word inside a retrace, a
//! reformulation, or an angle group was never word-validated at all.
//!
//! The observable consequence: the identical token was rejected outside a
//! group and accepted inside one. In English, `hello3 dog .` was invalid
//! (`E220`) while `hello3 [/] hello dog .` was valid, on every release up to
//! 2026-07-27. Every word-level rule inherited the hole, not just the recent
//! ones.
//!
//! # Why the obvious fix is wrong
//!
//! The shared `walk_words` walker recurses correctly, but it yields only
//! `Word` values, and the word-like items are not all words: a
//! `ReplacedWord::validate` carries E387, E388 and E389, and `entering()`
//! builds a language scope per item. Routing word validation through
//! `walk_words` would drop those.
//!
//! So this module recurses itself and hands each item to its OWN `Validate`
//! implementation.
//!
//! The reason recorded here until 2026-08-27 was a DIFFERENT one and is no
//! longer true: it said `Annotated<T>::validate` "deliberately does two
//! things", validating the payload AND the scoped annotations, so routing
//! through `walk_words` would drop annotation validation everywhere.
//! Annotations left that impl that day, for
//! `validation::main_tier::report_unknown_annotations`, and the impl is gone.
//! The justification had to be restated from what is still true rather than
//! left pointing at a mechanism that no longer exists.
//!
//! # Exhaustiveness is the point
//!
//! Neither match below has a `_` arm, deliberately. That is the repository
//! design rule ("exhaustive matches on `UtteranceContent`/`BracketedItem`, no
//! catch-alls that discard content, all group types recurse"), and it is the
//! specific rule whose violation caused the defect. A new content variant
//! must now fail to compile here rather than being silently skipped.

use crate::ErrorSink;
use crate::alignment::helpers::LanguageScope;
use crate::model::{BracketedItem, ContentAnnotation, UtteranceContent};
use crate::validation::{Validate, ValidationContext};
use std::borrow::Cow;

/// The context to validate `annotations`' contents under.
///
/// Borrowed unchanged when the annotations open no code-switch scope, which is
/// nearly every group; only a `[@s]` span pays for a clone. The selection rule
/// itself is [`LanguageScope::selected_by`], shared with the alignment walk, so
/// validation and metadata cannot disagree about which span governs a word.
fn entering<'a>(
    context: &'a ValidationContext,
    annotations: &[ContentAnnotation],
) -> Cow<'a, ValidationContext> {
    match LanguageScope::selected_by(annotations) {
        Some(span) => Cow::Owned(context.clone().with_code_switch_span(Some(span.clone()))),
        None => Cow::Borrowed(context),
    }
}

/// Validate every word-like item in `items`, recursing through all containers.
pub(crate) fn validate_words_at_every_depth(
    items: &[UtteranceContent],
    context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    for item in items {
        // ONE derivation per item, covering every annotation carrier, because
        // `scoped_annotations()` answers that for all of them. `context` is
        // shadowed deliberately: no arm below can reach the unscoped context
        // even by accident, because the name no longer refers to it.
        let scoped = entering(context, item.structure().scoped_annotations());
        let context = &*scoped;
        match item {
            // Word-like leaves: each validates itself. An annotated word hands
            // over its INNER word, because its annotations are not this walk's
            // business: `report_unknown_annotations` owns them for every
            // construct, not only the ones whose payload implements `Validate`.
            UtteranceContent::Word(word) => word.validate(context, errors),
            UtteranceContent::AnnotatedWord(annotated) => {
                annotated.inner.validate(context, errors);
            }
            UtteranceContent::ReplacedWord(replaced) => replaced.validate(context, errors),

            // Containers: recurse. These are precisely the variants the old
            // `_ => {}` discarded. `Quotation` belongs here and is easy to
            // miss: it carries `BracketedContent` like any other group.
            // Containers: ONE arm. Every one of these recursed unconditionally
            // into its enclosed content, which is what `ContentStructure::enclosed`
            // is for; its own docstring names the walkers under `validation/` and
            // `alignment/` as the callers that had not adopted it. The annotations
            // sit on the wrapper and are not part of the enclosed content, which is
            // what each of the separate arms already did.
            UtteranceContent::Group(_)
            | UtteranceContent::AnnotatedGroup(_)
            | UtteranceContent::Retrace(_)
            | UtteranceContent::AnnotatedRetrace(_)
            | UtteranceContent::PhoGroup(_)
            | UtteranceContent::SinGroup(_)
            | UtteranceContent::Quotation(_)
            | UtteranceContent::AnnotatedQuotation(_) => {
                if let Some(content) = item.structure().enclosed() {
                    validate_bracketed(&content.content, context, errors);
                }
            }

            // Genuine non-word leaves: events, pauses, actions, markers, and
            // span delimiters. Listed rather than swept into a catch-all so
            // that a new variant forces a decision here instead of being
            // silently skipped, which is the whole defect being fixed.
            UtteranceContent::Event(_)
            | UtteranceContent::AnnotatedEvent(_)
            | UtteranceContent::Pause(_)
            | UtteranceContent::Action(_)
            | UtteranceContent::AnnotatedAction(_)
            | UtteranceContent::Freecode(_)
            | UtteranceContent::Separator(_)
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

/// Validate every word-like item inside bracketed (grouped) content.
fn validate_bracketed(
    items: &[BracketedItem],
    context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    for item in items {
        // ONE derivation per item, covering every annotation carrier, because
        // `scoped_annotations()` answers that for all of them. `context` is
        // shadowed deliberately: no arm below can reach the unscoped context
        // even by accident, because the name no longer refers to it.
        let scoped = entering(context, item.structure().scoped_annotations());
        let context = &*scoped;
        match item {
            BracketedItem::Word(word) => word.validate(context, errors),
            BracketedItem::AnnotatedWord(annotated) => {
                annotated.inner.validate(context, errors);
            }
            BracketedItem::ReplacedWord(replaced) => replaced.validate(context, errors),

            // Nested containers: groups inside groups are ordinary in CA
            // transcription, so recursion here is not a theoretical case.
            // Containers: ONE arm. Every one of these recursed unconditionally
            // into its enclosed content, which is what `ContentStructure::enclosed`
            // is for; its own docstring names the walkers under `validation/` and
            // `alignment/` as the callers that had not adopted it. The annotations
            // sit on the wrapper and are not part of the enclosed content, which is
            // what each of the separate arms already did.
            BracketedItem::Group(_)
            | BracketedItem::AnnotatedGroup(_)
            | BracketedItem::Retrace(_)
            | BracketedItem::AnnotatedRetrace(_)
            | BracketedItem::PhoGroup(_)
            | BracketedItem::SinGroup(_)
            | BracketedItem::Quotation(_)
            | BracketedItem::AnnotatedQuotation(_) => {
                if let Some(content) = item.structure().enclosed() {
                    validate_bracketed(&content.content, context, errors);
                }
            }

            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::Separator(_)
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
