//! Attach helpers for the dependent tiers whose dedicated typed parsers need
//! `has_error` gating (`%mor` / `%gra` / `%pho` / `%mod` / `%sin` / `%wor`).
//!
//! These replace the removed raw-`&str` `match tier_kind` in the old
//! `apply_parsed_tier`: the typed dispatch in [`super::parse`] matches the
//! generated `UtteranceChild1Choice` variant and calls the matching helper
//! here, so the concrete tier wrapper (`MorDependentTierNode`, ...) arrives
//! already typed, with no `node.kind()` string dispatch. The gating and
//! recovery policy preserves the source slot. Every admitted recovery
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
use crate::parser::tier_parsers::gra::{gra_recovery_separator, parse_gra_tier_with_separator};
use crate::parser::tier_parsers::mor::parse_mor_tier;
use crate::parser::tier_parsers::pho::{parse_mod_tier, parse_pho_tier};
use crate::parser::tier_parsers::sin::parse_sin_tier;
use crate::parser::tier_parsers::wor::parse_wor_tier;
use talkbank_model::model::TierSeparator;
use talkbank_model::model::dependent_tier::{UnparsedTier, WorTier};

/// Attach parsed morphology or retain the original unparsed source in place.
/// Recovery is never represented by an empty tier or an invented terminator.
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
    let talkbank_model::ParseOutcome::Parsed(source) =
        crate::parser::tree_parsing::parser_helpers::extract_utf8_text(
            tier_node,
            n.source(),
            errors,
            "unparsed MOR source",
        )
    else {
        return;
    };
    match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => {
            report_tier_parse_error(recovery, "mor", errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::UnparsedMor(UnparsedTier::new(tier_span, source)),
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
                        DependentTier::UnparsedMor(UnparsedTier::new(tier_span, source)),
                        separator,
                    ));
            }
        },
    }
}

/// Attach a complete graph or retain the rejected tier's exact source.
pub(super) fn attach_gra<'tree>(
    n: SourceBound<'tree, '_, GraDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    let talkbank_model::ParseOutcome::Parsed(source) =
        crate::parser::tree_parsing::parser_helpers::extract_utf8_text(
            tier_node,
            input,
            errors,
            "unparsed GRA source",
        )
    else {
        return;
    };
    let raw = || DependentTier::UnparsedGra(UnparsedTier::new(span_of(tier_node), source));
    let parsed = match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => gra_recovery_separator(n).map(|separator| {
            report_tier_parse_error(recovery, "gra", errors);
            DependentTierEntry::with_separator(raw(), separator)
        }),
        TierAdmission::Healthy(healthy) => parse_gra_tier_with_separator(healthy.node(), errors)
            .map(|tier| {
                tier.into_entry(|gra| match gra {
                    talkbank_model::ParseOutcome::Parsed(gra) => DependentTier::Gra(gra),
                    talkbank_model::ParseOutcome::Rejected => raw(),
                })
            }),
    };
    match parsed {
        Ok(tier) => utterance.dependent_tiers.push(tier),
        Err(failure) => {
            crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
        }
    }
}

/// Attach parsed %pho or retain the original rejected tier in its source slot.
pub(super) fn attach_pho<'tree>(
    n: SourceBound<'tree, '_, PhoDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    let Ok(children) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(n),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    let Ok(separator) = crate::parser::typed_cst::report_admission(
        super::helpers::dependent_tier_separator(children.children().child_1.slot()),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    let talkbank_model::ParseOutcome::Parsed(source) =
        crate::parser::tree_parsing::parser_helpers::extract_utf8_text(
            tier_node,
            input,
            errors,
            "unparsed PHO source",
        )
    else {
        return;
    };
    let raw = || DependentTier::UnparsedPho(UnparsedTier::new(span_of(tier_node), source));
    let tier = match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => {
            report_tier_parse_error(recovery, "pho", errors);
            raw()
        }
        TierAdmission::Healthy(healthy) => match parse_pho_tier(healthy.node(), errors) {
            Ok(tier) => DependentTier::Pho(tier),
            Err(failure) => {
                crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
                raw()
            }
        },
    };
    utterance
        .dependent_tiers
        .push(DependentTierEntry::with_separator(tier, separator));
}

/// Attach parsed %mod or retain the original rejected tier in its source slot.
pub(super) fn attach_mod<'tree>(
    n: SourceBound<'tree, '_, ModDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    let Ok(children) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(n),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    let Ok(separator) = crate::parser::typed_cst::report_admission(
        super::helpers::dependent_tier_separator(children.children().child_1.slot()),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    let talkbank_model::ParseOutcome::Parsed(source) =
        crate::parser::tree_parsing::parser_helpers::extract_utf8_text(
            tier_node,
            input,
            errors,
            "unparsed MOD source",
        )
    else {
        return;
    };
    let raw = || DependentTier::UnparsedMod(UnparsedTier::new(span_of(tier_node), source));
    let tier = match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => {
            report_tier_parse_error(recovery, "mod", errors);
            raw()
        }
        TierAdmission::Healthy(healthy) => match parse_mod_tier(healthy.node(), errors) {
            Ok(tier) => DependentTier::Mod(tier),
            Err(failure) => {
                crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
                raw()
            }
        },
    };
    utterance
        .dependent_tiers
        .push(DependentTierEntry::with_separator(tier, separator));
}

/// Attach parsed %sin or retain the original rejected tier in its source slot.
pub(super) fn attach_sin<'tree>(
    n: SourceBound<'tree, '_, SinDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let input = n.source();
    let tier_node = n.raw_node();
    let Ok(children) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(n),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    let Ok(separator) = crate::parser::typed_cst::report_admission(
        super::helpers::dependent_tier_separator(children.children().child_1.slot()),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    let talkbank_model::ParseOutcome::Parsed(source) =
        crate::parser::tree_parsing::parser_helpers::extract_utf8_text(
            tier_node,
            input,
            errors,
            "unparsed SIN source",
        )
    else {
        return;
    };
    let raw = || DependentTier::UnparsedSin(UnparsedTier::new(span_of(tier_node), source));
    let tier = match TierAdmission::admit(n) {
        TierAdmission::Recovery(recovery) => {
            report_tier_parse_error(recovery, "sin", errors);
            raw()
        }
        TierAdmission::Healthy(healthy) => match parse_sin_tier(healthy.node(), errors) {
            Ok(tier) => DependentTier::Sin(tier),
            Err(failure) => {
                crate::parser::typed_cst::report_cst_failure(tier_node, input, failure, errors);
                raw()
            }
        },
    };
    utterance
        .dependent_tiers
        .push(DependentTierEntry::with_separator(tier, separator));
}

/// Attach parsed WOR or retain rejected source without inventing word timings.
pub(super) fn attach_wor<'tree>(
    n: SourceBound<'tree, '_, WorDependentTierNode<'tree>>,
    utterance: &mut Utterance,
    errors: &impl ErrorSink,
) {
    let Some(source) = WorSource::admit(n, errors) else {
        return;
    };
    match source.lower(errors) {
        Ok(tier) => utterance.dependent_tiers.push(tier.into_entry()),
        Err(source) => {
            if let Some(entry) = source.into_recovery(errors) {
                utterance.dependent_tiers.push(entry);
            }
        }
    }
}

/// Source evidence survives semantic rejection. Only admission can construct it;
/// lowering returns the same owner when it cannot construct a word tier.
struct WorSource<'tree, 'source> {
    node: SourceBound<'tree, 'source, WorDependentTierNode<'tree>>,
    metadata: WorSourceMetadata,
}

struct WorSourceMetadata {
    separator: TierSeparator,
    content_range: Option<talkbank_model::model::dependent_tier::DependentTierContentSpan>,
}

impl WorSourceMetadata {
    fn into_entry(self, tier: DependentTier) -> DependentTierEntry {
        let mut entry = DependentTierEntry::with_separator(tier, self.separator);
        if let Some(body) = self.content_range {
            entry.record_content_span(body);
        }
        entry
    }
}

impl<'tree, 'source> WorSource<'tree, 'source> {
    fn admit(
        node: SourceBound<'tree, 'source, WorDependentTierNode<'tree>>,
        errors: &impl ErrorSink,
    ) -> Option<Self> {
        let raw = node.raw_node();
        let input = node.source();
        let children = crate::parser::typed_cst::report_admission(
            crate::parser::typed_cst::extract_envelope(node),
            raw,
            input,
            errors,
        )
        .ok()?;
        let separator = crate::parser::typed_cst::report_admission(
            super::helpers::dependent_tier_separator(children.children().child_1.slot()),
            raw,
            input,
            errors,
        )
        .ok()?;
        let content_range = crate::parser::typed_cst::report_admission(
            super::content_range::wor(node),
            raw,
            input,
            errors,
        )
        .ok()?;
        Some(Self {
            node,
            metadata: WorSourceMetadata {
                separator,
                content_range,
            },
        })
    }

    fn lower(self, errors: &impl ErrorSink) -> Result<LoweredWorTier, Self> {
        let healthy = match TierAdmission::admit(self.node) {
            TierAdmission::Recovery(recovery) => {
                report_tier_parse_error(recovery, "wor", errors);
                return Err(self);
            }
            TierAdmission::Healthy(healthy) => healthy,
        };
        let node = healthy.node();
        match crate::parser::typed_cst::report_admission(
            parse_wor_tier(node, errors),
            node.raw_node(),
            node.source(),
            errors,
        ) {
            Ok(talkbank_model::ParseOutcome::Parsed(tier)) => Ok(LoweredWorTier {
                tier,
                metadata: self.metadata,
            }),
            Ok(talkbank_model::ParseOutcome::Rejected) | Err(_) => Err(self),
        }
    }

    fn into_recovery(self, errors: &impl ErrorSink) -> Option<DependentTierEntry> {
        let raw = self.node.raw_node();
        let source = crate::parser::tree_parsing::parser_helpers::extract_utf8_text(
            raw,
            self.node.source(),
            errors,
            "unparsed WOR source",
        )
        .into_option()?;
        Some(
            self.metadata
                .into_entry(DependentTier::UnparsedWor(UnparsedTier::new(
                    span_of(raw),
                    source,
                ))),
        )
    }
}

/// A lowered `%wor` tier and its source separator, held as a word tier by
/// type until it is placed among an utterance's dependent tiers.
pub(crate) struct LoweredWorTier {
    tier: WorTier,
    metadata: WorSourceMetadata,
}

impl LoweredWorTier {
    /// The lowered word tier.
    pub(crate) fn tier(&self) -> &WorTier {
        &self.tier
    }

    /// Move into a dependent-tier entry, keeping the source separator.
    pub(crate) fn into_entry(self) -> DependentTierEntry {
        self.metadata.into_entry(DependentTier::Wor(self.tier))
    }
}

/// Lower one source-bound word tier without deciding whether to retain it.
/// Recovery diagnostics stay with its owner until the disposition is selected.
pub(crate) fn parse_wor_entry<'tree>(
    n: SourceBound<'tree, '_, WorDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<LoweredWorTier> {
    WorSource::admit(n, errors)?.lower(errors).ok()
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
