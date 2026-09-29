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
    PhoDependentTierNode, SinDependentTierNode, SourceBound, SourceSlice, WorDependentTierNode,
    extract_wor_dependent_tier,
};
use crate::model::Utterance;
use crate::model::dependent_tier::{DependentTier, DependentTierEntry};
use crate::parser::node_span::span_of;
use crate::parser::tier_parsers::gra::parse_gra_tier;
use crate::parser::tier_parsers::mor::parse_mor_tier;
use crate::parser::tier_parsers::pho::{parse_mod_tier, parse_pho_tier};
use crate::parser::tier_parsers::sin::parse_sin_tier;
use crate::parser::tier_parsers::wor::parse_wor_tier;
use talkbank_model::model::Terminator;
use talkbank_model::model::dependent_tier::{GraTier, MorTier};

/// Attach a `%mor` tier. On a tier with a tree-sitter error, report one summary
/// diagnostic and push an EMPTY placeholder (so downstream regeneration can
/// mutate the `%mor` slot in place without reordering against later tiers such
/// as `%wor`, per the parser-recovery rule); otherwise parse it, pushing the
/// same empty placeholder on a `Rejected` outcome.
pub(super) fn attach_mor<'tree>(
    n: SourceBound<'tree, '_, MorDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    let separator = crate::parser::typed_cst::report_reconstruction(
        n.extract().and_then(|children| {
            super::helpers::dependent_tier_separator(children.children().child_1.slot())
        }),
        tier_node,
        input,
        errors,
    );
    let Ok(separator) = separator else {
        return;
    };
    let tier_span = span_of(tier_node);
    if tier_node.has_error() {
        report_tier_parse_error(n.source_slice(), "mor", errors);
        utterance
            .dependent_tiers
            .push(DependentTierEntry::with_separator(
                DependentTier::Mor(empty_mor_placeholder(tier_span)),
                separator,
            ));
    } else {
        match parse_mor_tier(n, errors) {
            talkbank_model::ParseOutcome::Parsed(tier) => {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Mor(tier),
                        separator,
                    ));
            }
            talkbank_model::ParseOutcome::Rejected => {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Mor(empty_mor_placeholder(tier_span)),
                        separator,
                    ));
            }
        }
    }
}

/// Attach a `%gra` tier. On a tier with a tree-sitter error, report one summary
/// diagnostic and push an EMPTY placeholder; otherwise parse and push it.
pub(super) fn attach_gra<'tree>(
    n: SourceBound<'tree, '_, GraDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    let separator = crate::parser::typed_cst::report_reconstruction(
        n.extract().and_then(|children| {
            super::helpers::dependent_tier_separator(children.children().child_1.slot())
        }),
        tier_node,
        input,
        errors,
    );
    let Ok(separator) = separator else {
        return;
    };
    let tier_span = span_of(tier_node);
    if tier_node.has_error() {
        report_tier_parse_error(n.source_slice(), "gra", errors);
        utterance
            .dependent_tiers
            .push(DependentTierEntry::with_separator(
                DependentTier::Gra(empty_gra_placeholder(tier_span)),
                separator,
            ));
    } else {
        let tier = match parse_gra_tier(n, errors) {
            Ok(tier) => tier,
            Err(failure) => {
                crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
                return;
            }
        };
        utterance
            .dependent_tiers
            .push(DependentTierEntry::with_separator(
                DependentTier::Gra(tier),
                separator,
            ));
    }
}

/// Attach a `%pho` tier. On a tier with a tree-sitter error, report one summary
/// diagnostic and DROP the tier (no placeholder); otherwise parse and push it.
pub(super) fn attach_pho<'tree>(
    n: SourceBound<'tree, '_, PhoDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(n.source_slice(), "pho", errors);
    } else {
        let separator = crate::parser::typed_cst::report_reconstruction(
            n.extract().and_then(|children| {
                super::helpers::dependent_tier_separator(children.children().child_1.slot())
            }),
            tier_node,
            input,
            errors,
        );
        let Ok(separator) = separator else {
            return;
        };
        let tier = match parse_pho_tier(n, errors) {
            Ok(tier) => tier,
            Err(failure) => {
                crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
                return;
            }
        };
        utterance
            .dependent_tiers
            .push(DependentTierEntry::with_separator(
                DependentTier::Pho(tier),
                separator,
            ));
    }
}

/// Attach a `%mod` tier. Same error handling as [`attach_pho`].
pub(super) fn attach_mod<'tree>(
    n: SourceBound<'tree, '_, ModDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(n.source_slice(), "mod", errors);
    } else {
        let separator = crate::parser::typed_cst::report_reconstruction(
            n.extract().and_then(|children| {
                super::helpers::dependent_tier_separator(children.children().child_1.slot())
            }),
            tier_node,
            input,
            errors,
        );
        let Ok(separator) = separator else {
            return;
        };
        let tier = match parse_mod_tier(n, errors) {
            Ok(tier) => tier,
            Err(failure) => {
                crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
                return;
            }
        };
        utterance
            .dependent_tiers
            .push(DependentTierEntry::with_separator(
                DependentTier::Mod(tier),
                separator,
            ));
    }
}

/// Attach a `%sin` tier. Same error handling as [`attach_pho`].
pub(super) fn attach_sin<'tree>(
    n: SourceBound<'tree, '_, SinDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(n.source_slice(), "sin", errors);
    } else {
        let separator = crate::parser::typed_cst::report_reconstruction(
            n.extract().and_then(|children| {
                super::helpers::dependent_tier_separator(children.children().child_1.slot())
            }),
            tier_node,
            input,
            errors,
        );
        let Ok(separator) = separator else {
            return;
        };
        let tier = match parse_sin_tier(n, errors) {
            Ok(tier) => tier,
            Err(failure) => {
                crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
                return;
            }
        };
        utterance
            .dependent_tiers
            .push(DependentTierEntry::with_separator(
                DependentTier::Sin(tier),
                separator,
            ));
    }
}

/// Attach a `%wor` tier. `%wor` is a generated tier; on a tier with a
/// tree-sitter error (e.g. legacy CLAN groups/retraces) report one summary
/// diagnostic and DROP the tier (the validator still flags it; align
/// regenerates `%wor`); otherwise parse and push it.
pub(super) fn attach_wor<'tree>(
    n: SourceBound<'tree, '_, WorDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.node().raw_node();
    if tier_node.has_error() {
        report_tier_parse_error(n.source_slice(), "wor", errors);
    } else {
        let separator = crate::parser::typed_cst::report_reconstruction(
            extract_wor_dependent_tier(n.node()).and_then(|children| {
                super::helpers::dependent_tier_separator(children.child_1.slot())
            }),
            tier_node,
            input,
            errors,
        );
        let Ok(separator) = separator else {
            return;
        };
        let Ok(tier) = crate::parser::typed_cst::report_reconstruction(
            parse_wor_tier(n, errors),
            tier_node,
            input,
            errors,
        ) else {
            return;
        };
        utterance
            .dependent_tiers
            .push(DependentTierEntry::with_separator(
                DependentTier::Wor(tier),
                separator,
            ));
    }
}

/// Report every recovery node in a dependent tier that has parse errors, in
/// the tier's own words, instead of parsing the broken tier element by
/// element (which cascades into many errors); the tier is then dropped.
///
/// The walk is the whole tier, not its direct children: a `%mor` word with
/// an empty part of speech is an ERROR two levels down, and the diagnostic
/// it produces here is what marks the tier's alignment domain tainted. Until
/// 2026-09-08 the utterance parser walked every tier for that reason before
/// attaching it, and the two reports met at the same span.
fn report_tier_parse_error(tier: SourceSlice<'_, '_>, tier_name: &str, errors: &impl ErrorSink) {
    use crate::parser::tree_parsing::parser_helpers::check_for_errors_recursive_with_context;

    let mut found = Vec::new();
    check_for_errors_recursive_with_context(tier, &mut found, Some(tier_name));
    errors.report_all(found);
}

/// Empty `%mor` standing in for a tier that failed to parse.
///
/// The span is the REAL tier span, not `Span::DUMMY`. A placeholder is still a
/// tier that exists at a place in the file, and everything downstream reads its
/// span: a diagnostic's reported location, an LSP position, `SpanShift` during
/// an edit. `DUMMY` is `0..0`, so all of those pointed at the start of the file
/// instead of at the offending tier.
///
/// This is design rule 7: lenient recovery preserves malformed tier slots IN
/// PLACE and never fabricates dummy model values.
fn empty_mor_placeholder(span: talkbank_model::Span) -> MorTier {
    MorTier::new_mor(Vec::new(), Terminator::Period { span }).with_span(span)
}

/// Empty `%gra` standing in for a tier that failed to parse. See
/// [`empty_mor_placeholder`] for why the span must be real.
fn empty_gra_placeholder(span: talkbank_model::Span) -> GraTier {
    GraTier::new_gra(Vec::new()).with_span(span)
}
