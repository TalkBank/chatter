//! Alignment-domain counting/extraction over main-tier content trees.
//!
//! Every function here takes a [`PositionalDomain`], which has no `%wor`:
//! the `%wor` count and pairing belong to `WorMainTierProjection`, and until
//! 2026-09-08 two `Wor` arms in this file computed the count a second time.
//!
//! One traversal, two sinks: [`count_tier_positions`] counts what
//! [`collect_tier_items`] collects, over the same walk, so the two cannot
//! disagree. Until 2026-09-08 the file carried a counting traversal and an
//! extracting traversal side by side, each with its own arms over the two
//! content enums, held equal by a test.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>
// Every match over the content enums in this file is exhaustive, so the lint
// costs nothing today and makes it stay that way: a new `UtteranceContent` or
// `BracketedItem` variant becomes a COMPILE ERROR here rather than a silent
// `_ =>` that answers wrong. Four such catch-alls have already shipped as
// defects; see `talkbank-parser-tests/src/content_catch_alls.rs`.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::model::{
    Action, Annotated, BracketedContent, BracketedItem, ContentAnnotation, Pause, ReplacedWord,
    Separator, UtteranceContent, Word,
};

use super::descent::{AtomicUnit, Descent, descend, excluded_by_annotations};
use super::domain::PositionalDomain;
use super::rules::{
    counts_for_tier, is_tag_marker_separator, should_align_replaced_word_in_pho_sin,
};
use super::to_chat_display_string as to_string;

/// One extracted alignable item shown in mismatch diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct TierPosition {
    /// Display text for this item (e.g., "hello", "&-um", ".")
    pub text: String,
    /// Optional description for complex items (e.g., "[/]" for retracing)
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Typed counts for cross-tier alignment
//
// These newtypes exist so a pipeline can't accidentally confuse "number of
// Mor-alignable words on the main tier" with "number of %mor items on the
// dependent tier". They agree on an Aligned utterance and disagree on a
// MisalignmentBug, both are interesting facts the type system must let us
// state without primitive obsession.
//
// The types are intentionally minimal: they expose only `get()` and basic
// derives. Arithmetic, serialization, and display formatting happen at the
// call site with explicit unwrapping, because any time you're doing math
// on these you are likely crossing a semantic boundary that deserves a
// comment.
// ---------------------------------------------------------------------------

/// Count of CHAT main-tier words alignable to the `%mor` dependent tier.
///
/// Produced by [`Utterance::mor_alignable_word_count`](crate::model::Utterance::mor_alignable_word_count)
/// applying the rules in `alignment/helpers/rules.rs::counts_for_tier`.
/// This is the **expected** size of the `%mor` tier on this utterance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MorAlignableWordCount(usize);

impl MorAlignableWordCount {
    /// Wrap a raw count.
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Inner count.
    pub fn get(self) -> usize {
        self.0
    }

    /// `true` iff this utterance has no Mor-alignable content, i.e.,
    /// `%mor` should not be produced at all (NotApplicable).
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for MorAlignableWordCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Count of items on a `%mor` dependent tier.
///
/// Produced by `mor_tier.items.len()` when the tier is present. When
/// paired with a [`MorAlignableWordCount`], the two either agree
/// (Aligned) or disagree (MisalignmentBug); the types prevent you from
/// swapping them in the comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MorItemCount(usize);

impl MorItemCount {
    /// Wrap a raw count.
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Inner count.
    pub fn get(self) -> usize {
        self.0
    }

    /// `true` iff the tier is empty (or was absent, in which case we
    /// represent the absence as `None` at the call site, do not
    /// substitute `MorItemCount::new(0)` for a missing tier).
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for MorItemCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Extract alignable items with their display text for a given alignment domain.
///
/// The returned sequence matches alignment traversal order and is used to build
/// human-readable mismatch diagnostics (`main` vs dependent-tier views).
pub fn collect_tier_items(
    content: &[UtteranceContent],
    domain: PositionalDomain,
) -> Vec<TierPosition> {
    let mut items = Vec::new();
    for item in content {
        walk_alignable_item(item, domain, &mut |position| {
            items.push(position.into_tier_position());
        });
    }
    items
}

/// Count alignable units for a given alignment domain.
///
/// This is the fast path for preflight length checks before building richer
/// positional mismatch details: the walk of [`collect_tier_items`] with a
/// counter for a sink.
pub fn count_tier_positions(content: &[UtteranceContent], domain: PositionalDomain) -> usize {
    count_items(content.iter(), domain)
}

/// Count alignable content up to (but not including) a specific index.
///
/// This is useful for LSP hover features where you need to know how many
/// alignable items precede a given position in the content array.
/// The result uses the same domain-specific inclusion rules as full alignment.
///
/// # Parameters
/// - `content`: The utterance content to count
/// - `max_index`: Only count items before this index (exclusive)
/// - `domain`: The alignment domain (Mor, Pho, or Sin)
///
/// # Returns
/// The count of alignable items in `content[0..max_index]`
///
/// # Examples
/// ```
/// use talkbank_model::alignment::{count_tier_positions_until, PositionalDomain};
/// use talkbank_model::model::{UtteranceContent, Word};
///
/// let content = vec![
///     UtteranceContent::Word(Box::new(Word::simple("hello"))),
///     UtteranceContent::Word(Box::new(Word::simple("world"))),
/// ];
///
/// // Count items before index 1 (only first word)
/// let count = count_tier_positions_until(&content, 1, PositionalDomain::Mor);
/// assert_eq!(count, 1);
///
/// // Count items before index 2 (both words)
/// let count = count_tier_positions_until(&content, 2, PositionalDomain::Mor);
/// assert_eq!(count, 2);
/// ```
pub fn count_tier_positions_until(
    content: &[UtteranceContent],
    max_index: usize,
    domain: PositionalDomain,
) -> usize {
    count_items(content.iter().take(max_index), domain)
}

fn count_items<'a>(
    items: impl Iterator<Item = &'a UtteranceContent>,
    domain: PositionalDomain,
) -> usize {
    let mut count = 0usize;
    for item in items {
        walk_alignable_item(item, domain, &mut |_| count += 1);
    }
    count
}

/// One position a dependent tier aligns to, as the walk meets it.
///
/// A count needs only that it was met; a diagnostic needs its text, which
/// [`Self::into_tier_position`] renders. The two consumers share the walk.
enum AlignablePosition<'a> {
    /// A word, or the replacement or original a replaced word contributes.
    Word(&'a Word),
    /// A container that IS one position in this tier (a phonological group
    /// under `%pho`, a sign group under `%sin`).
    Atomic(AtomicUnit<'a>),
    /// A tag-marker separator, which `%mor` aligns.
    Separator(&'a Separator),
    /// A pause, at the top level or inside a group, which `%pho` aligns.
    Pause(&'a Pause),
    /// A top-level action, which `%sin` aligns.
    Action(&'a Action),
    /// A top-level annotated action, rendered with its annotations.
    AnnotatedAction(&'a Annotated<Action>),
}

impl AlignablePosition<'_> {
    fn into_tier_position(self) -> TierPosition {
        match self {
            Self::Word(word) => TierPosition {
                text: to_string(word),
                description: None,
            },
            Self::Atomic(unit) => TierPosition {
                text: unit.display_text(),
                description: Some(unit.description().to_string()),
            },
            Self::Separator(sep) => TierPosition {
                text: to_string(sep),
                description: None,
            },
            Self::Pause(pause) => TierPosition {
                text: to_string(pause),
                description: Some("pause".to_string()),
            },
            Self::Action(action) => TierPosition {
                text: to_string(action),
                description: Some("action".to_string()),
            },
            Self::AnnotatedAction(action) => TierPosition {
                text: to_string(action),
                description: Some("action".to_string()),
            },
        }
    }
}

/// Walks one main-tier item, handing every alignable position in `domain`
/// to `sink`, in document order.
fn walk_alignable_item<'a>(
    item: &'a UtteranceContent,
    domain: PositionalDomain,
    sink: &mut impl FnMut(AlignablePosition<'a>),
) {
    match item {
        UtteranceContent::Word(word) => walk_alignable_word(word, &[], domain, sink),
        UtteranceContent::AnnotatedWord(annotated) => {
            walk_alignable_word(
                &annotated.inner,
                &annotated.scoped_annotations,
                domain,
                sink,
            );
        }
        UtteranceContent::ReplacedWord(replaced) => {
            walk_alignable_replaced_word(replaced, domain, sink);
        }
        // Containers: ONE arm. `helpers::descent` owns the rule, and its
        // three-valued answer is what a count needs and a walker discards:
        // `Atomic` is a container that IS one position in this tier, and its
        // payload carries what describes the position.
        UtteranceContent::Group(_)
        | UtteranceContent::AnnotatedGroup(_)
        | UtteranceContent::PhoGroup(_)
        | UtteranceContent::SinGroup(_)
        | UtteranceContent::Quotation(_)
        | UtteranceContent::AnnotatedQuotation(_)
        | UtteranceContent::Retrace(_)
        | UtteranceContent::AnnotatedRetrace(_) => {
            match descend(item.structure(), Some(domain.into())) {
                Descent::Into(entered) => {
                    walk_alignable_bracketed(entered.content(), domain, sink);
                }
                Descent::Atomic(unit) => sink(AlignablePosition::Atomic(unit)),
                Descent::Excluded => {}
            }
        }
        UtteranceContent::Separator(sep) => {
            if domain == PositionalDomain::Mor && is_tag_marker_separator(sep) {
                sink(AlignablePosition::Separator(sep));
            }
        }
        UtteranceContent::Pause(pause) => {
            // Pauses are phonological events that get transcribed in %pho tiers
            // but NOT in %wor tiers (which only contain actual words)
            // %mor and %sin also don't align to pauses
            if domain == PositionalDomain::Pho {
                sink(AlignablePosition::Pause(pause));
            }
        }
        UtteranceContent::Action(action) => {
            if domain == PositionalDomain::Sin {
                sink(AlignablePosition::Action(action));
            }
        }
        UtteranceContent::AnnotatedAction(action) => {
            if domain == PositionalDomain::Sin {
                sink(AlignablePosition::AnnotatedAction(action));
            }
        }
        // All remaining variants are non-alignable for every dependent tier:
        // events, markers, formatting, freecodes, overlap points, internal bullets.
        UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
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

/// Walks bracketed content recursively, depth-first, in document order.
fn walk_alignable_bracketed<'a>(
    content: &'a BracketedContent,
    domain: PositionalDomain,
    sink: &mut impl FnMut(AlignablePosition<'a>),
) {
    for item in &content.content {
        walk_alignable_bracketed_item(item, domain, sink);
    }
}

/// Walks one bracketed item. Mirrors the top-level rules for bracket-scoped
/// structures, with the action difference stated below (a pause mirrors the
/// top level since 2026-09-09).
fn walk_alignable_bracketed_item<'a>(
    item: &'a BracketedItem,
    domain: PositionalDomain,
    sink: &mut impl FnMut(AlignablePosition<'a>),
) {
    match item {
        BracketedItem::Word(word) => walk_alignable_word(word, &[], domain, sink),
        BracketedItem::AnnotatedWord(annotated) => {
            walk_alignable_word(
                &annotated.inner,
                &annotated.scoped_annotations,
                domain,
                sink,
            );
        }
        BracketedItem::ReplacedWord(replaced) => {
            walk_alignable_replaced_word(replaced, domain, sink);
        }
        // Containers: ONE arm, through `descent`, as at the top level.
        BracketedItem::Group(_)
        | BracketedItem::AnnotatedGroup(_)
        | BracketedItem::PhoGroup(_)
        | BracketedItem::SinGroup(_)
        | BracketedItem::Quotation(_)
        | BracketedItem::AnnotatedQuotation(_)
        | BracketedItem::Retrace(_)
        | BracketedItem::AnnotatedRetrace(_) => {
            match descend(item.structure(), Some(domain.into())) {
                Descent::Into(entered) => {
                    walk_alignable_bracketed(entered.content(), domain, sink);
                }
                Descent::Atomic(unit) => sink(AlignablePosition::Atomic(unit)),
                Descent::Excluded => {}
            }
        }
        BracketedItem::Separator(sep) => {
            if domain == PositionalDomain::Mor && is_tag_marker_separator(sep) {
                sink(AlignablePosition::Separator(sep));
            }
        }
        // A pause inside bracketed content is a phonological token exactly as
        // a top-level one is (see the `UtteranceContent::Pause` arm): the
        // phonological tiers carry the pause at the same slot wherever it
        // stands on the main tier.
        //
        // Until 2026-09-09 this arm yielded nothing, deliberately and
        // UNVERIFIED: the corpus then held one bracketed instance
        // (phon-other-data/Clinical/Cattini/SI/SI-COI1.cha), a data defect that
        // dropped the pause from `%mod` and `%pho` while keeping it in
        // `%xmodsyl`, so it was evidence for neither value, and the comment
        // here said to wait for Phon's updated files. They came: the Phon
        // team's French corpora carry a pause inside an overlap or retrace
        // group with `%mod`/`%pho` carrying it at the same slot, at scale (72
        // records in 49 of 327 Lyon sessions, TalkBank/chatter#5; the Phon
        // team's PhonBank review of 2026-09-09 lists Paris, Utrecht and
        // Providence with the same fault, 41, 14 and 2 sessions), and every
        // one was E715 and E734 one token long under the old arm. The top-level convention
        // (208 files clean with a pause token in every phonological tier)
        // extends inside a group, as the note predicted it would.
        BracketedItem::Pause(pause) => {
            if domain == PositionalDomain::Pho {
                sink(AlignablePosition::Pause(pause));
            }
        }
        // All remaining variants produce no alignable items inside bracketed
        // content: actions, events, markers, formatting, freecodes, overlap points.
        BracketedItem::Action(_)
        | BracketedItem::AnnotatedAction(_)
        | BracketedItem::Event(_)
        | BracketedItem::AnnotatedEvent(_)
        | BracketedItem::Freecode(_)
        | BracketedItem::OverlapPoint(_)
        | BracketedItem::InternalBullet(_)
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

/// A word position, unless its own scoped annotations exclude it from the
/// domain or the membership rule does.
fn walk_alignable_word<'a>(
    word: &'a Word,
    annotations: &[ContentAnnotation],
    domain: PositionalDomain,
    sink: &mut impl FnMut(AlignablePosition<'a>),
) {
    // A word carries its own scoped annotations exactly as a group does, and
    // the exclusion question is the same one, so it asks the same owner.
    if excluded_by_annotations(annotations, Some(domain.into())) {
        return;
    }
    if !counts_for_tier(word, domain.into()) {
        return;
    }
    sink(AlignablePosition::Word(word));
}

/// A replaced word's positions after replacement/retrace rules.
///
/// `%mor` aligns to the replacement words when present, because morphology
/// follows the corrected transcript slot; `%pho` and `%sin` align to the
/// original word (what was actually spoken or produced), at most once
/// whatever the replacement holds.
fn walk_alignable_replaced_word<'a>(
    entry: &'a ReplacedWord,
    domain: PositionalDomain,
    sink: &mut impl FnMut(AlignablePosition<'a>),
) {
    if excluded_by_annotations(&entry.scoped_annotations, Some(domain.into())) {
        return;
    }
    match domain {
        PositionalDomain::Mor => {
            if !entry.replacement.words.is_empty() {
                for word in &entry.replacement.words {
                    if counts_for_tier(word, domain.into()) {
                        sink(AlignablePosition::Word(word));
                    }
                }
            } else if counts_for_tier(&entry.word, domain.into()) {
                sink(AlignablePosition::Word(&entry.word));
            }
        }
        PositionalDomain::Pho | PositionalDomain::Sin => {
            if should_align_replaced_word_in_pho_sin(entry) {
                sink(AlignablePosition::Word(&entry.word));
            }
        }
    }
}
