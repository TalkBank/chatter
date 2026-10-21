//! Carrier-owned recovery reporting for main-tier body regions.

use crate::error::ErrorSink;
use crate::generated_traversal::{KindProof, NamedKind, PresenceProof, Raw};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use tree_sitter::Node;

use super::errors::{MainTierRegion, classify_main_tier_recovery};

/// Only generated carriers belonging to main-tier body syntax are admitted.
/// The carrier owns its sink and its grammar context; callers cannot pair an
/// arbitrary node slice with a different region or diagnostic label.
pub(crate) trait MainTierBodyCarrier<'tree>: sealed::Sealed {
    type Owner: NamedKind;
    fn unexpected(&self) -> &[Node<'tree>];
}

mod sealed {
    pub trait Sealed {}
}

// Only the sink is observed, so the same reporter supports both kind readings.
macro_rules! narrowed_body_carriers {
    ($($carrier:ident => $owner:ident),* $(,)?) => {
        $(
            impl<K: KindProof> sealed::Sealed for crate::generated_traversal::$carrier<'_, K> {}
            impl<'tree, K: KindProof> MainTierBodyCarrier<'tree> for crate::generated_traversal::$carrier<'tree, K> {
                type Owner = crate::generated_traversal::$owner<'tree>;
                fn unexpected(&self) -> &[Node<'tree>] { &self.unexpected }
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
        impl<K: KindProof, P: PresenceProof> sealed::Sealed
            for crate::generated_traversal::$carrier<'_, K, Raw, P> {}
        impl<'tree, K: KindProof, P: PresenceProof> MainTierBodyCarrier<'tree>
            for crate::generated_traversal::$carrier<'tree, K, Raw, P>
        {
            type Owner = crate::generated_traversal::$owner<'tree>;
            fn unexpected(&self) -> &[Node<'tree>] { &self.unexpected }
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
                *node,
                source,
                MainTierRegion::Body,
            ));
        } else {
            surface_displaced(
                std::slice::from_ref(node),
                C::Owner::KIND.name(),
                source,
                errors,
            );
        }
    }
}
