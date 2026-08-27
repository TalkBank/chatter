//! Generic wrapper for adding scoped annotations to content items.
//!
//! The `Annotated<T>` wrapper adds scoped annotations (bracketed markers) to any
//! content type that supports them. Scoped annotations provide linguistic, error,
//! and explanatory information about the preceding element.
//!
//! # Scoped Annotation Types
//!
//! - **Error codes** (`[* code]`) - Mark speech errors like `[* m]`, `[* s]`
//! - **Explanations** (`[= text]`) - Provide clarification or translation
//! - **Additions** (`[+ text]`) - Add transcriber comments
//! - **Retracing** (`[/]`, `[//]`, `[///]`) - Mark repetitions and corrections
//! - **Paralinguistic** (`[! text]`) - Note tone, emphasis, gestures
//! - **Replacements** (`[: text]`) - Show what was actually said
//!
//! # CHAT Format Examples
//!
//! ```text
//! I want [* m] cookie                      Word with error code
//! &=laughs [! loudly]                      Event with paralinguistic note
//! <I want> [/] I need cookie              Group with retracing
//! hola [= hello]                           Word with explanation
//! dog [: cat]                              Word with replacement
//! ```
//!
//! # References
//!
//! - [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
//! - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)

use super::{ContentAnnotation, WriteChat};
use crate::model::{
    SemanticDiff, SemanticDiffContext, SemanticDiffReport, SemanticEq, SemanticPath, normalize_span,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use talkbank_derive::{SemanticEq, SpanShift};

/// Scoped annotations attached to an `Annotated<T>` wrapper, NEVER empty.
///
/// # The invariant is structural now, and it used to be a rule nothing enforced
///
/// "Must contain at least one annotation" was prose here, backed by a runtime
/// code (E214) reported during validation. Measured 2026-08-26 across a
/// 106,000-file corpus, that arrangement was failing in both directions at
/// once. E214 could not fire on any input: an empty bracket is a parse error,
/// its own spec example produced no diagnostics, and the only payload that was
/// routinely empty is an `Action`, whose wrapper is never validated because
/// `Action` does not implement `Validate`. Meanwhile 20,184,072 `Annotated`
/// values in the corpus DID carry an empty list, every one of them a bare `0`
/// the parser had nowhere else to put, because `UtteranceContent` had no bare
/// `Action` variant while it had a bare `Event`.
///
/// How that number was taken, because a measurement without its method is not
/// one: over a `chatter to-json` mirror of roughly 106,000 files, counting
/// occurrences of `"type": "annotated_action"` and subtracting those followed
/// by a comma, a comma meaning the object carries further fields and so a real
/// annotation. 20,318,021 total, 133,949 annotated, 20,184,072 empty. The
/// pattern needs the space after the colon: the mirror is pretty-printed, so
/// the unspaced form matches nothing on any input and reads as a clean zero.
///
/// So the rule was unenforceable where it was true and unenforced where it was
/// violated. The bare variant exists now and this type refuses the empty case,
/// which retired E214 entirely.
///
/// # Every route in, enumerated
///
/// A proof type is only as strong as its weakest constructor, so: [`Self::new`]
/// is the only public constructor and returns `None` for an empty list;
/// `TryFrom<Vec<_>>` is the same check under the conversion trait, replacing an
/// infallible `From` that skipped it; `Deserialize` rejects an empty list
/// rather than accepting one off the wire; there is deliberately no `Default`,
/// which would have manufactured the forbidden value; and the type no longer
/// takes `collection_newtype_ops!`, whose `take` and `retain` can empty a
/// collection in place. None of those operations had a single caller on this
/// type. Read access is `Deref`, which cannot resize, and the `&mut` iterator
/// yields elements without allowing the length to change.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Error_Coding>
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct AnnotatedContentAnnotations(#[schemars(length(min = 1))] Vec<ContentAnnotation>);

impl AnnotatedContentAnnotations {
    /// Wraps scoped annotations, or `None` when there are none.
    ///
    /// `None` is not a failure and is not reported as one: it means the caller
    /// is not looking at an annotated construct at all, and should build the
    /// BARE variant instead. That branch is the whole point of the type, which
    /// is why this returns an `Option` rather than a `Result` with a domain
    /// error nobody would ever surface.
    #[must_use]
    pub fn new(annotations: Vec<ContentAnnotation>) -> Option<Self> {
        if annotations.is_empty() {
            None
        } else {
            Some(Self(annotations))
        }
    }
}

impl Deref for AnnotatedContentAnnotations {
    type Target = Vec<ContentAnnotation>;

    /// Borrows the underlying annotation vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The empty list, which is not an annotated construct.
///
/// Its own type rather than a bare `()` so a caller reading a `Result` sees
/// what went wrong without consulting the docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an annotated construct must carry at least one scoped annotation")]
pub struct NoScopedAnnotations;

impl TryFrom<Vec<ContentAnnotation>> for AnnotatedContentAnnotations {
    type Error = NoScopedAnnotations;

    /// Wraps a raw scoped-annotation vector, refusing the empty one.
    ///
    /// `TryFrom` rather than `From`: an infallible conversion that skips an
    /// invariant is how the empty state got in, and the house rule is that a
    /// conversion which can reject its input must say so in its type.
    fn try_from(annotations: Vec<ContentAnnotation>) -> Result<Self, Self::Error> {
        Self::new(annotations).ok_or(NoScopedAnnotations)
    }
}

impl<'de> Deserialize<'de> for AnnotatedContentAnnotations {
    /// Rejects an empty list off the wire.
    ///
    /// The wire is a route into the type exactly as a constructor is, and a
    /// transparent derive would have accepted `[]` and rebuilt the state the
    /// rest of this file exists to forbid.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let annotations = Vec::<ContentAnnotation>::deserialize(deserializer)?;
        Self::new(annotations).ok_or_else(|| serde::de::Error::custom(NoScopedAnnotations))
    }
}

impl<'a> IntoIterator for &'a AnnotatedContentAnnotations {
    type Item = &'a ContentAnnotation;
    type IntoIter = std::slice::Iter<'a, ContentAnnotation>;

    /// Iterates immutably over scoped annotations.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut AnnotatedContentAnnotations {
    type Item = &'a mut ContentAnnotation;
    type IntoIter = std::slice::IterMut<'a, ContentAnnotation>;

    /// Iterates mutably over scoped annotations.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for AnnotatedContentAnnotations {
    type Item = ContentAnnotation;
    type IntoIter = std::vec::IntoIter<ContentAnnotation>;

    /// Consumes the wrapper and yields owned scoped annotations.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl AnnotatedContentAnnotations {
    /// Report unknown scoped markers, located at `span`.
    ///
    /// Takes the span rather than reading it out of a `ValidationContext`,
    /// which is why this is no longer a `Validate` impl. That indirection cost
    /// an `unwrap_or(Span::DUMMY)`: a sentinel equal to `Span::default()` and
    /// to a real zero-length position at offset 0, standing in for a location
    /// the only caller always had. It also enforced non-empty, reporting E214,
    /// until the type made that state unconstructible.
    /// Takes a SLICE, not `&self`: the traversal that owns this question hands
    /// out `&[ContentAnnotation]` through
    /// `ContentStructure::scoped_annotations`, because a LEAF carries its
    /// annotations as a plain field and has no newtype to offer.
    pub(crate) fn report_unknown_markers(
        annotations: &[ContentAnnotation],
        span: crate::Span,
        errors: &impl crate::ErrorSink,
    ) {
        for annotation in annotations {
            if let ContentAnnotation::Unknown(unknown) = annotation {
                let marker = &unknown.marker;
                errors.report(
                    crate::ParseError::new(
                        crate::ErrorCode::UnknownAnnotation,
                        crate::Severity::Error,
                        crate::SourceLocation::new(span),
                        crate::ErrorContext::new(marker.as_str(), 0..marker.len(), "annotation"),
                        unknown.unreadable_message(),
                    )
                    .with_suggestion("Check CHAT manual for valid annotation types"),
                );
            }
        }
    }
}

/// Generic wrapper that adds scoped annotations to a content item.
///
/// This wrapper is used throughout CHAT to attach bracketed annotations to
/// words, events, groups, and actions. The annotations appear immediately
/// after the annotated element in CHAT format.
///
/// # CHAT Format Examples
///
/// ```text
/// want [* m]                               Word error (missing word)
/// going [* s]                              Word error (started utterance)
/// dog [= explanation]                      Explanation annotation
/// &=laughs [! loudly]                      Paralinguistic note
/// <I want> [/] I need                      Repetition retracing
/// <the dog> [//] the cat                   Correction retracing
/// perro [= dog]                            Translation
/// ```
///
/// # Type Parameter
///
/// - `T` - The inner content type being annotated (Word, Event, Group, or Action)
///
/// # Common Uses
///
/// - `Annotated<Word>` - Word with error codes, explanations, or comments
/// - `Annotated<Event>` - Event with paralinguistic notes
/// - `Annotated<Group>` - Group with retracing markers
/// - `Annotated<Action>` - Action with explanatory notes
///
/// # References
///
/// - [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
/// - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift)]
pub struct Annotated<T> {
    /// The payload that receives scoped annotations.
    #[serde(flatten)]
    pub inner: T,

    /// Scoped annotations emitted immediately after [`Self::inner`].
    ///
    /// Examples: `[*]`, `[= text]`, `[+ text]`, `[//]`.
    /// Always at least one; see [`AnnotatedContentAnnotations`]. It carried
    /// `skip_serializing_if`/`default` until 2026-08-26, a pair whose only
    /// effect was to hide the empty case on the wire and rebuild it on read.
    pub scoped_annotations: AnnotatedContentAnnotations,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip, default = "crate::Span::dummy")]
    #[schemars(skip)]
    pub span: crate::Span,
}

impl<T: SemanticEq> SemanticEq for Annotated<T> {
    /// Semantic equality ignores wrapper span and compares payload + annotations.
    fn semantic_eq(&self, other: &Self) -> bool {
        self.inner.semantic_eq(&other.inner)
            && self
                .scoped_annotations
                .semantic_eq(&other.scoped_annotations)
    }
}

impl<T: SemanticDiff> SemanticDiff for Annotated<T> {
    /// Computes nested semantic diff while preserving wrapper span in context.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        let prev_span = ctx.push_span(normalize_span(self.span));

        path.push_field("inner");
        self.inner
            .semantic_diff_into(&other.inner, path, report, ctx);
        path.pop();

        if !report.is_truncated() {
            path.push_field("scoped_annotations");
            self.scoped_annotations.semantic_diff_into(
                &other.scoped_annotations,
                path,
                report,
                ctx,
            );
            path.pop();
        }

        ctx.pop_span(prev_span);
    }
}

impl<T> Annotated<T> {
    /// Creates an annotated wrapper around `inner`.
    ///
    /// Takes the annotations rather than starting empty and filling in
    /// afterwards. The builder shape it replaces (`new(inner)` then
    /// `with_scoped_annotation`) meant every wrapper passed through the empty
    /// state, and one caller simply never left it: the parser wrapped every
    /// bare `0` and produced 20,184,072 empty wrappers across the corpus; see
    /// [`AnnotatedContentAnnotations`] for how that was counted.
    ///
    /// # Everything this needs is reachable from `talkbank_model::model`
    ///
    /// The example below is compiled, and that is its job. When the non-empty
    /// invariant landed it added three public items and re-exported none of
    /// them, so a caller who could name `Annotated` could name neither the
    /// proof this constructor demands nor the error its `TryFrom` returns. A
    /// prose note would not have noticed; a doctest that imports only from the
    /// curated path fails to compile the moment one of them leaves it again.
    ///
    /// ```
    /// use talkbank_model::model::{
    ///     Action, Annotated, AnnotatedContentAnnotations, ContentAnnotation,
    ///     NoScopedAnnotations, ScopedExplanation,
    /// };
    ///
    /// let annotations = AnnotatedContentAnnotations::try_from(vec![
    ///     ContentAnnotation::Explanation(ScopedExplanation { text: "whining".into() }),
    /// ])?;
    /// let annotated = Annotated::new(Action::new(), annotations);
    /// assert_eq!(annotated.scoped_annotations.len(), 1);
    ///
    /// // The empty list is refused, and the refusal has a name a caller can match on.
    /// assert_eq!(
    ///     AnnotatedContentAnnotations::try_from(Vec::new()).err(),
    ///     Some(NoScopedAnnotations),
    /// );
    /// # Ok::<(), NoScopedAnnotations>(())
    /// ```
    pub fn new(inner: T, scoped_annotations: AnnotatedContentAnnotations) -> Self {
        Self {
            inner,
            scoped_annotations,
            span: crate::Span::DUMMY,
        }
    }

    /// Creates an annotated wrapper carrying exactly one annotation.
    ///
    /// The promotion path: a bare construct meets its first scoped marker and
    /// becomes an annotated one. Infallible by construction, which is why it
    /// exists beside [`Self::new`].
    pub fn with_one(inner: T, annotation: ContentAnnotation) -> Self {
        Self::new(inner, AnnotatedContentAnnotations(vec![annotation]))
    }

    /// Sets source span metadata used in diagnostics.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }

    /// Appends one scoped annotation to the existing list.
    pub fn with_scoped_annotation(mut self, annotation: ContentAnnotation) -> Self {
        self.scoped_annotations.0.push(annotation);
        self
    }
}

impl<T: WriteChat> WriteChat for Annotated<T> {
    /// Serializes `inner` followed by each scoped annotation separated by spaces.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.inner.write_chat(w)?;
        for ann in &self.scoped_annotations {
            w.write_char(' ')?;
            ann.write_chat(w)?;
        }
        Ok(())
    }
}

// `impl<T: Validate> Validate for Annotated<T>` stood here and is DELETED.
//
// It did two things: validate the inner payload, and report unknown scoped
// annotations. The second half moved to
// `validation::main_tier::report_unknown_annotations` on 2026-08-27, because
// reaching a construct's annotations only when its PAYLOAD implements
// `Validate` is a trait bound standing in for a policy, and it left the re2c
// backend silent on four hosts.
//
// That left this impl as pure delegation, and leaving it standing was the
// hazard rather than the cost: an affordance beats a rule, so the next reader
// would see `Annotated<T>: Validate` and reasonably put the annotation check
// back into it, re-creating the coupling. Its two call sites in
// `word_recursion` say `annotated.inner.validate(..)` now, which is what the
// impl did, said where it happens.

// The test module that stood here is gone with the code it guarded. Its two
// tests asserted that an empty scoped-annotation list reports E214, and that a
// non-empty one does not. The empty list cannot be built any more, so the first
// test could not be written and the second has only one case left. A type that
// makes a state unconstructible retires the tests that guarded it rather than
// keeping them as runtime proof of what the compiler now refuses.
