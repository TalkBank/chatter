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
use crate::model::dependent_tier::DependentTier;
use crate::model::{TextTier, Utterance};
use crate::node_types::*;
use talkbank_model::ParseOutcome;
use talkbank_model::model::dependent_tier::{
    PhoalnTier, SylTier, SylTierType, XphointTier, parse_phoaln_content, parse_syl_content,
    parse_xphoint_content,
};
use tree_sitter::Node;

use super::helpers::read_tier_body_text;

/// Apply a raw (text) tier to the utterance.
///
/// Returns `true` if this tier type was handled (even if content extraction failed).
/// Returns `false` if this is not a raw tier type.
///
/// If content extraction fails, the tier is NOT added to the utterance and an
/// error has already been reported.
///
/// Every raw tier shares the grammar shape
/// `seq(<x>_tier_prefix, tier_sep, text_with_bullets, newline)`. Each arm drives
/// the generated typed visitor: it extracts its concrete tier via
/// `extract_<kind>_dependent_tier`, reads the body (`child_2`) and surfaces the
/// carrier's `unexpected` sink through the shared [`read_tier_body_text`], and,
/// when the body parses to a non-empty content string, builds and pushes its
/// concrete [`DependentTier`] variant. This replaces the removed
/// `extract_unparsed_tier_content` `match child.kind()` body-location hand-walk;
/// the per-tier model construction (plain [`TextTier`] vs the fallible Phon-tier
/// parses) is unchanged.
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
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Ort(TextTier::new(content).with_span(span)));
            }
        }
        ENG_DEPENDENT_TIER => {
            let children = extract_eng_dependent_tier(EngDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Eng(TextTier::new(content).with_span(span)));
            }
        }
        GLS_DEPENDENT_TIER => {
            let children = extract_gls_dependent_tier(GlsDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Gls(TextTier::new(content).with_span(span)));
            }
        }
        ALT_DEPENDENT_TIER => {
            let children = extract_alt_dependent_tier(AltDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Alt(TextTier::new(content).with_span(span)));
            }
        }
        COH_DEPENDENT_TIER => {
            let children = extract_coh_dependent_tier(CohDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Coh(TextTier::new(content).with_span(span)));
            }
        }
        DEF_DEPENDENT_TIER => {
            let children = extract_def_dependent_tier(DefDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Def(TextTier::new(content).with_span(span)));
            }
        }
        ERR_DEPENDENT_TIER => {
            let children = extract_err_dependent_tier(ErrDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Err(TextTier::new(content).with_span(span)));
            }
        }
        FAC_DEPENDENT_TIER => {
            let children = extract_fac_dependent_tier(FacDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Fac(TextTier::new(content).with_span(span)));
            }
        }
        FLO_DEPENDENT_TIER => {
            let children = extract_flo_dependent_tier(FloDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Flo(TextTier::new(content).with_span(span)));
            }
        }
        PAR_DEPENDENT_TIER => {
            let children = extract_par_dependent_tier(ParDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Par(TextTier::new(content).with_span(span)));
            }
        }
        TIM_DEPENDENT_TIER => {
            let children = extract_tim_dependent_tier(TimDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                utterance.dependent_tiers.push(DependentTier::Tim(
                    crate::model::dependent_tier::TimTier::from_text(content).with_span(span),
                ));
            }
        }
        MODSYL_DEPENDENT_TIER => {
            let children = extract_modsyl_dependent_tier(ModsylDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                let words = parse_syl_content(content.as_str());
                utterance.dependent_tiers.push(DependentTier::Modsyl(
                    SylTier::new(SylTierType::Modsyl, words).with_span(span),
                ));
            }
        }
        PHOSYL_DEPENDENT_TIER => {
            let children = extract_phosyl_dependent_tier(PhosylDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                let words = parse_syl_content(content.as_str());
                utterance.dependent_tiers.push(DependentTier::Phosyl(
                    SylTier::new(SylTierType::Phosyl, words).with_span(span),
                ));
            }
        }
        PHOALN_DEPENDENT_TIER => {
            let children = extract_phoaln_dependent_tier(PhoalnDependentTierNode(tier_node));
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                match parse_phoaln_content(content.as_str()) {
                    Ok(words) => {
                        utterance.dependent_tiers.push(DependentTier::Phoaln(
                            PhoalnTier::new(words).with_span(span),
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
            if let ParseOutcome::Parsed(content) = read_tier_body_text(
                tier_node,
                children.child_2.slot,
                &children.unexpected,
                input,
                errors,
            ) {
                match parse_xphoint_content(content.as_str()) {
                    Ok(groups) => {
                        utterance.dependent_tiers.push(DependentTier::Xphoint(
                            XphointTier::new(groups).with_span(span),
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
