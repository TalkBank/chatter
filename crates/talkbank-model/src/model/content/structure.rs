//! The one place that knows which content variants contain other content.
//!
//! # Why this exists
//!
//! Every walker over CHAT content needs the same three facts about an item: is
//! it a word, is it a retrace, and does it enclose further content. Until this
//! module, each walker answered them with its own hand-written match, and the
//! copies drifted. `validation::retrace::visit` recursed into `PhoGroup` and
//! `SinGroup`; its sibling `marker_on_marker` listed both as leaves. E377
//! therefore stopped firing inside `‹...›`, and nothing could have caught it:
//! the two walkers shared no definition of "container", so there was no
//! artifact for the compiler or a test to find the disagreement in.
//!
//! One owner makes that disagreement unrepresentable FOR ITS CONSUMERS. Be
//! precise about who those are, because the first draft of this comment was
//! not: today they are the retrace validators (`validation::retrace::visit`
//! and `without_words`) and nobody else. `alignment::helpers::walk` still
//! carries eight independent container matches of its own, and roughly a dozen
//! other walkers under `validation/` and `alignment/` carry one each. Adding a
//! container variant is a compile error HERE and a silent omission THERE.
//!
//! Migrating them is not mechanical, and the reason was worth recording: `Word`
//! and `Container` were payload-free, while the walkers that would adopt them
//! need `&Word` (to validate its annotations) or need to know WHICH container
//! they are in (so a tier domain can skip `PhoGroup` but not `Quotation`).
//!
//! **Those payloads are settled** (`WordRef`, `GroupRef`, `RetraceRef`,
//! `LeafRef`), so the type is PUBLIC as of v0.11.0. It was `pub(crate)` while
//! they were unsettled, and that privacy had a cost this crate does not pay
//! but its consumers do: every downstream walker re-derived the container set
//! by hand, and two of them were wrong within a day of v0.10.0 adding
//! `AnnotatedRetrace`. One dropped a retrace out of an utterance-segmentation
//! pass, reviving a stranding bug on a real corpus; the other stopped
//! descending into annotated retraces entirely. Both compiled clean, because a
//! hand-written match with a catch-all cannot notice a new variant. Public,
//! this type turns that class into a compile error at every consumer.
//!
//! # Why one classification instead of three predicates
//!
//! Three accessors (`is_word`, `as_retrace`, `container_content`) would each
//! need their own exhaustive match to be safe, and the natural way to write
//! them, `matches!(self, Self::Word(_) | ...)`, is NOT exhaustive: a future
//! `EmphasizedWord` variant would silently answer "not a word". A single
//! method returning a closed sum type needs one exhaustive match per enum and
//! forces a decision about every new variant at the point it is added.
//!
//! It also lets the type say something the old walkers only did by accident: a
//! retrace IS a container over the material it retraces, so [`Retrace`] carries
//! the node and the caller reaches its content through it.

// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `audit_content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]
use crate::model::annotation::ContentAnnotation;
use crate::model::{
    Annotated, BracketedContent, BracketedItem, Group, PhoGroup, Quotation, ReplacedWord, Retrace,
    SinGroup, UtteranceContent, Word,
};

/// A model node that CARRIES scoped annotations.
///
/// # Why a trait rather than an arm per variant
///
/// The first version of the annotation accessor decided each variant by hand,
/// and got `ReplacedWord` wrong: it answered "no annotations" for
/// `dog [: cat] [* p:w]`, whose annotations two rendering paths and the
/// alignment units already read. Nothing objected, because a hand-written arm
/// list is a mapping from variant to answer with no tie to whether that node
/// actually has the field.
///
/// One implementation per node TYPE moves the answer to where the field is.
/// A node that carries annotations implements this and cannot be given `&[]`
/// by a distracted caller; a node that does not carry them has no impl to
/// write, so `&[]` at the call site is checkable by looking at one struct.
///
/// Implemented for both annotation carriers, which wrap the same
/// `Vec<ContentAnnotation>` in two different newtypes
/// (`AnnotatedContentAnnotations`, `ReplacedWordAnnotations`); that duplication
/// is why the two were never connected in the first place.
pub trait ScopedAnnotated {
    /// The annotations scoped to this node, empty when it carries none.
    fn scoped_annotations(&self) -> &[ContentAnnotation];
}

impl<T> ScopedAnnotated for Annotated<T> {
    fn scoped_annotations(&self) -> &[ContentAnnotation] {
        &self.scoped_annotations
    }
}

impl ScopedAnnotated for ReplacedWord {
    fn scoped_annotations(&self) -> &[ContentAnnotation] {
        &self.scoped_annotations
    }
}

/// What a content item is, structurally, to something walking the tree.
///
/// Deliberately coarse. This answers "how do I traverse and classify this
/// item", not "what is it"; callers that need the payload of a specific
/// variant still match on the enum itself.
#[derive(Debug, Clone, Copy)]
pub enum ContentStructure<'a> {
    /// A word, in any of its forms. Untranscribed material (`xxx`, `yyy`,
    /// `www`) lowers as a word and is deliberately included; whether that
    /// counts is the caller's question, not this classification's.
    Word(WordRef<'a>),
    /// A retrace, which is also a container: the material it retraces is its
    /// own `content`.
    Retrace(RetraceRef<'a>),
    /// A container that is not a retrace, keeping its kind and payload.
    Group(GroupRef<'a>),
    /// A leaf enclosing no further content: events, pauses, actions, CA
    /// markers, bullets, freecodes, and the long-feature and nonvocal
    /// delimiters.
    Leaf(LeafRef<'a>),
}

/// The two spellings of a retrace, each keeping its payload.
///
/// `Retrace(&Retrace)` reached the annotated spelling through
/// `&annotated.inner`, DISCARDING the annotations that follow the marker. That
/// is this module's own Shape C: a total function silently dropping
/// information, in the type whose job is to be the one owner. It is also why
/// `iisrp_session_profile` had to hand-roll a `carries_annotations` walker in
/// August 2026: the owner could not answer the question.
#[derive(Debug, Clone, Copy)]
pub enum RetraceRef<'a> {
    /// `<a b> [/]`
    Bare(&'a Retrace),
    /// `<a b> [/] [* p:w]`
    Annotated(&'a Annotated<Retrace>),
}

impl<'a> RetraceRef<'a> {
    /// The annotations scoped to this retrace.
    ///
    /// Every ref type answers this, so a consumer holding one does not have to
    /// route back through [`ContentStructure`] or re-match the enum, which is
    /// the hand-rolled walk this module exists to remove.
    #[inline]
    pub fn scoped_annotations(self) -> &'a [ContentAnnotation] {
        match self {
            Self::Annotated(annotated) => annotated.scoped_annotations(),
            Self::Bare(_) => &[],
        }
    }

    /// The retrace node itself, in either spelling.
    #[inline]
    pub fn inner(self) -> &'a Retrace {
        match self {
            Self::Bare(retrace) => retrace,
            Self::Annotated(annotated) => &annotated.inner,
        }
    }
}

/// Whether a leaf is material a listener would HEAR.
///
/// A CHAT fact, so the model owns it. It was written out per variant in
/// `validation::retrace::collection`, twice, once per content enum, sixteen
/// arms that had to agree and were bound by nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafContent {
    /// Events, pauses and other spoken events: audible material.
    Spoken,
    /// Separators, overlap points, bullets, freecodes, actions, and the
    /// long-feature, underline and nonvocal delimiters: notation ABOUT the
    /// utterance rather than speech in it.
    Notation,
}

/// A leaf: content that encloses nothing further.
///
/// A STRUCT, not an enum, and that is the whole point. Two independent facts
/// are true of every leaf, and the predecessor `Unannotated | Event | Action`
/// made one of them the discriminant and hid the other: it named IDENTITY
/// while encoding ANNOTATION STATE, so a bare `&=laughs` was `Unannotated` and
/// a consumer matching `Event` silently missed the commoner spelling. Both
/// facts are fields now, so neither can be read as the other.
#[derive(Debug, Clone, Copy)]
pub struct LeafRef<'a> {
    /// Whether this leaf is audible material.
    pub content: LeafContent,
    /// The annotations scoped to it, empty when it carries none.
    pub annotations: &'a [ContentAnnotation],
}

impl<'a> LeafRef<'a> {
    /// A leaf carrying no annotations.
    const fn bare(content: LeafContent) -> Self {
        Self {
            content,
            annotations: &[],
        }
    }
}

/// The three spellings of a word, each keeping its payload.
///
/// [`ContentStructure::Word`] was payload-free in its first version, and that
/// is what blocked every migration onto this type: `temporal::has_transcribed_content`
/// needs the `&Word` to ask whether it is untranscribed, and
/// `main_tier::word_recursion` WOULD need the whole `Annotated<Word>` so it can
/// validate the annotations rather than only the word inside them. It does not
/// use this type today, and the distinction matters: its own header explains
/// that it must hand each item to that item's `Validate` impl, so migrating it
/// carelessly would drop the annotation validation. A classification that
/// forces its callers to re-match the enum is not an owner.
///
/// `Replaced` deliberately keeps the `ReplacedWord` rather than flattening to
/// its surface word: `dog [: cat]` contributes BOTH the produced form and the
/// target, and which one a caller wants is caller-specific. [`WordRef::words`]
/// is there for callers that want all of them.
#[derive(Debug, Clone, Copy)]
pub enum WordRef<'a> {
    /// `dog`
    Bare(&'a Word),
    /// `dog [* p:w]`
    Annotated(&'a Annotated<Word>),
    /// `dog [: cat]`
    Replaced(&'a ReplacedWord),
}

impl<'a> WordRef<'a> {
    /// Every `Word` this item contributes, produced form first.
    ///
    /// For a replacement that is the surface form followed by the target's
    /// words, because a caller asking "is there a real word here" should see
    /// both and a caller asking "what was said" should take the first.
    #[inline]
    pub fn words(self) -> impl Iterator<Item = &'a Word> {
        let (head, tail) = match self {
            Self::Bare(word) => (word, None),
            Self::Annotated(annotated) => (&annotated.inner, None),
            Self::Replaced(replaced) => (&replaced.word, Some(&replaced.replacement.words)),
        };
        std::iter::once(head).chain(tail.into_iter().flatten())
    }
}

/// The kinds of container that are not retraces, each keeping its payload.
///
/// `Container(&BracketedContent)` erased the kind, and that erasure is what
/// blocked the second wave of migrations onto this type. Two real callers need
/// it, both of them bugs at the time of writing:
///
/// - `validation::main_tier::has_nested_quotation` must know a QUOTATION from
///   any other container, and could not, so `“a <“b”> [/] c”` reported nothing
///   while `“I said “hello” there”` reported E372.
/// - `alignment::helpers::walk` skips `PhoGroup` and `SinGroup` for the Pho and
///   Sin tier domains but not `Quotation`, and inspects an annotated group's
///   annotations to decide whether it is retrace-like. Eight copies of the
///   container set live there waiting on this.
///
/// `Angle` and `AnnotatedAngle` stay separate for the same reason `WordRef`
/// keeps `Annotated`: a caller that must read the annotations cannot get them
/// from a flattened `&BracketedContent`. Note that `BracketedItem` has no bare
/// group variant, so `Angle` arises only from main-tier content.
#[derive(Debug, Clone, Copy)]
pub enum GroupRef<'a> {
    /// `<...>` with no scoped annotations.
    Angle(&'a Group),
    /// `<...> [...]`
    AnnotatedAngle(&'a Annotated<Group>),
    /// `"..."`
    Quotation(&'a Quotation),
    /// A phonological group.
    Pho(&'a PhoGroup),
    /// A sign or gesture group.
    Sin(&'a SinGroup),
}

impl<'a> GroupRef<'a> {
    /// The annotations scoped to this group.
    #[inline]
    pub fn scoped_annotations(self) -> &'a [ContentAnnotation] {
        match self {
            Self::AnnotatedAngle(annotated) => annotated.scoped_annotations(),
            Self::Angle(_) | Self::Quotation(_) | Self::Pho(_) | Self::Sin(_) => &[],
        }
    }

    /// The content this group encloses.
    #[inline]
    pub fn content(self) -> &'a BracketedContent {
        match self {
            Self::Angle(group) => &group.content,
            Self::AnnotatedAngle(annotated) => &annotated.inner.content,
            Self::Quotation(quotation) => &quotation.content,
            Self::Pho(group) => &group.content,
            Self::Sin(group) => &group.content,
        }
    }
}

impl<'a> ContentStructure<'a> {
    /// The content this item encloses, if any.
    ///
    /// Folds the two container-bearing variants together for walkers that only
    /// need to recurse and do not care which kind of container they are in.
    #[inline]
    pub fn enclosed(self) -> Option<&'a BracketedContent> {
        match self {
            Self::Retrace(retrace) => Some(&retrace.inner().content),
            Self::Group(group) => Some(group.content()),
            Self::Word(_) | Self::Leaf(_) => None,
        }
    }
}

impl<'a> ContentStructure<'a> {
    /// Whether any word beneath this item satisfies `predicate`.
    ///
    /// # Why this is here and not written out at each caller
    ///
    /// Three validators wanted the same recursion within a week, each with a
    /// different word predicate, and each hand-rolled it:
    ///
    /// - `retrace::without_words::contains_word`: is there ANY word
    /// - `temporal::has_transcribed_content`: any word that is transcribed
    /// - `main_tier::has_nested_quotation`: (a container test, see below)
    ///
    /// That is the same duplicated-traversal shape this module exists to end,
    /// one level up: the module owns "which variants contain content", and the
    /// callers were each re-deriving "walk them all and test the leaves".
    ///
    /// The nesting rule stays separate on purpose. It tests CONTAINERS rather
    /// than words, so it is a genuinely different question and folding it in
    /// would need a second predicate parameter that two of the three callers
    /// would pass as "always false".
    pub fn any_word(self, predicate: &impl Fn(&Word) -> bool) -> bool {
        match self {
            Self::Word(word) => word.words().any(predicate),
            Self::Retrace(_) | Self::Group(_) => self.enclosed().is_some_and(|content| {
                content
                    .content
                    .iter()
                    .any(|item| item.structure().any_word(predicate))
            }),
            Self::Leaf(_) => false,
        }
    }
}

impl<'a> ContentStructure<'a> {
    /// The annotations scoped to THIS item, empty when it carries none.
    ///
    /// One owner for the annotation axis, which this type used to answer
    /// inconsistently: an annotated group kept its annotations, an annotated
    /// retrace had them dropped on the way in, and an annotated event or
    /// action became a payload-free `Other`. A downstream crate wanting "does
    /// this carry annotations" therefore had to hand-roll a 22-arm match, and
    /// did.
    ///
    /// Scoped to the item itself, NOT to anything beneath it: a caller asking
    /// about a container's contents walks [`ContentStructure::enclosed`].
    pub fn scoped_annotations(self) -> &'a [ContentAnnotation] {
        match self {
            Self::Word(word) => word.scoped_annotations(),
            Self::Retrace(retrace) => retrace.scoped_annotations(),
            Self::Group(group) => group.scoped_annotations(),
            Self::Leaf(leaf) => leaf.annotations,
        }
    }
}

impl<'a> WordRef<'a> {
    /// The annotations scoped to this word.
    ///
    /// Delegates to [`ScopedAnnotated`] for both carriers. `Bare` is the only
    /// spelling with no impl to call, and `Word` has no annotations field, so
    /// the empty slice there is a fact about the struct rather than a decision
    /// made here. The predecessor decided all three by hand and answered wrong
    /// for `Replaced`.
    #[inline]
    pub fn scoped_annotations(self) -> &'a [ContentAnnotation] {
        match self {
            Self::Annotated(annotated) => annotated.scoped_annotations(),
            Self::Replaced(replaced) => replaced.scoped_annotations(),
            Self::Bare(_) => &[],
        }
    }
}

impl UtteranceContent {
    /// Classify this item for traversal. See [`ContentStructure`].
    ///
    /// `#[inline]` because this is called once per content item on the hottest
    /// walk in the crate, and `[profile.release]` here deliberately carries no
    /// LTO, so a cross-codegen-unit call would not be inlined on its own.
    #[inline]
    pub fn structure(&self) -> ContentStructure<'_> {
        match self {
            Self::Word(word) => ContentStructure::Word(WordRef::Bare(word)),
            Self::AnnotatedWord(annotated) => ContentStructure::Word(WordRef::Annotated(annotated)),
            Self::ReplacedWord(replaced) => ContentStructure::Word(WordRef::Replaced(replaced)),
            Self::Retrace(retrace) => ContentStructure::Retrace(RetraceRef::Bare(retrace)),
            Self::AnnotatedRetrace(annotated) => {
                ContentStructure::Retrace(RetraceRef::Annotated(annotated))
            }
            Self::AnnotatedEvent(annotated) => ContentStructure::Leaf(LeafRef {
                content: LeafContent::Spoken,
                annotations: annotated.scoped_annotations(),
            }),
            Self::AnnotatedAction(annotated) => ContentStructure::Leaf(LeafRef {
                content: LeafContent::Notation,
                annotations: annotated.scoped_annotations(),
            }),
            Self::Group(group) => ContentStructure::Group(GroupRef::Angle(group)),
            Self::AnnotatedGroup(annotated) => {
                ContentStructure::Group(GroupRef::AnnotatedAngle(annotated))
            }
            Self::Quotation(quotation) => ContentStructure::Group(GroupRef::Quotation(quotation)),
            Self::PhoGroup(group) => ContentStructure::Group(GroupRef::Pho(group)),
            Self::SinGroup(group) => ContentStructure::Group(GroupRef::Sin(group)),
            // Listed rather than caught by `_`: a catch-all here would route a
            // future container variant to "leaf", which is the exact drift this
            // module exists to end.
            Self::Event(_) | Self::Pause(_) | Self::OtherSpokenEvent(_) => {
                ContentStructure::Leaf(LeafRef::bare(LeafContent::Spoken))
            }
            Self::Freecode(_)
            | Self::Separator(_)
            | Self::OverlapPoint(_)
            | Self::InternalBullet(_)
            | Self::LongFeatureBegin(_)
            | Self::LongFeatureEnd(_)
            | Self::UnderlineBegin(_)
            | Self::UnderlineEnd(_)
            | Self::NonvocalBegin(_)
            | Self::NonvocalEnd(_)
            | Self::NonvocalSimple(_) => {
                ContentStructure::Leaf(LeafRef::bare(LeafContent::Notation))
            }
        }
    }
}

impl BracketedItem {
    /// Classify this item for traversal. See [`ContentStructure`].
    #[inline]
    pub fn structure(&self) -> ContentStructure<'_> {
        match self {
            Self::Word(word) => ContentStructure::Word(WordRef::Bare(word)),
            Self::AnnotatedWord(annotated) => ContentStructure::Word(WordRef::Annotated(annotated)),
            Self::ReplacedWord(replaced) => ContentStructure::Word(WordRef::Replaced(replaced)),
            Self::Retrace(retrace) => ContentStructure::Retrace(RetraceRef::Bare(retrace)),
            Self::AnnotatedRetrace(annotated) => {
                ContentStructure::Retrace(RetraceRef::Annotated(annotated))
            }
            Self::AnnotatedGroup(annotated) => {
                ContentStructure::Group(GroupRef::AnnotatedAngle(annotated))
            }
            Self::Quotation(quotation) => ContentStructure::Group(GroupRef::Quotation(quotation)),
            Self::PhoGroup(group) => ContentStructure::Group(GroupRef::Pho(group)),
            Self::SinGroup(group) => ContentStructure::Group(GroupRef::Sin(group)),
            Self::AnnotatedEvent(annotated) => ContentStructure::Leaf(LeafRef {
                content: LeafContent::Spoken,
                annotations: annotated.scoped_annotations(),
            }),
            Self::AnnotatedAction(annotated) => ContentStructure::Leaf(LeafRef {
                content: LeafContent::Notation,
                annotations: annotated.scoped_annotations(),
            }),
            // Exhaustive for the same reason as above.
            Self::Event(_) | Self::Pause(_) | Self::OtherSpokenEvent(_) => {
                ContentStructure::Leaf(LeafRef::bare(LeafContent::Spoken))
            }
            Self::Action(_)
            | Self::OverlapPoint(_)
            | Self::Separator(_)
            | Self::InternalBullet(_)
            | Self::Freecode(_)
            | Self::LongFeatureBegin(_)
            | Self::LongFeatureEnd(_)
            | Self::UnderlineBegin(_)
            | Self::UnderlineEnd(_)
            | Self::NonvocalBegin(_)
            | Self::NonvocalEnd(_)
            | Self::NonvocalSimple(_) => {
                ContentStructure::Leaf(LeafRef::bare(LeafContent::Notation))
            }
        }
    }
}
