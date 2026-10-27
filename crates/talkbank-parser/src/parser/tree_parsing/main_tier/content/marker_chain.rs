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

use crate::model::{
    Action, Annotated, BracketedContent, ContentAnnotation, Event, Group, Quotation, ReplacedWord,
    Retrace, UtteranceContent, Word,
};

use super::super::annotations::ParsedAnnotation;
use super::group::convert_to_group_content;

/// Only these source constructs can start a scoped-marker chain.
pub(super) enum MarkerSeed {
    Word(Box<Word>),
    ReplacedWord(Box<ReplacedWord>),
    Group(Group),
    Event(Event),
    Action(Action),
    Quotation(Quotation),
}

/// The closed set of states reachable by folding markers onto a seed.
enum Chain {
    Core(MarkerSeed),
    AnnotatedWord(Box<Annotated<Word>>),
    AnnotatedGroup(Annotated<Group>),
    AnnotatedEvent(Annotated<Event>),
    AnnotatedAction(Annotated<Action>),
    AnnotatedQuotation(Annotated<Quotation>),
    Retrace(Box<Retrace>),
    AnnotatedRetrace(Box<Annotated<Retrace>>),
}

impl MarkerSeed {
    fn into_content(self) -> UtteranceContent {
        match self {
            Self::Word(word) => UtteranceContent::Word(word),
            Self::ReplacedWord(word) => UtteranceContent::ReplacedWord(word),
            Self::Group(group) => UtteranceContent::Group(group),
            Self::Event(event) => UtteranceContent::Event(event),
            Self::Action(action) => UtteranceContent::Action(action),
            Self::Quotation(quotation) => UtteranceContent::Quotation(quotation),
        }
    }
}

impl Chain {
    fn into_content(self) -> UtteranceContent {
        match self {
            Self::Core(seed) => seed.into_content(),
            Self::AnnotatedWord(word) => UtteranceContent::AnnotatedWord(word),
            Self::AnnotatedGroup(group) => UtteranceContent::AnnotatedGroup(group),
            Self::AnnotatedEvent(event) => UtteranceContent::AnnotatedEvent(event),
            Self::AnnotatedAction(action) => UtteranceContent::AnnotatedAction(action),
            Self::AnnotatedQuotation(quotation) => UtteranceContent::AnnotatedQuotation(quotation),
            Self::Retrace(retrace) => UtteranceContent::Retrace(retrace),
            Self::AnnotatedRetrace(retrace) => UtteranceContent::AnnotatedRetrace(retrace),
        }
    }
}

/// Fold a content item and its ordered marker run into one `UtteranceContent`.
///
/// `span` is the whole construct, the item through the final `]`; only its
/// start is read. Every value the fold builds is placed exactly at itself,
/// the convention both backends share: a wrapper runs from the construct
/// through the LAST annotation attached to it (so the outermost wrapper ends
/// at the chain's final `]`, which E757's glue detection relies on, and an
/// inner wrapper ends at its own last annotation), and a retrace runs from
/// its content through its marker.
pub(super) fn fold_marker_chain(
    core: MarkerSeed,
    markers: Vec<ParsedAnnotation>,
    span: Span,
) -> UtteranceContent {
    let start = span.start();
    markers
        .into_iter()
        .fold(Chain::Core(core), |current, marker| match marker {
            ParsedAnnotation::Content {
                annotation,
                span: annotation_span,
            } => annotate(current, annotation, Span::new(start, annotation_span.end())),
            ParsedAnnotation::Retrace {
                kind,
                span: marker_span,
            } => retrace(
                current,
                kind,
                Span::new(start, marker_span.end()),
                marker_span,
            ),
        })
        .into_content()
}

/// A wrapper that gains an annotation grows to end where that annotation
/// ends; its start never moves.
fn extend<T>(
    mut annotated: Annotated<T>,
    annotation: ContentAnnotation,
    span: Span,
) -> Annotated<T> {
    annotated = annotated.with_scoped_annotation(annotation);
    annotated.location = talkbank_model::Location::Located(span);
    annotated
}

/// Attach one annotation to whatever the chain has built so far; `span` is
/// the construct through this annotation, which the wrapper takes as its own.
///
/// An already-annotated wrapper gains the annotation rather than being wrapped
/// again: `dog [?] [!]` is one word with two annotations, which is how every
/// other scoped symbol in the model already behaves, and nesting two wrappers
/// there would serialize identically while making every consumer walk twice.
fn annotate(current: Chain, annotation: ContentAnnotation, span: Span) -> Chain {
    match current {
        // Already a wrapper: extend it, in annotations and in extent.
        Chain::AnnotatedWord(annotated) => {
            Chain::AnnotatedWord(Box::new(extend(*annotated, annotation, span)))
        }
        Chain::AnnotatedGroup(annotated) => {
            Chain::AnnotatedGroup(extend(annotated, annotation, span))
        }
        Chain::AnnotatedQuotation(annotated) => {
            Chain::AnnotatedQuotation(extend(annotated, annotation, span))
        }
        Chain::AnnotatedEvent(annotated) => {
            Chain::AnnotatedEvent(extend(annotated, annotation, span))
        }
        Chain::AnnotatedAction(annotated) => {
            Chain::AnnotatedAction(extend(annotated, annotation, span))
        }
        Chain::AnnotatedRetrace(annotated) => {
            Chain::AnnotatedRetrace(Box::new(extend(*annotated, annotation, span)))
        }
        // A replacement keeps its own annotation list, which is the pre-existing
        // behaviour of `word [: text] [* code]` and is not this change's to move.
        Chain::Core(MarkerSeed::ReplacedWord(replaced)) => {
            // `ReplacedWord` has only a plural setter, so the existing list is
            // read out and rewritten. Cheap: a replaced word carries 0-2
            // annotations, and this runs once per annotation on that word.
            let mut scoped: Vec<ContentAnnotation> =
                replaced.scoped_annotations.iter().cloned().collect();
            scoped.push(annotation);
            let mut replaced = replaced.with_scoped_annotations(scoped);
            replaced.location = talkbank_model::Location::Located(span);
            Chain::Core(MarkerSeed::ReplacedWord(Box::new(replaced)))
        }
        // Not yet a wrapper: become one.
        Chain::Core(MarkerSeed::Word(word)) => {
            Chain::AnnotatedWord(Box::new(Annotated::with_one(span, *word, annotation)))
        }
        Chain::Core(MarkerSeed::Group(group)) => {
            Chain::AnnotatedGroup(Annotated::with_one(span, group, annotation))
        }
        Chain::Core(MarkerSeed::Quotation(quotation)) => {
            Chain::AnnotatedQuotation(Annotated::with_one(span, quotation, annotation))
        }
        Chain::Core(MarkerSeed::Event(event)) => {
            Chain::AnnotatedEvent(Annotated::with_one(span, event, annotation))
        }
        // Added 2026-08-26 with the bare `Action` variant: an action that
        // meets its first scoped marker is promoted exactly as an event is.
        // Before that variant existed the parser had pre-wrapped every action,
        // so there was nothing here to promote.
        Chain::Core(MarkerSeed::Action(action)) => {
            Chain::AnnotatedAction(Annotated::with_one(span, action, annotation))
        }
        Chain::Retrace(retraced) => {
            Chain::AnnotatedRetrace(Box::new(Annotated::with_one(span, *retraced, annotation)))
        }
    }
}

/// Wrap whatever the chain has built so far in a retrace.
///
/// `span` is the content through the marker, which is the retrace's own
/// extent; `marker_span` is the marker token's own bytes, which E370 reports
/// at.
fn retrace(current: Chain, kind: RetraceKind, span: Span, marker_span: Span) -> Chain {
    // A bare group hands its brackets to the retrace, which is exactly what
    // `Retrace::is_group` records. Nesting it instead would serialize
    // `<<a b>> [/]`, adding a bracket pair the transcriber did not write.
    //
    // SHARED SEMANTICS, written twice. `talkbank-parser-re2c`'s
    // `parser::classify::Chain::retraced` decides the same thing on that
    // crate's own AST type, so no constructor can own it for both. Drift here
    // is silent and corrupts output, so the enforcement is the cross-parser
    // test `equivalence_marker_chain`; change one of these two and run it.
    let built = if let Chain::Core(MarkerSeed::Group(group)) = current {
        // A BARE group hands its brackets to the retrace, which is what
        // `Retrace::is_group` records. Tested here rather than reported by the
        // converter, because this is the only caller for which it matters.
        Retrace::new(span, marker_span, group.content, kind).as_group()
    } else {
        Retrace::new(
            span,
            marker_span,
            BracketedContent::new(vec![convert_to_group_content(current.into_content())]),
            kind,
        )
    };
    Chain::Retrace(Box::new(built))
}
