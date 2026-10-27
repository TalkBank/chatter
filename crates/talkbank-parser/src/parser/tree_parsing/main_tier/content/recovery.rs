//! Carrier-owned recovery reporting for main-tier body regions.

use crate::error::ErrorSink;
use crate::generated_traversal::{AsRawNode, KindProof, NamedKind, PresenceProof, RangePhase};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use tree_sitter::Node;

use super::errors::{MainTierRegion, classify_main_tier_recovery};

/// Only generated carriers belonging to main-tier body syntax are admitted.
/// The carrier owns its sink and its grammar context; callers cannot pair an
/// arbitrary node slice with a different region or diagnostic label.
pub(crate) trait MainTierBodyCarrier<'tree>: sealed::Sealed {
    type Owner: NamedKind;
    fn unexpected(&self) -> impl Iterator<Item = Node<'tree>>;
}

mod sealed {
    pub trait Sealed {}
}

// Only the sink is observed, so the same reporter supports both kind readings.
macro_rules! narrowed_body_carriers {
    ($($carrier:ident => $owner:ident),* $(,)?) => {
        $(
            impl<'tree, K: KindProof, R: RangePhase<'tree>> sealed::Sealed for crate::generated_traversal::$carrier<'tree, K, R> {}
            impl<'tree, K: KindProof, R: RangePhase<'tree>> MainTierBodyCarrier<'tree> for crate::generated_traversal::$carrier<'tree, K, R>
            where R::Node: AsRawNode<'tree> {
                type Owner = crate::generated_traversal::$owner<'tree>;
                fn unexpected(&self) -> impl Iterator<Item = Node<'tree>> {
                    self.unexpected.iter().map(AsRawNode::raw_node)
                }
            }
        )*
    };
}

narrowed_body_carriers! {
    ContentsChildren => ContentsNode,
    ContentItemChildren => ContentItemNode,
    TierBodyChildren => TierBodyNode,
    TierBodyLanguageCodeChildren => TierBodyNode,
    UtteranceEndChildren => UtteranceEndNode,
    UtteranceEndChild2Children => UtteranceEndNode,
    FinalCodesChildren => FinalCodesNode,
    FinalCodesChild0Children => FinalCodesNode,
    FinalCodesChild1Children => FinalCodesNode,
}

macro_rules! presence_body_carriers {
    ($($carrier:ident => $owner:ident),* $(,)?) => {$(
        impl<'tree, K: KindProof, R: RangePhase<'tree>, P: PresenceProof> sealed::Sealed
            for crate::generated_traversal::$carrier<'tree, K, R, P> {}
        impl<'tree, K: KindProof, R: RangePhase<'tree>, P: PresenceProof> MainTierBodyCarrier<'tree>
            for crate::generated_traversal::$carrier<'tree, K, R, P>
        where R::Node: AsRawNode<'tree>
        {
            type Owner = crate::generated_traversal::$owner<'tree>;
            fn unexpected(&self) -> impl Iterator<Item = Node<'tree>> {
                self.unexpected.iter().map(AsRawNode::raw_node)
            }
        }
    )*};
}

presence_body_carriers! {
    GroupWithAnnotationsChildren => GroupWithAnnotationsNode,
    MainPhoGroupChildren => MainPhoGroupNode,
    MainSinGroupChildren => MainSinGroupNode,
}

/// Classify every displaced body child with its carrier's own context.
/// ERROR nodes use body recovery; other displaced nodes use the shared reporter.
pub(crate) fn surface_main_tier_sink<'tree, C: MainTierBodyCarrier<'tree>>(
    carrier: &C,
    source: &str,
    errors: &impl ErrorSink,
) {
    for node in carrier.unexpected() {
        if node.is_error() {
            errors.report(classify_main_tier_recovery(
                node,
                source,
                MainTierRegion::Body,
            ));
        } else {
            surface_displaced(
                std::slice::from_ref(&node),
                C::Owner::KIND.name(),
                source,
                errors,
            );
        }
    }
}
