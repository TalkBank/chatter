//! Dispatch for simple text-like dependent tiers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{
    AltDependentTierNode, CohDependentTierNode, DefDependentTierNode, EngDependentTierNode,
    ErrDependentTierNode, FacDependentTierNode, FloDependentTierNode, GlsDependentTierNode,
    ModsylDependentTierNode, OrtDependentTierNode, ParDependentTierNode, PhoalnDependentTierNode,
    PhosylDependentTierNode, TimDependentTierNode, XphointDependentTierNode,
    extract_alt_dependent_tier, extract_coh_dependent_tier, extract_def_dependent_tier,
    extract_eng_dependent_tier, extract_err_dependent_tier, extract_fac_dependent_tier,
    extract_flo_dependent_tier, extract_gls_dependent_tier, extract_modsyl_dependent_tier,
    extract_ort_dependent_tier, extract_par_dependent_tier, extract_phoaln_dependent_tier,
    extract_phosyl_dependent_tier, extract_tim_dependent_tier, extract_xphoint_dependent_tier,
};
use crate::model::Utterance;
use crate::model::dependent_tier::{DependentTier, DependentTierEntry};
use crate::node_types::*;
use talkbank_model::ParseOutcome;
use talkbank_model::model::dependent_tier::{
    PhoalnTier, SylTier, SylTierType, XphointTier, parse_phoaln_content, parse_syl_content,
    parse_xphoint_content,
};
use tree_sitter::Node;

use super::helpers::{read_optional_tier_body_raw_text, read_optional_tier_body_text};

/// Apply a raw (text) tier to the utterance.
///
/// Returns `true` if this tier type was handled (even if content extraction failed).
/// Returns `false` if this is not a raw tier type.
///
/// If content extraction fails, the tier is NOT added to the utterance and an
/// error has already been reported.
///
/// Every raw tier shares the grammar shape
/// `seq(<x>_tier_prefix, tier_sep, optional(text_with_bullets), newline)`. Each
/// arm drives the generated typed visitor: it extracts its concrete tier via
/// `extract_<kind>_dependent_tier`, reads the body (`child_2`) and surfaces the
/// carrier's `unexpected` sink through [`read_optional_tier_body_text`] or
/// [`read_optional_tier_body_raw_text`], and builds and pushes its concrete
/// [`DependentTier`] variant. This replaces the removed
/// `extract_unparsed_tier_content` `match child.kind()` body-location hand-walk;
/// the per-tier model construction (plain [`TextTier`] vs the fallible Phon-tier
/// parses) is unchanged.
///
/// A tier is pushed even when its body is ABSENT. That is the E756 widening: an
/// empty tier line is a real (if invalid) construct, so the parser records what
/// the file contains and `DependentTier::declares_nothing` lets the validator
/// judge it. This doc previously said a tier is pushed "when the body parses to
/// a non-empty content string", which is now false by design; dropping the tier
/// is what used to lose the line on roundtrip.
pub(super) fn apply_raw_tier(
    utterance: &mut Utterance,
    tier_kind: &str,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    let span = Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);

    match tier_kind {
        ORT_DEPENDENT_TIER => {
            let children = extract_ort_dependent_tier(OrtDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Ort(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        ENG_DEPENDENT_TIER => {
            let children = extract_eng_dependent_tier(EngDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Eng(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        GLS_DEPENDENT_TIER => {
            let children = extract_gls_dependent_tier(GlsDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Gls(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        ALT_DEPENDENT_TIER => {
            let children = extract_alt_dependent_tier(AltDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Alt(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        COH_DEPENDENT_TIER => {
            let children = extract_coh_dependent_tier(CohDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Coh(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        DEF_DEPENDENT_TIER => {
            let children = extract_def_dependent_tier(DefDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Def(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        ERR_DEPENDENT_TIER => {
            let children = extract_err_dependent_tier(ErrDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Err(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        FAC_DEPENDENT_TIER => {
            let children = extract_fac_dependent_tier(FacDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Fac(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        FLO_DEPENDENT_TIER => {
            let children = extract_flo_dependent_tier(FloDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Flo(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        PAR_DEPENDENT_TIER => {
            let children = extract_par_dependent_tier(ParDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Par(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        TIM_DEPENDENT_TIER => {
            let children = extract_tim_dependent_tier(TimDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(content) = read_optional_tier_body_raw_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                // An absent body is `TimTier::Empty`, not a fabricated time and
                // not a dropped tier: E756 judges it and the line roundtrips.
                let tier = match content {
                    Some(text) => crate::model::dependent_tier::TimTier::from_text(text),
                    None => crate::model::dependent_tier::TimTier::empty(),
                };
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Tim(tier.with_span(span)),
                        separator,
                    ));
            }
        }
        MODSYL_DEPENDENT_TIER => {
            let children = extract_modsyl_dependent_tier(ModsylDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(content) = read_optional_tier_body_raw_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                // An absent body yields no words, which is what `SylTier::is_empty`
                // reports and E756 judges; the tier stays in the model either way.
                let words = match &content {
                    Some(text) => parse_syl_content(text.as_str()),
                    None => Vec::new(),
                };
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Modsyl(
                            SylTier::new(SylTierType::Modsyl, words).with_span(span),
                        ),
                        separator,
                    ));
            }
        }
        PHOSYL_DEPENDENT_TIER => {
            let children = extract_phosyl_dependent_tier(PhosylDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(content) = read_optional_tier_body_raw_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                // An absent body yields no words, which is what `SylTier::is_empty`
                // reports and E756 judges; the tier stays in the model either way.
                let words = match &content {
                    Some(text) => parse_syl_content(text.as_str()),
                    None => Vec::new(),
                };
                utterance
                    .dependent_tiers
                    .push(DependentTierEntry::with_separator(
                        DependentTier::Phosyl(
                            SylTier::new(SylTierType::Phosyl, words).with_span(span),
                        ),
                        separator,
                    ));
            }
        }
        PHOALN_DEPENDENT_TIER => {
            let children = extract_phoaln_dependent_tier(PhoalnDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(content) = read_optional_tier_body_raw_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                // An absent body is the empty tier, which `PhoalnTier::is_empty`
                // reports and E756 judges. Only a body that IS there and does
                // not parse is a malformed-content error, so the absent case
                // never reaches the content parser.
                let parsed = match &content {
                    Some(text) => parse_phoaln_content(text.as_str()),
                    None => Ok(Vec::new()),
                };
                match parsed {
                    Ok(words) => {
                        utterance
                            .dependent_tiers
                            .push(DependentTierEntry::with_separator(
                                DependentTier::Phoaln(PhoalnTier::new(words).with_span(span)),
                                separator,
                            ));
                    }
                    Err(e) => {
                        errors.report(ParseError::new(
                            ErrorCode::InvalidDependentTier,
                            Severity::Error,
                            SourceLocation::from_offsets(
                                tier_node.start_byte(),
                                tier_node.end_byte(),
                            ),
                            ErrorContext::new(
                                input,
                                tier_node.start_byte()..tier_node.end_byte(),
                                "%phoaln",
                            ),
                            format!("malformed %phoaln content: {}", e),
                        ));
                    }
                }
            }
        }
        XPHOINT_DEPENDENT_TIER => {
            let children = extract_xphoint_dependent_tier(XphointDependentTierNode(tier_node));
            let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
            if let ParseOutcome::Parsed(content) = read_optional_tier_body_raw_text(
                tier_node,
                children.child_2.slot(),
                &children.unexpected,
                input,
                errors,
            ) {
                // An absent body is the empty tier, which `XphointTier::is_empty`
                // reports and E756 judges. Only a body that IS there and does
                // not parse is a malformed-content error, so the absent case
                // never reaches the content parser.
                let parsed = match &content {
                    Some(text) => parse_xphoint_content(text.as_str()),
                    None => Ok(Vec::new()),
                };
                match parsed {
                    Ok(groups) => {
                        utterance
                            .dependent_tiers
                            .push(DependentTierEntry::with_separator(
                                DependentTier::Xphoint(XphointTier::new(groups).with_span(span)),
                                separator,
                            ));
                    }
                    Err(e) => {
                        errors.report(ParseError::new(
                            ErrorCode::InvalidDependentTier,
                            Severity::Error,
                            SourceLocation::from_offsets(
                                tier_node.start_byte(),
                                tier_node.end_byte(),
                            ),
                            ErrorContext::new(
                                input,
                                tier_node.start_byte()..tier_node.end_byte(),
                                "%xphoint",
                            ),
                            format!("malformed %xphoint content: {}", e),
                        ));
                    }
                }
            }
        }
        _ => return false,
    }

    true
}
