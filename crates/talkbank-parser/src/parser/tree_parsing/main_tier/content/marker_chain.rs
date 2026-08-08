//! Folding a content item and the scoped markers written after it into the model.
//!
//! # The chain
//!
//! In CHAT, a content item followed by a run of scoped markers is a
//! LEFT-ASSOCIATIVE CHAIN: each marker scopes over everything to its left.
//!
//! ```text
//! dog [* p:w] [/]   =  retrace( error( dog ) )
//! dog [/] [* p:w]   =  error( retrace( dog ) )
//! ```
//!
//! Those are different claims about the same two words, so the order is
//! information. Folding left, one wrapper per marker, IS the meaning of the
//! surface rather than an encoding of it, which is why nothing here needs to
//! ask which side of a retrace marker an annotation fell on.
//!
//! # What this replaces, and why the fold is the honest shape
//!
//! The lowering used to receive `{ content: Vec<ContentAnnotation>, retrace:
//! Option<RetraceKind> }`, a PARTITION of the run. A partition of an ordered
//! sequence can represent neither the interleaving nor a second marker, so the
//! parser silently rewrote `dog [* p:w] [/]` as `dog [/] [* p:w]` (12,226
//! attested places in the corpora) and let a second marker overwrite the first
//! (105 places). Both were invisible to `validate`, to `--roundtrip` and to
//! `SemanticEq`.
//!
//! A later attempt split the run around the marker and REFUSED a second one at
//! parse time. That fixed the ordering but put a judgement in the lowering:
//! the rule then existed only for the tree-sitter backend, it discarded the
//! rest of the run during recovery, and the same shape written with brackets
//! (`<<a> [/]> [//]`) needed a second implementation elsewhere.
//!
//! Folding instead lowers every run faithfully, including the illegal ones,
//! and leaves the judgement to validation, where one rule covers both
//! spellings and both parser backends. `a [//] [/]` becomes a retrace whose
//! content is a lone retrace, which serializes straight back to `a [//] [/]`.
//!
//! # Serialization round-trips by construction
//!
//! `Retrace::write_chat` emits its content then its marker, and
//! `Annotated<T>::write_chat` emits its inner value then its annotations. Those
//! are exactly the two positions the fold builds, so a folded chain writes back
//! the bytes it was folded from without anyone maintaining a rule about it.

use talkbank_model::Span;
use talkbank_model::model::RetraceKind;

use crate::model::{Annotated, BracketedContent, ContentAnnotation, Retrace, UtteranceContent};

use super::super::annotations::ParsedAnnotation;
use super::group::convert_to_group_content;

/// Fold a content item and its ordered marker run into one `UtteranceContent`.
///
/// `span` is the whole construct, the item through the final `]`. E757's glue
/// detection relies on a wrapper's span ending at the last bracket, so every
/// wrapper the fold builds gets it.
pub(crate) fn fold_marker_chain(
    core: UtteranceContent,
    markers: Vec<ParsedAnnotation>,
    span: Span,
) -> UtteranceContent {
    markers
        .into_iter()
        .fold(core, |current, marker| match marker {
            ParsedAnnotation::Content(annotation) => annotate(current, annotation, span),
            ParsedAnnotation::Retrace(kind) => retrace(current, kind, span),
        })
}

/// Attach one annotation to whatever the chain has built so far.
///
/// An already-annotated wrapper gains the annotation rather than being wrapped
/// again: `dog [?] [!]` is one word with two annotations, which is how every
/// other scoped symbol in the model already behaves, and nesting two wrappers
/// there would serialize identically while making every consumer walk twice.
fn annotate(
    current: UtteranceContent,
    annotation: ContentAnnotation,
    span: Span,
) -> UtteranceContent {
    match current {
        // Already a wrapper: extend it.
        UtteranceContent::AnnotatedWord(annotated) => UtteranceContent::AnnotatedWord(Box::new(
            (*annotated).with_scoped_annotation(annotation),
        )),
        UtteranceContent::AnnotatedGroup(annotated) => {
            UtteranceContent::AnnotatedGroup(annotated.with_scoped_annotation(annotation))
        }
        UtteranceContent::AnnotatedEvent(annotated) => {
            UtteranceContent::AnnotatedEvent(annotated.with_scoped_annotation(annotation))
        }
        UtteranceContent::AnnotatedAction(annotated) => {
            UtteranceContent::AnnotatedAction(annotated.with_scoped_annotation(annotation))
        }
        UtteranceContent::AnnotatedRetrace(annotated) => UtteranceContent::AnnotatedRetrace(
            Box::new((*annotated).with_scoped_annotation(annotation)),
        ),
        // A replacement keeps its own annotation list, which is the pre-existing
        // behaviour of `word [: text] [* code]` and is not this change's to move.
        UtteranceContent::ReplacedWord(replaced) => {
            // `ReplacedWord` has only a plural setter, so the existing list is
            // read out and rewritten. Cheap: a replaced word carries 0-2
            // annotations, and this runs once per annotation on that word.
            let mut scoped: Vec<ContentAnnotation> =
                replaced.scoped_annotations.iter().cloned().collect();
            scoped.push(annotation);
            UtteranceContent::ReplacedWord(Box::new(replaced.with_scoped_annotations(scoped)))
        }
        // Not yet a wrapper: become one.
        UtteranceContent::Word(word) => UtteranceContent::AnnotatedWord(Box::new(
            Annotated::new(*word)
                .with_scoped_annotation(annotation)
                .with_span(span),
        )),
        UtteranceContent::Group(group) => UtteranceContent::AnnotatedGroup(
            Annotated::new(group)
                .with_scoped_annotation(annotation)
                .with_span(span),
        ),
        UtteranceContent::Event(event) => UtteranceContent::AnnotatedEvent(
            Annotated::new(event)
                .with_scoped_annotation(annotation)
                .with_span(span),
        ),
        UtteranceContent::Retrace(retraced) => UtteranceContent::AnnotatedRetrace(Box::new(
            Annotated::new(*retraced)
                .with_scoped_annotation(annotation)
                .with_span(span),
        )),
        // Unreachable from the three seed call sites, which produce only
        // `Word`, `ReplacedWord`, `Group`, `Event` and `AnnotatedAction`, and
        // the fold itself only ever adds `Retrace` and the annotated forms
        // above. Spelled out anyway rather than caught by `_`, because a `_`
        // here is the shape that let `AnnotatedRetrace` slip past five other
        // matches when the variant was added, and because adding a content
        // variant should break this build rather than silently reach an arm
        // that drops an annotation.
        item @ (UtteranceContent::Pause(_)
        | UtteranceContent::PhoGroup(_)
        | UtteranceContent::SinGroup(_)
        | UtteranceContent::Quotation(_)
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
        | UtteranceContent::OtherSpokenEvent(_)) => item,
    }
}

/// Wrap whatever the chain has built so far in a retrace.
fn retrace(current: UtteranceContent, kind: RetraceKind, span: Span) -> UtteranceContent {
    // A bare group hands its brackets to the retrace, which is exactly what
    // `Retrace::is_group` records. Nesting it instead would serialize
    // `<<a b>> [/]`, adding a bracket pair the transcriber did not write.
    //
    // SHARED SEMANTICS, written twice. `talkbank-parser-re2c`'s
    // `parser::classify::Chain::retraced` decides the same thing on that
    // crate's own AST type, so no constructor can own it for both. Drift here
    // is silent and corrupts output, so the enforcement is the cross-parser
    // test `equivalence_marker_chain`; change one of these two and run it.
    let built = match convert_to_group_content(current) {
        Ok(item) => Retrace::new(BracketedContent::new(vec![item]), kind),
        Err(group) => Retrace::new(group.content, kind).as_group(),
    };
    UtteranceContent::Retrace(Box::new(built.with_span(span)))
}
