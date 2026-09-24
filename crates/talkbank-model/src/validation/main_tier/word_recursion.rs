//! Word validation at every depth through the shared structural owner.
//!
//! Each item establishes its annotation-selected language scope before its
//! payload is validated. Replaced words retain their own validation (including
//! E387/E388/E389); flattening them to individual words would lose those rules.
//! Groups and retraces supply enclosed content infallibly. Top-level and nested
//! items use the same exhaustive structural dispatch, so container membership
//! cannot drift between two independent variant lists.

use crate::ErrorSink;
use crate::alignment::helpers::LanguageScope;
use crate::model::{ContentAnnotation, ContentStructure, UtteranceContent, WordRef};
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
        validate_structure(item.structure(), context, errors);
    }
}

/// Enter this item's language scope before validating its payload or children.
fn validate_structure(
    structure: ContentStructure<'_>,
    context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    let scoped = entering(context, structure.scoped_annotations());
    let context = &*scoped;
    let content = match structure {
        ContentStructure::Word(word) => {
            match word {
                WordRef::Bare(word) => word.validate(context, errors),
                WordRef::Annotated(annotated) => annotated.inner.validate(context, errors),
                WordRef::Replaced(replaced) => replaced.validate(context, errors),
            }
            return;
        }
        ContentStructure::Group(group) => group.content(),
        ContentStructure::Retrace(retrace) => &retrace.inner().content,
        ContentStructure::Leaf(_) => return,
    };
    for item in &content.content {
        validate_structure(item.structure(), context, errors);
    }
}
