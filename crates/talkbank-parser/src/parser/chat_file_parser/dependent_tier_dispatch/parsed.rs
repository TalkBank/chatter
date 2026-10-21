//! Attach helpers for the dependent tiers whose dedicated typed parsers need
//! `has_error` gating (`%mor` / `%gra` / `%pho` / `%mod` / `%sin` / `%wor`).
//!
//! These replace the removed raw-`&str` `match tier_kind` in the old
//! `apply_parsed_tier`: the typed dispatch in [`super::parse`] matches the
//! generated `UtteranceChild1Choice` variant and calls the matching helper
//! here, so the concrete tier wrapper (`MorDependentTierNode`, ...) arrives
//! already typed, with no `node.kind()` string dispatch. The gating and
//! placeholder policy preserves the source slot. Every admitted recovery
//! carrier reports an error, even when the native walk exposes no individually
//! reportable ERROR/MISSING node.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, GraDependentTierNode, ModDependentTierNode, MorDependentTierNode,
    PhoDependentTierNode, SinDependentTierNode, SourceBound, WorDependentTierNode,
};
use crate::model::Utterance;
use crate::model::dependent_tier::{DependentTier, DependentTierEntry};
use crate::parser::node_span::span_of;
use crate::parser::tier_parsers::admission::{TierAdmission, TierRecovery};
use crate::parser::tier_parsers::gra::{
    parse_gra_tier_with_separator, recover_gra_tier_with_separator,
};
use crate::parser::tier_parsers::mor::parse_mor_tier;
use crate::parser::tier_parsers::pho::{parse_mod_tier, parse_pho_tier};
use crate::parser::tier_parsers::sin::parse_sin_tier_with_separator;
use crate::parser::tier_parsers::wor::parse_wor_tier;
use talkbank_model::model::Terminator;
use talkbank_model::model::TierSeparator;
use talkbank_model::model::dependent_tier::{MorTier, WorTier};

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
    let tier_node = n.raw_node();
    let Ok(children) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(n),
        tier_node,
        n.source(),
        errors,
    ) else {
        return;
    };
    let Ok(separator) = crate::parser::typed_cst::report_admission(
        super::helpers::dependent_tier_separator(children.children().child_1.slot()),
        tier_node,
        n.source(),
        errors,
    ) else {
        return;
    };
    let tier_span = span_of(tier_node);
    match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => {
            report_tier_parse_error(recovery, "mor", errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Mor(empty_mor_placeholder(tier_span)),
                    separator,
                ));
        }
        TierAdmission::Healthy(healthy) => match parse_mor_tier(healthy, errors) {
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
        },
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
    let parsed = match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => recover_gra_tier_with_separator(n)
            .inspect(|_| report_tier_parse_error(recovery, "gra", errors)),
        TierAdmission::Healthy(healthy) => parse_gra_tier_with_separator(healthy.node(), errors),
    };
    let tier = match parsed {
        Ok(tier) => tier,
        Err(failure) => {
            crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
            return;
        }
    };
    utterance
        .dependent_tiers
        .push(tier.into_entry(DependentTier::Gra));
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
    let healthy = match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => {
            report_tier_parse_error(recovery, "pho", errors);
            return;
        }
        TierAdmission::Healthy(healthy) => healthy,
    };
    let n = healthy.node();
    let Ok(children) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(n),
        tier_node,
        n.source(),
        errors,
    ) else {
        return;
    };
    let Ok(separator) = crate::parser::typed_cst::report_admission(
        super::helpers::dependent_tier_separator(children.children().child_1.slot()),
        n.raw_node(),
        n.source(),
        errors,
    ) else {
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

/// Attach a `%mod` tier. Same error handling as [`attach_pho`].
pub(super) fn attach_mod<'tree>(
    n: SourceBound<'tree, '_, ModDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    let healthy = match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => {
            report_tier_parse_error(recovery, "mod", errors);
            return;
        }
        TierAdmission::Healthy(healthy) => healthy,
    };
    let n = healthy.node();
    let Ok(children) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(n),
        tier_node,
        n.source(),
        errors,
    ) else {
        return;
    };
    let Ok(separator) = crate::parser::typed_cst::report_admission(
        super::helpers::dependent_tier_separator(children.children().child_1.slot()),
        n.raw_node(),
        n.source(),
        errors,
    ) else {
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

/// Attach a `%sin` tier. Same error handling as [`attach_pho`].
pub(super) fn attach_sin<'tree>(
    n: SourceBound<'tree, '_, SinDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    let healthy = match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => {
            report_tier_parse_error(recovery, "sin", errors);
            return;
        }
        TierAdmission::Healthy(healthy) => healthy,
    };
    let n = healthy.node();
    let tier = match parse_sin_tier_with_separator(n, errors) {
        Ok(tier) => tier,
        Err(failure) => {
            crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
            return;
        }
    };
    utterance
        .dependent_tiers
        .push(tier.into_entry(DependentTier::Sin));
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
    if let Some(tier) = parse_wor_entry(n, errors) {
        utterance.dependent_tiers.push(tier.into_entry());
    }
}

/// A lowered `%wor` tier and its source separator, held as a word tier by
/// type until it is placed among an utterance's dependent tiers.
pub(crate) struct LoweredWorTier {
    tier: WorTier,
    separator: TierSeparator,
    content_range: Option<talkbank_model::model::dependent_tier::DependentTierContentSpan>,
}

impl LoweredWorTier {
    /// The lowered word tier.
    pub(crate) fn tier(&self) -> &WorTier {
        &self.tier
    }

    /// Move into a dependent-tier entry, keeping the source separator.
    pub(crate) fn into_entry(self) -> DependentTierEntry {
        let mut entry =
            DependentTierEntry::with_separator(DependentTier::Wor(self.tier), self.separator);
        if let Some(body) = self.content_range {
            entry.record_content_span(body);
        }
        entry
    }
}

/// Lower one source-bound word tier without deciding whether to retain it.
/// Recovery diagnostics stay with its owner until the disposition is selected.
pub(crate) fn parse_wor_entry<'tree>(
    n: SourceBound<'tree, '_, WorDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<LoweredWorTier> {
    let input = n.source();
    let tier_node = n.node().raw_node();
    let healthy = match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => {
            report_tier_parse_error(recovery, "wor", errors);
            return None;
        }
        TierAdmission::Healthy(healthy) => healthy,
    };
    let n = healthy.node();
    let children = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(n),
        tier_node,
        input,
        errors,
    )
    .ok()?;
    let separator = crate::parser::typed_cst::report_admission(
        super::helpers::dependent_tier_separator(children.children().child_1.slot()),
        tier_node,
        input,
        errors,
    )
    .ok()?;
    let Ok(tier) = crate::parser::typed_cst::report_admission(
        parse_wor_tier(n, errors),
        tier_node,
        input,
        errors,
    ) else {
        return None;
    };
    let Ok(content_range) = crate::parser::typed_cst::report_admission(
        super::content_range::wor(n),
        tier_node,
        input,
        errors,
    ) else {
        return None;
    };
    Some(LoweredWorTier {
        tier,
        separator,
        content_range,
    })
}

/// Consume native recovery evidence. A summary fallback describes the actual
/// tier range, never an invented missing token or guessed source prefix.
/// The whole-tier walk retains concrete nested recovery diagnostics; the
/// fallback is only for carriers whose recovery flag has no reportable child.
fn report_tier_parse_error(
    recovery: TierRecovery<'_, '_>,
    tier_name: &str,
    errors: &impl ErrorSink,
) {
    use crate::parser::tree_parsing::parser_helpers::check_for_errors_recursive_with_context;
    use talkbank_model::{DiagnosticLocation, ErrorCode, ErrorContext, ParseError, Severity};

    let tier = recovery.slice();
    let mut found = Vec::new();
    check_for_errors_recursive_with_context(tier, &mut found, Some(tier_name));
    if found.is_empty() {
        let node = tier.raw_node();
        found.push(ParseError::new(
            ErrorCode::UnparsableContent,
            Severity::Error,
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(tier.source(), span_of(node), tier.text()),
            format!("Unparsable %{tier_name} tier: tree-sitter recovered its syntax"),
        ));
    }
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
    MorTier::new_mor(
        span,
        Vec::new(),
        Terminator::Period {
            location: span.into(),
        },
    )
}
