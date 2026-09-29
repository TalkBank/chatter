//! `%mor` tier parser using tree-sitter CST.
//!
//! This module parses morphology content into `MorTier`/`MorWord` structures
//! and is used both for `%mor` tier parsing and for alignment-critical
//! utterance health checks.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

pub mod item;
pub mod tier;
pub mod word;

use crate::generated_traversal::{MorDependentTierNode, SourceBound};
use talkbank_model::ErrorSink;
use talkbank_model::ParseOutcome;
use talkbank_model::model::MorTier;

pub use tier::parse_mor_tier_inner;

/// Converts one `%mor` tier from a source-bound CST node.
///
/// Bind the generated node through its existing `ParsedSource` owner before
/// calling. Equal source bytes from another parse do not establish ownership.
///
/// Returns [`ParseOutcome::Rejected`] when the tier has any
/// unrecoverable parse failure: a malformed item, an unrecognized
/// terminator, or no terminator at all. Diagnostics are streamed via
/// the supplied [`ErrorSink`]; the caller decides per-utterance
/// whether to mark the morphology as `BlockedByMorParseFailure` (rule
/// 6e) or skip it.
pub fn parse_mor_tier<'tree>(
    node: SourceBound<'tree, '_, MorDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    parse_mor_tier_inner(node, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_traversal::{AsRawNode, SourceBindingError};
    use talkbank_model::ErrorCollector;

    #[test]
    fn morphology_tier_entry_requires_its_own_source() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/edge-cases/clitics-and-compounds.cha"
        ));
        let parser = crate::TreeSitterParser::new().expect("parser");
        let owner = parser
            .parse_source_incremental(source, None)
            .expect("owner");
        let other = parser
            .parse_source_incremental(source, None)
            .expect("independent parse");
        let mut tiers = 0;
        for node in owner.root().expect("root").descendants() {
            let Some(tier) = node
                .expect("readable reference")
                .typed::<MorDependentTierNode>()
            else {
                continue;
            };
            assert!(matches!(
                other.bind(tier.raw_node()),
                Err(SourceBindingError::ForeignTree)
            ));
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(parsed) = parse_mor_tier(tier, &errors) else {
                panic!("source-bound reference tier must parse");
            };
            assert!(!parsed.items().is_empty());
            assert!(errors.is_empty());
            tiers += 1;
        }
        assert_eq!(tiers, 4);
    }
}
