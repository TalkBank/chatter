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
pub(crate) trait ScopedAnnotated {
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
    ///
    /// Use [`Self::span`] rather than `inner().span` to LOCATE the construct:
    /// the annotated spelling's own span covers its annotations and the inner
    /// one does not.
    #[inline]
    pub fn inner(self) -> &'a Retrace {
        match self {
            Self::Bare(retrace) => retrace,
            Self::Annotated(annotated) => &annotated.inner,
        }
    }

    /// Where this retrace is, INCLUDING its annotations when it has them.
    #[inline]
    #[must_use]
    pub fn span(self) -> crate::Span {
        match self {
            Self::Bare(retrace) => retrace.span,
            Self::Annotated(annotated) => annotated.span,
        }
    }
}

/// The two spellings of a quotation, each keeping its payload.
///
/// Mirrors [`RetraceRef`], and for a reason paid for in a shipped bug. Until
/// 2026-08-27 these were two SIBLING variants of [`GroupRef`], `Quotation` and
/// `AnnotatedQuotation`, so "is this a quotation" was a two-arm question that
/// every caller had to spell out and that nothing forced anyone to spell in
/// full. `matches!(group, GroupRef::Quotation(_))` compiles and silently
/// answers "no" for half the quotations that exist.
///
/// Two callers wrote that list. `alignment::helpers::descent` named both
/// spellings; `validation::main_tier::has_nested_quotation` named one. The
/// annotated spelling was introduced by the same release that added scoped
/// annotations to quotations, and the nesting rule was never taught about it,
/// so E372 stopped firing the moment either quotation carried an annotation:
///
/// ```text
/// *CHI:    “a “b” c” .              reported E372
/// *CHI:    “a “b” c” [//] hello .    reported NOTHING
/// *CHI:    “a “b” [% note] c” .      reported NOTHING
/// ```
///
/// A STRUCT, not an enum, for the reason [`LeafRef`] below spells out: an enum
/// would name IDENTITY while encoding ANNOTATION STATE, so
/// `matches!(q, QuotationRef::Bare(_))` would be the same half-answer one
/// level down, and a consumer would silently miss the annotated spelling
/// exactly as `GroupRef` did. Both facts are fields, so neither can be read as
/// the other, and there is no arm to forget.
#[derive(Debug, Clone, Copy)]
pub struct QuotationRef<'a> {
    /// The quotation itself, whichever spelling carried it.
    pub quotation: &'a Quotation,
    /// The annotations scoped to it, empty when it carries none.
    pub annotations: &'a [ContentAnnotation],
    /// Where the construct is, INCLUDING its annotations when it has them.
    ///
    /// Not `quotation.span`, and the difference is load-bearing: the annotated
    /// spelling's own span covers the annotations and the inner one does not.
    /// `GroupRef::span` drew that distinction by hand before this type existed,
    /// so folding the spellings without carrying it would have narrowed every
    /// annotated quotation's reported location, silently.
    pub span: crate::Span,
}

impl<'a> QuotationRef<'a> {
    /// A quotation carrying no annotations.
    #[inline]
    fn bare(quotation: &'a Quotation) -> Self {
        Self {
            quotation,
            annotations: &[],
            span: quotation.span,
        }
    }

    /// A quotation carrying its own scoped annotations.
    #[inline]
    fn annotated(annotated: &'a Annotated<Quotation>) -> Self {
        Self {
            quotation: &annotated.inner,
            annotations: annotated.scoped_annotations(),
            span: annotated.span,
        }
    }
}

/// The two spellings of an angle group, in the shape [`QuotationRef`] explains.
///
/// A STRUCT for the same reason: sibling variants would name IDENTITY while
/// encoding ANNOTATION STATE, and `matches!(g, AngleRef::Bare(_))` would be the
/// half-answer one level down.
#[derive(Debug, Clone, Copy)]
pub struct AngleRef<'a> {
    /// The group itself, whichever spelling carried it.
    pub group: &'a Group,
    /// The annotations scoped to it, empty when it carries none.
    pub annotations: &'a [ContentAnnotation],
    /// Where the construct is, INCLUDING its annotations when it has them.
    pub span: crate::Span,
}

impl<'a> AngleRef<'a> {
    /// A group carrying no annotations.
    #[inline]
    fn bare(group: &'a Group) -> Self {
        Self {
            group,
            annotations: &[],
            span: group.span,
        }
    }

    /// A group carrying its own scoped annotations.
    #[inline]
    fn annotated(annotated: &'a Annotated<Group>) -> Self {
        Self {
            group: &annotated.inner,
            annotations: annotated.scoped_annotations(),
            span: annotated.span,
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

impl WordRef<'_> {
    /// Where this word is.
    ///
    /// Infallible: all three spellings carry a span. Added 2026-08-26 because
    /// the LSP was answering this question by matching all 28 `UtteranceContent`
    /// variants itself, in five byte-identical copies, which meant a new
    /// variant was classified once here and once there and the two could
    /// disagree where nothing would notice.
    #[inline]
    #[must_use]
    pub fn span(self) -> crate::Span {
        match self {
            Self::Bare(word) => word.span,
            Self::Annotated(annotated) => annotated.span,
            Self::Replaced(replaced) => replaced.span,
        }
    }
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
/// `Angle` and `Quotation` each carry BOTH spellings, bare and annotated, as a
/// struct rather than as sibling variants. The reason once recorded here for
/// keeping them apart was "a caller that must read the annotations cannot get
/// them from a flattened `&BracketedContent`", which argued against FLATTENING
/// and never applied: neither was flattened, and [`AngleRef`] and
/// [`QuotationRef`] hand the annotations over as a field. Two sibling variants
/// made `matches!(group, GroupRef::Angle(_))` silently answer "no" for
/// `<a b> [xyz]`, which is the bug [`QuotationRef`] documents.
///
/// Note that `BracketedItem` has no bare group variant, so a bare `Angle`
/// arises only from main-tier content.
#[derive(Debug, Clone, Copy)]
pub enum GroupRef<'a> {
    /// `<...>`, with or without scoped annotations.
    Angle(AngleRef<'a>),
    /// A quotation, in either spelling. ONE variant on purpose; see
    /// [`QuotationRef`] for the bug that two of them shipped.
    Quotation(QuotationRef<'a>),
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
            Self::Angle(group) => group.annotations,
            Self::Quotation(quotation) => quotation.annotations,
            Self::Pho(_) | Self::Sin(_) => &[],
        }
    }

    /// The content this group encloses.
    #[inline]
    pub fn content(self) -> &'a BracketedContent {
        match self {
            Self::Angle(group) => &group.group.content,
            Self::Quotation(quotation) => &quotation.quotation.content,
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

/// What a visitor wants to happen BENEATH the item it was just handed.
///
/// The third state is why this exists as an enum rather than a `bool`.
/// `validation::retrace::visit` removed a `ControlFlow` in August 2026 saying
/// "reinstate it when a caller genuinely needs it, not before"; the nested
/// quotation rule is that caller, and it needs the middle one. It must not
/// descend past a quotation, because the predicate it runs there already
/// answers for the whole subtree, and walking on would report the same nesting
/// once per level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Descend {
    /// Walk what this item encloses.
    Into,
    /// Leave this item's contents alone; carry on with its siblings.
    Skip,
    /// Stop the traversal entirely.
    Stop,
}

impl<'a> ContentStructure<'a> {
    /// Visit this item, then whatever it encloses, outermost first.
    ///
    /// # Why descent has ONE owner
    ///
    /// This module already owns which variants contain content
    /// ([`Self::enclosed`]); the walk over them was written out four times
    /// anyway, in `any_word`, in `validation::retrace::visit::visit_structure`,
    /// and twice in the nested-quotation rule. Every one of them was the same
    /// `enclosed()` loop with a different action at the node, and every one is
    /// a place the next container variant would be a silent omission.
    ///
    /// Recursive and closure-driven rather than an `Iterator`, deliberately: an
    /// iterator over a tree needs an explicit stack, and this runs per utterance
    /// of every file in a six-figure corpus. This allocates nothing.
    /// Returns `()`, not a `ControlFlow`. [`Descend::Stop`] ends the traversal,
    /// and a caller that wants to know whether it fired learns that from the
    /// same captured state that decided to stop: every real caller here already
    /// holds one. Returning the flow as well made three of four call sites
    /// discard a `#[must_use]`, which is noise standing where a decision looks
    /// like it should be.
    pub fn walk(self, visit: &mut impl FnMut(Self) -> Descend) {
        // The flow is consumed HERE, at the one place that knows it carries no
        // information a caller could act on: a `Break` means the visitor asked
        // to stop and already recorded why.
        let (core::ops::ControlFlow::Continue(()) | core::ops::ControlFlow::Break(())) =
            self.walk_inner(visit);
    }

    /// The recursion, which DOES need the flow to unwind a `Stop` through every
    /// enclosing level.
    fn walk_inner(self, visit: &mut impl FnMut(Self) -> Descend) -> core::ops::ControlFlow<()> {
        match visit(self) {
            Descend::Stop => return core::ops::ControlFlow::Break(()),
            Descend::Skip => return core::ops::ControlFlow::Continue(()),
            Descend::Into => {}
        }
        if let Some(content) = self.enclosed() {
            for item in content.content.iter() {
                item.structure().walk_inner(visit)?;
            }
        }
        core::ops::ControlFlow::Continue(())
    }

    /// Where this item is, when it carries a span of its own.
    ///
    /// `None` only for a leaf: events, pauses, actions, CA markers, bullets and
    /// the delimiters record no span of their own, so a caller wanting to
    /// report about one falls back to the tier's span and should say so.
    /// Every other ref type answers infallibly.
    #[inline]
    #[must_use]
    pub fn span(self) -> Option<crate::Span> {
        match self {
            Self::Word(word) => Some(word.span()),
            Self::Retrace(retrace) => Some(retrace.span()),
            Self::Group(group) => group.span(),
            Self::Leaf(_) => None,
        }
    }

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
        let mut found = false;
        self.walk(&mut |structure| match structure {
            Self::Word(word) if word.words().any(predicate) => {
                found = true;
                Descend::Stop
            }
            // A word encloses nothing, so there is nothing below it to skip;
            // `Into` and `Skip` are the same answer here and `Into` says less.
            Self::Word(_) | Self::Retrace(_) | Self::Group(_) | Self::Leaf(_) => Descend::Into,
        });
        found
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
    /// Delegates to `ScopedAnnotated` for both carriers. `Bare` is the only
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
            Self::Action(_) => ContentStructure::Leaf(LeafRef::bare(LeafContent::Notation)),
            Self::AnnotatedAction(annotated) => ContentStructure::Leaf(LeafRef {
                content: LeafContent::Notation,
                annotations: annotated.scoped_annotations(),
            }),
            Self::Group(group) => ContentStructure::Group(GroupRef::Angle(AngleRef::bare(group))),
            Self::AnnotatedGroup(annotated) => {
                ContentStructure::Group(GroupRef::Angle(AngleRef::annotated(annotated)))
            }
            Self::AnnotatedQuotation(annotated) => {
                ContentStructure::Group(GroupRef::Quotation(QuotationRef::annotated(annotated)))
            }
            Self::Quotation(quotation) => {
                ContentStructure::Group(GroupRef::Quotation(QuotationRef::bare(quotation)))
            }
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
            Self::Group(group) => ContentStructure::Group(GroupRef::Angle(AngleRef::bare(group))),
            Self::Retrace(retrace) => ContentStructure::Retrace(RetraceRef::Bare(retrace)),
            Self::AnnotatedRetrace(annotated) => {
                ContentStructure::Retrace(RetraceRef::Annotated(annotated))
            }
            Self::AnnotatedGroup(annotated) => {
                ContentStructure::Group(GroupRef::Angle(AngleRef::annotated(annotated)))
            }
            Self::AnnotatedQuotation(annotated) => {
                ContentStructure::Group(GroupRef::Quotation(QuotationRef::annotated(annotated)))
            }
            Self::Quotation(quotation) => {
                ContentStructure::Group(GroupRef::Quotation(QuotationRef::bare(quotation)))
            }
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

/// Which kind of container a group is, independent of how it is borrowed.
///
/// [`GroupRef`] encodes this in its variants, which is right for a `Copy` view
/// whose callers want the payload. A walker that only needs to DECIDE (does
/// this tier domain descend into a phonological group?) wants the kind alone,
/// and [`GroupMut`] cannot spell it as variants without repeating the payload
/// split six times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    /// `<...>`, annotated or not.
    Angle,
    /// A quotation, annotated or not.
    Quotation,
    /// A phonological group.
    Pho,
    /// A sign or gesture group.
    Sin,
}

impl GroupRef<'_> {
    /// Where this group is, or `None` when the model records no span for it.
    ///
    /// `None` is a FACT here, not a policy: `PhoGroup` and `SinGroup` carry
    /// only their content and are the only content containers with no span
    /// field, while `Group` and `Quotation` next door have theirs. A caller
    /// that wants to exclude a kind it COULD locate is making a different
    /// decision and should say so on `kind()`.
    #[inline]
    #[must_use]
    pub fn span(self) -> Option<crate::Span> {
        match self {
            Self::Angle(group) => Some(group.span),
            Self::Quotation(quotation) => Some(quotation.span),
            Self::Pho(_) | Self::Sin(_) => None,
        }
    }
}

impl GroupRef<'_> {
    /// Which kind of container this is.
    #[inline]
    #[must_use]
    pub fn kind(self) -> GroupKind {
        match self {
            Self::Angle(_) => GroupKind::Angle,
            Self::Quotation(_) => GroupKind::Quotation,
            Self::Pho(_) => GroupKind::Pho,
            Self::Sin(_) => GroupKind::Sin,
        }
    }
}

/// A container reached through `&mut`: its kind, its annotations, and the
/// content it encloses.
///
/// # Why this is not `ContentStructure` with `&mut` substituted
///
/// [`ContentStructure`] is `Copy` and its accessors take `self`, which a
/// unique borrow cannot do twice. A walker needs BOTH the annotations that
/// gate the descent and the content to descend into, so this hands over all
/// three facts at once ([`Self::into_parts`]) rather than offering three
/// accessors of which only the first is callable.
///
/// That is also the property worth having: the content cannot be obtained
/// while forgetting the annotations and the kind that decide whether to use
/// it. The four `_mut` walkers each wrote a container arm PER VARIANT applying
/// different gate rules, and the arms drifted: `walk/bracketed.rs` shipped
/// four ungated `AnnotatedQuotation` arms on 2026-08-26 while `count.rs` gated
/// the same variant, so the two disagreed about one node.
///
/// # It supplies FACTS, not policy
///
/// Deliberately no `should_descend(domain)` method. `TierDomain` is an
/// alignment concept and this module answers "what shape is this", exactly as
/// [`ContentStructure`]'s own header says. The gates stay with the walkers
/// that own them; what changes is that they are written once per walker
/// instead of once per container variant.
#[derive(Debug)]
pub struct GroupMut<'a> {
    kind: GroupKind,
    scoped_annotations: &'a [ContentAnnotation],
    content: &'a mut BracketedContent,
}

impl<'a> GroupMut<'a> {
    /// The kind, the annotations scoped to this group, and its content.
    ///
    /// The annotations are empty for the unannotated spellings, matching
    /// [`GroupRef::scoped_annotations`]. The two references borrow DISJOINT
    /// fields, which is what makes handing out a shared and a unique borrow
    /// together sound.
    #[inline]
    #[must_use]
    pub fn into_parts(self) -> (GroupKind, &'a [ContentAnnotation], &'a mut BracketedContent) {
        (self.kind, self.scoped_annotations, self.content)
    }
}

/// A content item that encloses further content, reached through `&mut`.
///
/// Retraces are separate from groups for the same reason [`ContentStructure`]
/// separates them: their descent is gated by the tier domain alone, never by
/// annotations, and a walker that conflated the two would apply the wrong
/// rule.
#[derive(Debug)]
pub enum ContainerMut<'a> {
    /// A group, quotation, or phonological/sign group.
    Group(GroupMut<'a>),
    /// A retrace; the material it retraces is its content.
    Retrace(&'a mut BracketedContent),
}

impl UtteranceContent {
    /// The content this item encloses, reached mutably, or `None` for a leaf.
    ///
    /// The mutable counterpart of [`ContentStructure::enclosed`], except that
    /// it keeps the kind and annotations `enclosed` discards, because those
    /// are what every caller's descent gate reads.
    #[inline]
    pub fn container_mut(&mut self) -> Option<ContainerMut<'_>> {
        match self {
            Self::Group(group) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Angle,
                scoped_annotations: &[],
                content: &mut group.content,
            })),
            Self::AnnotatedGroup(annotated) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Angle,
                scoped_annotations: &annotated.scoped_annotations,
                content: &mut annotated.inner.content,
            })),
            Self::Quotation(quotation) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Quotation,
                scoped_annotations: &[],
                content: &mut quotation.content,
            })),
            Self::AnnotatedQuotation(annotated) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Quotation,
                scoped_annotations: &annotated.scoped_annotations,
                content: &mut annotated.inner.content,
            })),
            Self::PhoGroup(group) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Pho,
                scoped_annotations: &[],
                content: &mut group.content,
            })),
            Self::SinGroup(group) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Sin,
                scoped_annotations: &[],
                content: &mut group.content,
            })),
            Self::Retrace(retrace) => Some(ContainerMut::Retrace(&mut retrace.content)),
            Self::AnnotatedRetrace(annotated) => {
                Some(ContainerMut::Retrace(&mut annotated.inner.content))
            }
            // Leaves. Listed rather than `_ =>` so a new container variant is a
            // compile error here, which is the whole reason this module exists.
            Self::Word(_)
            | Self::AnnotatedWord(_)
            | Self::ReplacedWord(_)
            | Self::Event(_)
            | Self::AnnotatedEvent(_)
            | Self::Action(_)
            | Self::AnnotatedAction(_)
            | Self::Pause(_)
            | Self::Freecode(_)
            | Self::Separator(_)
            | Self::OverlapPoint(_)
            | Self::InternalBullet(_)
            | Self::LongFeatureBegin(_)
            | Self::LongFeatureEnd(_)
            | Self::UnderlineBegin(_)
            | Self::UnderlineEnd(_)
            | Self::NonvocalBegin(_)
            | Self::NonvocalEnd(_)
            | Self::NonvocalSimple(_)
            | Self::OtherSpokenEvent(_) => None,
        }
    }
}

impl BracketedItem {
    /// The content this item encloses, reached mutably, or `None` for a leaf.
    ///
    /// The bracketed counterpart of [`UtteranceContent::container_mut`]. Note
    /// `BracketedItem` has no bare `Group` variant (a bare `<...>` cannot
    /// appear inside brackets), which is why `GroupKind::Angle` arises here
    /// only from the annotated spelling.
    #[inline]
    pub fn container_mut(&mut self) -> Option<ContainerMut<'_>> {
        match self {
            Self::Group(group) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Angle,
                scoped_annotations: &[],
                content: &mut group.content,
            })),
            Self::AnnotatedGroup(annotated) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Angle,
                scoped_annotations: &annotated.scoped_annotations,
                content: &mut annotated.inner.content,
            })),
            Self::Quotation(quotation) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Quotation,
                scoped_annotations: &[],
                content: &mut quotation.content,
            })),
            Self::AnnotatedQuotation(annotated) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Quotation,
                scoped_annotations: &annotated.scoped_annotations,
                content: &mut annotated.inner.content,
            })),
            Self::PhoGroup(group) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Pho,
                scoped_annotations: &[],
                content: &mut group.content,
            })),
            Self::SinGroup(group) => Some(ContainerMut::Group(GroupMut {
                kind: GroupKind::Sin,
                scoped_annotations: &[],
                content: &mut group.content,
            })),
            Self::Retrace(retrace) => Some(ContainerMut::Retrace(&mut retrace.content)),
            Self::AnnotatedRetrace(annotated) => {
                Some(ContainerMut::Retrace(&mut annotated.inner.content))
            }
            // Leaves, listed rather than `_ =>` for the reason above.
            Self::Word(_)
            | Self::AnnotatedWord(_)
            | Self::ReplacedWord(_)
            | Self::Event(_)
            | Self::AnnotatedEvent(_)
            | Self::Pause(_)
            | Self::Action(_)
            | Self::AnnotatedAction(_)
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
            | Self::NonvocalSimple(_)
            | Self::OtherSpokenEvent(_) => None,
        }
    }
}

#[cfg(test)]
mod container_mut_tests {
    use super::*;
    use crate::model::{BracketedContent, Group, Quotation};

    /// The unannotated spellings report NO annotations, which is what makes the
    /// four `_mut` walkers' collapse behaviour-preserving.
    ///
    /// SURVIVES a type: the walkers replaced their per-variant container arms with one gate
    /// per RULE, and the bare `Group` / `Quotation` arms previously had NO gate
    /// at all. They now route through `descent::descends_into_group`, whose
    /// annotation arm reduces to `domain == Mor && annotations.iter().any(..)`.
    /// That is `false` for an empty slice, so the collapse preserves the
    /// unconditional descent, and
    /// this pins the empty-slice half of that argument at the source. A type
    /// cannot state it: `&'a [ContentAnnotation]` admits both empty and not.
    #[test]
    fn unannotated_containers_carry_no_annotations() {
        let mut bare_group = UtteranceContent::Group(Group::new(BracketedContent::new(Vec::new())));
        let Some(ContainerMut::Group(group)) = bare_group.container_mut() else {
            panic!("a bare group is a container");
        };
        let (kind, annotations, _) = group.into_parts();
        assert_eq!(kind, GroupKind::Angle);
        assert!(annotations.is_empty(), "a bare group gates on nothing");

        let mut bare_quotation =
            UtteranceContent::Quotation(Quotation::new(BracketedContent::new(Vec::new())));
        let Some(ContainerMut::Group(group)) = bare_quotation.container_mut() else {
            panic!("a bare quotation is a container");
        };
        let (kind, annotations, _) = group.into_parts();
        assert_eq!(
            kind,
            GroupKind::Quotation,
            "a quotation is not an angle group"
        );
        assert!(annotations.is_empty(), "a bare quotation gates on nothing");
    }

    /// A leaf is not a container, so a walker cannot descend into one.
    #[test]
    fn a_leaf_has_no_container() {
        let mut pause =
            UtteranceContent::Pause(crate::model::Pause::new(crate::model::PauseDuration::Short));
        assert!(pause.container_mut().is_none());
    }
}
