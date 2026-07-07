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
    MorContentsChild0MorContentChild2Child1Choice, UtteranceEndChild0Choice,
    WorTierBodyChild2Choice,
};
use crate::model::Terminator;
use talkbank_model::Span;
use tree_sitter::Node;

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
pub(crate) trait NewTerminatorChoice<'tree>: AsRawNode<'tree> {
    /// Map this exhaustively-classified terminator subtype to its [`Terminator`]
    /// model variant, computing the source span from the concrete terminator
    /// node ([`AsRawNode::raw_node`]).
    fn to_terminator(&self) -> Terminator;
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
    ($choice_enum:ident) => {
        impl<'tree> NewTerminatorChoice<'tree> for $choice_enum<'tree> {
            fn to_terminator(&self) -> Terminator {
                let span = span_of(self.raw_node());
                match self {
                    $choice_enum::Period(_) => Terminator::Period { span },
                    $choice_enum::Question(_) => Terminator::Question { span },
                    $choice_enum::Exclamation(_) => Terminator::Exclamation { span },
                    $choice_enum::TrailingOff(_) => Terminator::TrailingOff { span },
                    $choice_enum::Interruption(_) => Terminator::Interruption { span },
                    $choice_enum::SelfInterruption(_) => Terminator::SelfInterruption { span },
                    $choice_enum::InterruptedQuestion(_) => {
                        Terminator::InterruptedQuestion { span }
                    }
                    $choice_enum::BrokenQuestion(_) => Terminator::BrokenQuestion { span },
                    $choice_enum::QuotedNewLine(_) => Terminator::QuotedNewLine { span },
                    $choice_enum::QuotedPeriodSimple(_) => Terminator::QuotedPeriodSimple { span },
                    $choice_enum::SelfInterruptedQuestion(_) => {
                        Terminator::SelfInterruptedQuestion { span }
                    }
                    $choice_enum::TrailingOffQuestion(_) => {
                        Terminator::TrailingOffQuestion { span }
                    }
                    $choice_enum::BreakForCoding(_) => Terminator::BreakForCoding { span },
                }
            }
        }
    };
}

impl_new_terminator_choice!(UtteranceEndChild0Choice);
impl_new_terminator_choice!(WorTierBodyChild2Choice);
impl_new_terminator_choice!(MorContentsChild0MorContentChild2Child1Choice);
impl_new_terminator_choice!(MorContentsChild0BreakForCodingChoice);

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
pub(crate) fn terminator_from_new_choice<'tree, C: NewTerminatorChoice<'tree>>(
    choice: &C,
) -> Terminator {
    choice.to_terminator()
}

/// Byte span of a CST node, matching the `Span::new(start as u32, end as u32)` form
/// the terminator / bullet conversion code uses.
///
/// Shared by [`terminator_from_new_choice`] (via [`NewTerminatorChoice`]) and by
/// `convert/ending.rs` (for the trailing media bullet span), so the span
/// computation exists exactly once.
pub(crate) fn span_of(node: Node<'_>) -> Span {
    Span::new(node.start_byte() as u32, node.end_byte() as u32)
}
