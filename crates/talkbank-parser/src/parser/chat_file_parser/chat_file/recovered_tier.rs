//! Admit document-level recovery from surviving generated CST kinds.

use crate::generated_traversal::{
    AsRawNode, ContinuationNode, GraTierPrefixNode, MorTierPrefixNode, Never, NodeSlot,
    SourceSlice, TierSepNode,
};
use crate::parser::node_span::span_of;
use talkbank_model::model::{DependentTier, DependentTierEntry, TierSeparator, UnparsedTier};

/// The observed prefix determines the shared recovery variant.
enum RecoveryKind {
    Mor,
    Gra,
}

/// Only typed CST admission can construct this trailing tier evidence.
/// A prefix, separator and ERROR extent establish raw retention, not a complete
/// dependent-tier envelope with a selected newline. Consequently this carrier
/// does not certify a body range from the ERROR node's endpoint.
pub(super) struct RecoveredTier {
    kind: RecoveryKind,
    raw: UnparsedTier,
    separator: TierSeparator,
}

impl RecoveredTier {
    pub(super) fn starts_at(&self, end: usize) -> bool {
        self.raw
            .location()
            .span()
            .is_some_and(|span| span.start() == end)
    }

    pub(super) fn into_entry(self) -> DependentTierEntry {
        let tier = match self.kind {
            RecoveryKind::Mor => DependentTier::UnparsedMor(self.raw),
            RecoveryKind::Gra => DependentTier::UnparsedGra(self.raw),
        };
        DependentTierEntry::with_separator(tier, self.separator)
    }
}

/// Recognize one rejected logical tier without parsing its ERROR text.
/// Every internal line break must belong to an observed continuation node;
/// a region spanning independent lines cannot be admitted as one tier.
pub(super) fn read(
    source: SourceSlice<'_, '_>,
) -> Result<Option<RecoveredTier>, crate::CstFailure> {
    let mut prefix = None;
    let mut separator = None;
    let mut continuations = Vec::new();
    for child in source.descendants() {
        let child = child?;
        let observed_prefix = if let Some(node) = child.typed::<MorTierPrefixNode>() {
            Some((RecoveryKind::Mor, node.raw_node()))
        } else {
            child
                .typed::<GraTierPrefixNode>()
                .map(|node| (RecoveryKind::Gra, node.raw_node()))
        };
        if let Some((kind, node)) = observed_prefix
            && !node.is_missing()
            && node.start_byte() == source.raw_node().start_byte()
        {
            prefix = Some((kind, node.end_byte()));
        }
        if let Some(node) = child.typed::<TierSepNode>()
            && prefix
                .as_ref()
                .is_some_and(|(_, end)| *end == node.raw_node().start_byte())
            && !node.raw_node().is_missing()
            && !node.raw_node().has_error()
        {
            separator = Some(crate::parser::tier_parsers::dependent_tier_separator::<
                Never,
                Never,
                Never,
            >(&NodeSlot::Present(node.node()))?);
        }
        if let Some(node) = child.typed::<ContinuationNode>()
            && !node.raw_node().is_missing()
            && !node.raw_node().has_error()
        {
            continuations.push(node.raw_node().byte_range());
        }
    }
    let (Some((kind, _)), Some(separator)) = (prefix, separator) else {
        return Ok(None);
    };
    for (offset, byte) in source
        .text()
        .trim_end_matches(['\r', '\n'])
        .bytes()
        .enumerate()
    {
        if matches!(byte, b'\r' | b'\n')
            && !continuations
                .iter()
                .any(|range| range.contains(&(source.raw_node().start_byte() + offset)))
        {
            return Ok(None);
        }
    }
    Ok(Some(RecoveredTier {
        kind,
        raw: UnparsedTier::new(span_of(source.raw_node()), source.text()),
        separator,
    }))
}
