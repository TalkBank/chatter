//! What a tier-domain traversal does with a container, decided once for every
//! traversal in this crate.
//!
//! # Three outcomes, not two, and the third is why this module moved
//!
//! - **`Into`**: enter the container and keep going.
//! - **`Atomic`**: do not enter, because the container is itself ONE alignable
//!   position in this tier. A phonological group is one `%pho` unit; the words
//!   inside it are not separately aligned.
//! - **`Excluded`**: do not enter, and it contributes nothing at all.
//!
//! A walker that emits WORDS cannot tell `Atomic` from `Excluded`, since
//! neither yields a word, and this module used to return an `Option` that
//! folded them together. That fold was the reason `count.rs` could not share
//! the rule and hand-wrote it about thirty more times across four traversals,
//! which is how the two drifted: `walk/bracketed.rs` shipped four ungated
//! `AnnotatedQuotation` arms on 2026-08-26 while `count.rs` gated the same
//! variant, so one node was walked by one and skipped by the other.
//!
//! Measured before the fold was removed: of the thirty-six (container, domain)
//! cells, exactly FOUR need the third state, and all four are a phonological or
//! sign group under a domain that measures it. The other thirty-two were
//! already this function.
//!
//! # Where the rules live
//!
//! [`measuring_verdict`] owns the phonological/sign table and
//! [`excluded_by_annotations`] owns the annotation rule. Both entry points
//! assemble their answer from those two and nothing else, so the two spellings
//! below differ only in what they can carry, never in what they decide.
//! `walk::tests::container_descent_table_is_one_rule_for_both_consumers`
//! measures every cell through both consumers and names the one that moved.
//!
//! # Why the rules are not on the structural types
//!
//! `ContentStructure`, `GroupRef` and [`ContainerMut`] answer "what SHAPE is
//! this item" and deliberately do not know `TierDomain`; their own headers say
//! so. The shape types supply the facts and this module applies the policy.

// The sibling traversal modules carry this and this one decides for all of
// them: a new container variant landing in the wrong arm here would silently
// change what every walker and every count sees.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::ContentStructure;
use crate::alignment::helpers::domain::TierDomain;
use crate::alignment::helpers::rules::annotations_have_alignment_ignore;
use crate::model::{
    BracketedContent, ContainerMut, ContentAnnotation, GroupKind, GroupRef, PhoGroup, SinGroup,
};

use super::to_chat_display_string;
use super::walk::LanguageScope;

/// Whether `annotations` exclude what they are scoped to from `domain`.
///
/// Only `%mor` excludes: an exclusion marker says the material has no
/// morphological analysis, but the speaker still PRODUCED it, so `%pho`,
/// `%sin` and `%wor` all include it. `None` is a traversal that is not
/// tier-scoped at all, which excludes nothing.
///
/// Used by the container rules below AND by every `AnnotatedWord` arm in the
/// walkers: a word carries its own scoped annotations exactly as a group does,
/// and the exclusion question is the same one. It absorbed
/// `rules::should_skip_group`, which was this minus the `Option`, once the
/// four `count.rs` traversals that were its other caller started coming
/// through here.
#[inline]
pub(super) fn excluded_by_annotations(
    annotations: &[ContentAnnotation],
    domain: Option<TierDomain>,
) -> bool {
    match domain {
        // Only `%mor` excludes: an exclusion marker says the material has no
        // morphological analysis, but the speaker still PRODUCED it.
        Some(TierDomain::Mor) => annotations_have_alignment_ignore(annotations),
        Some(TierDomain::Pho | TierDomain::Sin | TierDomain::Wor) | None => false,
    }
}

/// [`excluded_by_annotations`] shaped as a [`Verdict`], for the container path.
///
/// Two views of ONE rule rather than two rules: a WORD is never atomic, so the
/// word arms in the walkers want the boolean, while a container has three
/// possible answers and wants the verdict.
#[inline]
fn annotation_verdict(annotations: &[ContentAnnotation], domain: Option<TierDomain>) -> Verdict {
    if excluded_by_annotations(annotations, domain) {
        Verdict::Excluded
    } else {
        Verdict::Enter
    }
}

/// The two group kinds a tier domain can measure as a unit of its own.
///
/// A subset of [`GroupKind`] rather than the whole of it, so that
/// [`measuring_verdict`] cannot be asked about an angle group, and
/// [`Descent::Atomic`] cannot be constructed for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AtomicKind {
    /// A phonological group, one `%pho` unit.
    Pho,
    /// A sign or gesture group, one `%sin` unit.
    Sin,
}

/// What a traversal does with a container, before any payload is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Enter,
    Atomic,
    Excluded,
}

/// The phonological/sign table: what `domain` does with a group of `kind`.
///
/// The whole cross-product, with no cell folded into another, because the
/// interesting fact is that the two CROSS cells differ: a phonological group
/// is one unit under `%pho` and contributes nothing at all under `%sin`.
/// Collapsing those to one boolean is what a walker does, and it is why a
/// walker's answer cannot serve a count.
#[inline]
fn measuring_verdict(kind: AtomicKind, domain: Option<TierDomain>) -> Verdict {
    match (kind, domain) {
        // No tier domain, or a domain with no tier for these groups: the words
        // inside are ordinary main-tier words.
        (_, None) | (_, Some(TierDomain::Mor | TierDomain::Wor)) => Verdict::Enter,
        // The domain that measures this kind treats it as one unit.
        (AtomicKind::Pho, Some(TierDomain::Pho)) | (AtomicKind::Sin, Some(TierDomain::Sin)) => {
            Verdict::Atomic
        }
        // The OTHER measuring domain: a phonological group has no sign
        // representation, and a sign group has no phonological one.
        (AtomicKind::Pho, Some(TierDomain::Sin)) | (AtomicKind::Sin, Some(TierDomain::Pho)) => {
            Verdict::Excluded
        }
    }
}

/// What `domain` does with a RETRACE.
///
/// Written as an exhaustive `match` rather than `matches!(domain, Some(Mor))`,
/// and that is the point rather than a style choice: this module denies
/// `clippy::wildcard_enum_match_arm`, and THE LINT DOES NOT FIRE ON `matches!`.
/// The rule was spelled out three times in `matches!` form here, in the file
/// whose whole purpose is that the container rules have one owner and a full
/// cross-product. A fifth `TierDomain` would have compiled clean and silently
/// decided that retraced material is included under it.
///
/// `%mor` does not morphologically analyse retraced material. Every other
/// domain includes it, because the speaker did produce it.
#[inline]
fn retrace_verdict(domain: Option<TierDomain>) -> Verdict {
    match domain {
        Some(TierDomain::Mor) => Verdict::Excluded,
        Some(TierDomain::Pho | TierDomain::Sin | TierDomain::Wor) | None => Verdict::Enter,
    }
}

/// A container that IS one alignable position, keeping what describes it.
#[derive(Debug, Clone, Copy)]
pub(super) enum AtomicUnit<'a> {
    /// A phonological group under `%pho`.
    Pho(&'a PhoGroup),
    /// A sign group under `%sin`.
    Sin(&'a SinGroup),
}

impl AtomicUnit<'_> {
    /// How a position of this kind is described to a reader.
    ///
    /// The one owner of these two strings, which were written out at the two
    /// `extract` call sites that build a `TierPosition`. It sits here rather
    /// than on [`AtomicKind`] because both call sites already hold the unit;
    /// routing through a `kind()` accessor was a hop that existed only to
    /// reach this method.
    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Pho(_) => "phonological group",
            Self::Sin(_) => "sign group",
        }
    }

    /// The CHAT text of this position, for display.
    ///
    /// Presentation only: this is what a reader sees in a tier-position
    /// listing, never something re-parsed.
    pub(super) fn display_text(self) -> String {
        match self {
            Self::Pho(group) => to_chat_display_string(group),
            Self::Sin(group) => to_chat_display_string(group),
        }
    }
}

/// A container the traversal is entering: what to descend into, and what
/// governs the language scope inside it.
///
/// # Why the scope is a method and not the annotations
///
/// The word-emitting walkers thread a [`LanguageScope`] and must enter a
/// `<...> [@s]` group's scope, but a RETRACE opens none: its annotations
/// describe the retrace itself, not the material inside it. That exception was
/// written out per variant, and losing it is silent, because the wrong scope
/// still type-checks and still walks every word.
///
/// [`Self::scope_in`] is the whole interface to it. Handing out the
/// annotations instead would be worse than it looks: for a retrace they would
/// have to be empty, and "this container carries no annotations" is FALSE of an
/// annotated retrace.
#[derive(Debug, Clone, Copy)]
pub(super) struct Entered<'a> {
    content: &'a BracketedContent,
    scope_annotations: &'a [ContentAnnotation],
}

impl<'a> Entered<'a> {
    /// The content to descend into.
    #[inline]
    pub(super) fn content(self) -> &'a BracketedContent {
        self.content
    }

    /// The language scope in force INSIDE this container, given the scope
    /// outside it.
    #[inline]
    pub(super) fn scope_in(self, outer: LanguageScope<'a>) -> LanguageScope<'a> {
        outer.inside(self.scope_annotations)
    }
}

/// What a traversal for a given domain does with one container.
#[derive(Debug, Clone, Copy)]
pub(super) enum Descent<'a> {
    /// Enter it.
    Into(Entered<'a>),
    /// Do not enter: it is one alignable position in this tier, of its own.
    Atomic(AtomicUnit<'a>),
    /// Do not enter: it contributes nothing to this tier.
    Excluded,
}

impl<'a> Descent<'a> {
    /// The content to walk into, for a consumer that only emits WORDS.
    ///
    /// Such a consumer treats `Atomic` and `Excluded` identically, because
    /// neither yields a word. Only a consumer counting POSITIONS can tell them
    /// apart, which is exactly the information this projection discards, and
    /// exactly why it is a named projection rather than the return type.
    #[inline]
    pub(super) fn entered(self) -> Option<Entered<'a>> {
        match self {
            Self::Into(entered) => Some(entered),
            Self::Atomic(_) | Self::Excluded => None,
        }
    }
}

/// What a traversal for `domain` does with the container at `structure`, or
/// [`Descent::Excluded`] for an item that encloses nothing at all.
///
/// A non-container reads as `Excluded` because that is what it is worth to a
/// traversal: nothing to enter and no position of its own. Callers reach this
/// only from an arm that has already matched the container variants.
#[inline]
pub(super) fn descend<'a>(
    structure: ContentStructure<'a>,
    domain: Option<TierDomain>,
) -> Descent<'a> {
    let into = |group: GroupRef<'a>| {
        Descent::Into(Entered {
            content: group.content(),
            scope_annotations: group.scoped_annotations(),
        })
    };
    match structure {
        // The measuring kinds match on the REFERENCE, so the atomic payload is
        // attached where the reference is in hand and an angle group can never
        // reach `Atomic`.
        ContentStructure::Group(group @ GroupRef::Pho(inner)) => {
            match measuring_verdict(AtomicKind::Pho, domain) {
                Verdict::Enter => into(group),
                Verdict::Atomic => Descent::Atomic(AtomicUnit::Pho(inner)),
                Verdict::Excluded => Descent::Excluded,
            }
        }
        ContentStructure::Group(group @ GroupRef::Sin(inner)) => {
            match measuring_verdict(AtomicKind::Sin, domain) {
                Verdict::Enter => into(group),
                Verdict::Atomic => Descent::Atomic(AtomicUnit::Sin(inner)),
                Verdict::Excluded => Descent::Excluded,
            }
        }
        ContentStructure::Group(group @ (GroupRef::Angle(_) | GroupRef::Quotation(_))) => {
            match annotation_verdict(group.scoped_annotations(), domain) {
                Verdict::Enter => into(group),
                // An angle group or quotation is never a unit of its own; only a
                // phonological or sign group is, which is what `AtomicKind` says.
                Verdict::Atomic | Verdict::Excluded => Descent::Excluded,
            }
        }
        // Rule and rationale: `retrace_verdict`.
        ContentStructure::Retrace(retrace) => match retrace_verdict(domain) {
            Verdict::Enter => Descent::Into(Entered {
                content: &retrace.inner().content,
                // A retrace opens no code-switch scope; see `Entered`.
                scope_annotations: &[],
            }),
            Verdict::Atomic | Verdict::Excluded => Descent::Excluded,
        },
        ContentStructure::Word(_) | ContentStructure::Leaf(_) => Descent::Excluded,
    }
}

/// [`descend`] for a traversal holding a unique borrow.
///
/// Two-valued, because no `_mut` traversal counts positions: they rewrite
/// words in place. The asymmetry is safe for a better reason than "none needs
/// it today": [`ContainerMut::Retrace`] carries only its content, with no
/// annotations to reach for, so the exception that makes [`Entered::scope_in`]
/// necessary is UNCONSTRUCTIBLE on this side.
///
/// Takes the caller's `Option<ContainerMut>` so that "not a container" folds
/// here rather than at four identical call sites; a unique borrow cannot be
/// re-derived from the item, which is why the caller obtains it first.
#[inline]
pub(super) fn descend_mut<'a>(
    container: Option<ContainerMut<'a>>,
    domain: Option<TierDomain>,
) -> Option<&'a mut BracketedContent> {
    match container? {
        ContainerMut::Group(group) => {
            let (kind, annotations, content) = group.into_parts();
            let verdict = match kind {
                GroupKind::Pho => measuring_verdict(AtomicKind::Pho, domain),
                GroupKind::Sin => measuring_verdict(AtomicKind::Sin, domain),
                GroupKind::Angle | GroupKind::Quotation => annotation_verdict(annotations, domain),
            };
            match verdict {
                Verdict::Enter => Some(content),
                // A `_mut` traversal rewrites words and counts nothing, so it
                // treats both refusals alike. Matched rather than compared with
                // `== Verdict::Enter`, which a fourth variant would silently
                // read as "does not enter".
                Verdict::Atomic | Verdict::Excluded => None,
            }
        }
        ContainerMut::Retrace(content) => match retrace_verdict(domain) {
            Verdict::Enter => Some(content),
            Verdict::Atomic | Verdict::Excluded => None,
        },
    }
}
