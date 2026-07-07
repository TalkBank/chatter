//! Attach helpers for the dependent tiers whose dedicated typed parsers need
//! `has_error` gating (`%mor` / `%gra` / `%pho` / `%mod` / `%sin` / `%wor`).
//!
//! These replace the removed raw-`&str` `match tier_kind` in the old
//! `apply_parsed_tier`: the typed dispatch in [`super::parse`] matches the
//! generated `UtteranceChild1Choice` variant and calls the matching helper
//! here, so the concrete tier wrapper (`MorDependentTierNode`, ...) arrives
//! already typed, with no `node.kind()` string dispatch. The gating and
//! placeholder behavior is byte-identical to the pre-migration code.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, GraDependentTierNode, ModDependentTierNode, MorDependentTierNode,
    PhoDependentTierNode, SinDependentTierNode, WorDependentTierNode,
};
use crate::model::Utterance;
use crate::model::dependent_tier::DependentTier;
use crate::parser::tier_parsers::gra::parse_gra_tier;
use crate::parser::tier_parsers::mor::parse_mor_tier;
use crate::parser::tier_parsers::pho::{parse_mod_tier, parse_pho_tier};
use crate::parser::tier_parsers::sin::parse_sin_tier;
use crate::parser::tier_parsers::wor::parse_wor_tier;
use talkbank_model::model::Terminator;
use talkbank_model::model::dependent_tier::{GraTier, MorTier};
use tree_sitter::Node;

/// Attach a `%mor` tier. On a tier with a tree-sitter error, report one summary
/// diagnostic and push an EMPTY placeholder (so downstream regeneration can
/// mutate the `%mor` slot in place without reordering against later tiers such
/// as `%wor`, per the parser-recovery rule); otherwise parse it, pushing the
/// same empty placeholder on a `Rejected` outcome.
pub(super) fn attach_mor(
    n: MorDependentTierNode,
    utterance: &mut Utterance,
    input: &str,
    errors: &impl ErrorSink,
) {
    let tier_node = n.raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(tier_node, input, "mor", errors);
        utterance
            .dependent_tiers
            .push(DependentTier::Mor(empty_mor_placeholder()));
    } else {
        match parse_mor_tier(tier_node, input, errors) {
            talkbank_model::ParseOutcome::Parsed(tier) => {
                utterance.dependent_tiers.push(DependentTier::Mor(tier));
            }
            talkbank_model::ParseOutcome::Rejected => {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Mor(empty_mor_placeholder()));
            }
        }
    }
}

/// Attach a `%gra` tier. On a tier with a tree-sitter error, report one summary
/// diagnostic and push an EMPTY placeholder; otherwise parse and push it.
pub(super) fn attach_gra(
    n: GraDependentTierNode,
    utterance: &mut Utterance,
    input: &str,
    errors: &impl ErrorSink,
) {
    let tier_node = n.raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(tier_node, input, "gra", errors);
        utterance
            .dependent_tiers
            .push(DependentTier::Gra(empty_gra_placeholder()));
    } else {
        let tier = parse_gra_tier(tier_node, input, errors);
        utterance.dependent_tiers.push(DependentTier::Gra(tier));
    }
}

/// Attach a `%pho` tier. On a tier with a tree-sitter error, report one summary
/// diagnostic and DROP the tier (no placeholder); otherwise parse and push it.
pub(super) fn attach_pho(
    n: PhoDependentTierNode,
    utterance: &mut Utterance,
    input: &str,
    errors: &impl ErrorSink,
) {
    let tier_node = n.raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(tier_node, input, "pho", errors);
    } else {
        let tier = parse_pho_tier(tier_node, input, errors);
        utterance.dependent_tiers.push(DependentTier::Pho(tier));
    }
}

/// Attach a `%mod` tier. Same error handling as [`attach_pho`].
pub(super) fn attach_mod(
    n: ModDependentTierNode,
    utterance: &mut Utterance,
    input: &str,
    errors: &impl ErrorSink,
) {
    let tier_node = n.raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(tier_node, input, "mod", errors);
    } else {
        let tier = parse_mod_tier(tier_node, input, errors);
        utterance.dependent_tiers.push(DependentTier::Mod(tier));
    }
}

/// Attach a `%sin` tier. Same error handling as [`attach_pho`].
pub(super) fn attach_sin(
    n: SinDependentTierNode,
    utterance: &mut Utterance,
    input: &str,
    errors: &impl ErrorSink,
) {
    let tier_node = n.raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(tier_node, input, "sin", errors);
    } else {
        let tier = parse_sin_tier(tier_node, input, errors);
        utterance.dependent_tiers.push(DependentTier::Sin(tier));
    }
}

/// Attach a `%wor` tier. `%wor` is a generated tier; on a tier with a
/// tree-sitter error (e.g. legacy CLAN groups/retraces) report one summary
/// diagnostic and DROP the tier (the validator still flags it; align
/// regenerates `%wor`); otherwise parse and push it.
pub(super) fn attach_wor(
    n: WorDependentTierNode,
    utterance: &mut Utterance,
    input: &str,
    errors: &impl ErrorSink,
) {
    let tier_node = n.raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(tier_node, input, "wor", errors);
    } else {
        let tier = parse_wor_tier(tier_node, input, errors);
        utterance.dependent_tiers.push(DependentTier::Wor(tier));
    }
}

/// Report a single summary error for a dependent tier that has parse errors.
///
/// This implements fail-fast: instead of parsing a broken tier element-by-element
/// (which cascades into many errors), we report one error and drop the tier.
fn report_tier_parse_error(tier_node: Node, input: &str, tier_name: &str, errors: &impl ErrorSink) {
    use crate::parser::tree_parsing::parser_helpers::error_analysis::analyze_dependent_tier_error_with_context;

    // Count error nodes for the summary message
    let mut cursor = tier_node.walk();
    for child in tier_node.children(&mut cursor) {
        if child.is_error() || child.is_missing() {
            errors.report(analyze_dependent_tier_error_with_context(
                child,
                input,
                Some(tier_name),
            ));
        }
    }
}

fn empty_mor_placeholder() -> MorTier {
    MorTier::new_mor(
        Vec::new(),
        Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    )
}

fn empty_gra_placeholder() -> GraTier {
    GraTier::new_gra(Vec::new())
}
