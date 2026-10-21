//! Mapping from the typed CST terminator supertype to `model::Terminator`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#BreakForCoding>
//! - <https://talkbank.org/0info/manuals/CHAT.html#BrokenQuestion_Terminator>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>

use crate::generated_traversal::{
    AsRawNode, MorContentsChild0BreakForCodingChoice,
    MorContentsChild0MorContentChild2Child1Choice, Ranged, Raw, SourceSlice, TerminatorChoice,
    UtteranceEndChild0Choice, WorTierBodyChild2Choice,
};
use crate::model::Terminator;

/// A NEW-backend `terminator` supertype choice, unified across the distinct
/// per-embedding-position enum names the generator mangles for it.
///
/// The NEW-backend generator emits a SEPARATE choice enum for every place the
/// `terminator` supertype is referenced ([`UtteranceEndChild0Choice`] inside
/// `utterance_end`, [`WorTierBodyChild2Choice`] inside `wor_tier_body`,
/// [`MorContentsChild0MorContentChild2Child1Choice`] inside the optional
/// trailing `(whitespaces, terminator)` group of `mor_contents`'s items
/// alternative, and [`MorContentsChild0BreakForCodingChoice`] for
/// `mor_contents`'s bare-terminator alternative), even though each is
/// variant-for-variant identical (the same 13 terminator subtypes). This local
/// trait unifies them so the single [`terminator_from_new_choice`] maps ANY of
/// them to the [`Terminator`] model with one shared 13-arm mapping. It is the
/// resolution the earlier `terminator_from_choice` / `terminator_from_new_choice`
/// split explicitly deferred to chatter visitor-migration Task B4 ("cannot
/// share one signature until B4 determines what type `wor.rs`'s own migrated
/// call site receives"): `wor.rs` receives [`WorTierBodyChild2Choice`], and
/// Task C's `tier_parsers/mor/tier.rs` receives the two `mor_contents` choice
/// types above, so all embeddings now flow through this one path and the
/// OLD-backend `terminator_from_choice` is retired.
pub(crate) trait NewTerminatorChoice {
    /// Map this exhaustively-classified terminator subtype to its [`Terminator`]
    /// model variant, computing the source span from the concrete terminator
    /// node ([`AsRawNode::raw_node`]).
    fn to_terminator(&self) -> Terminator;
}

/// Checked source projection exists only after generated range admission.
pub(crate) trait RangedTerminatorChoice<'tree, 'source>: NewTerminatorChoice {
    fn source_slice(&self) -> SourceSlice<'tree, 'source>;
}

/// Implement [`NewTerminatorChoice`] for one generated terminator-choice enum.
///
/// Every NEW-backend terminator choice enum is variant-for-variant identical, so
/// this single 13-arm mapping is the ONE source of truth shared across all of
/// them (currently [`UtteranceEndChild0Choice`], [`WorTierBodyChild2Choice`],
/// [`MorContentsChild0MorContentChild2Child1Choice`], and
/// [`MorContentsChild0BreakForCodingChoice`]).
/// The match is exhaustive with NO `_` catch-all, so a future terminator subtype
/// is a compile error in EACH expansion (compiler-enforced instead of routed
/// through a `node.kind()` string dispatch, the point of the visitor migration).
macro_rules! impl_new_terminator_choice {
    ($choice_enum:ident, $phase:ident) => {
        impl_new_terminator_choice!(@variants $choice_enum, $phase;
            Period, Question, Exclamation, TrailingOff, Interruption,
            SelfInterruption, InterruptedQuestion, BrokenQuestion, QuotedNewLine,
            QuotedPeriodSimple, SelfInterruptedQuestion, TrailingOffQuestion,
            BreakForCoding
        );
    };
    (@variants $choice_enum:ident, Raw; $($variant:ident),+) => {
        impl_new_terminator_choice!(@phase $choice_enum, ['tree], Raw; $($variant),+);
    };
    (@variants $choice_enum:ident, Ranged; $($variant:ident),+) => {
        impl_new_terminator_choice!(@phase $choice_enum, ['tree, 'source], Ranged<'source>; $($variant),+);
    };
    (@variants $choice_enum:ident, RangedWithSource; $($variant:ident),+) => {
        impl_new_terminator_choice!(@variants $choice_enum, Ranged; $($variant),+);
        impl<'tree, 'source> RangedTerminatorChoice<'tree, 'source>
            for $choice_enum<'tree, Ranged<'source>>
        {
            fn source_slice(&self) -> SourceSlice<'tree, 'source> {
                match self {
                    $($choice_enum::$variant(node) => node.source_slice(),)+
                }
            }
        }
    };
    (@phase $choice_enum:ident, [$($lifetime:lifetime),+], $phase:ty; $($variant:ident),+) => {
        impl<$($lifetime),+> NewTerminatorChoice for $choice_enum<'tree, $phase> {
            fn to_terminator(&self) -> Terminator {
                match self {
                    $($choice_enum::$variant(node) => Terminator::$variant {
                        location: span_of(node.raw_node()).into(),
                    },)+
                }
            }
        }
    };
}

// Emit only the phases actually supplied by each embedding's producer. The
// lexical MOR choice alone needs checked projection for its missing slot.
impl_new_terminator_choice!(UtteranceEndChild0Choice, Raw);
impl_new_terminator_choice!(TerminatorChoice, Raw);
impl_new_terminator_choice!(WorTierBodyChild2Choice, Raw);
impl_new_terminator_choice!(
    MorContentsChild0MorContentChild2Child1Choice,
    RangedWithSource
);
impl_new_terminator_choice!(MorContentsChild0BreakForCodingChoice, Ranged);

/// Map any NEW-backend terminator supertype choice (see [`NewTerminatorChoice`])
/// to its [`Terminator`] model variant.
///
/// Used by the migrated `convert/ending.rs` (the `utterance_end` terminator,
/// [`UtteranceEndChild0Choice`]), `tier_parsers/wor.rs` (the `%wor` tier-body
/// terminator, [`WorTierBodyChild2Choice`]), and `tier_parsers/mor/tier.rs`
/// (the `%mor` tier's terminator, either
/// [`MorContentsChild0MorContentChild2Child1Choice`] trailing the items
/// alternative or [`MorContentsChild0BreakForCodingChoice`] standing alone);
/// all share the single mapping in [`impl_new_terminator_choice`].
pub(crate) fn terminator_from_new_choice<C: NewTerminatorChoice>(choice: &C) -> Terminator {
    choice.to_terminator()
}

/// Byte span of a CST node.
///
/// Re-exported here because [`terminator_from_new_choice`] and
/// `convert/ending.rs` both reach for it under this path; the conversion itself
/// lives in [`crate::parser::node_span`], which is the single home for it.
pub(crate) use crate::parser::node_span::span_of;
