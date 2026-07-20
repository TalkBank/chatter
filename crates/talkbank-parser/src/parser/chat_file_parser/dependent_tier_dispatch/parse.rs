//! Dependent-tier dispatch that attaches one parsed tier onto a parent utterance.
//!
//! Driven by the generated typed visitor: the utterance parser already extracts
//! each dependent tier as a typed `UtteranceChild1Choice` (the `dependent_tier`
//! supertype, classified into its concrete subtype), so this dispatch is a single
//! EXHAUSTIVE `match` over that 32-variant enum, replacing THREE removed
//! `node.kind()` hand-walks at once:
//!
//! - the old `resolve_tier_node` (`node.kind() == DEPENDENT_TIER` + `child(0)`
//!   supertype unwrap): unnecessary now, the choice is already concrete;
//! - the old `apply_parsed_tier`'s raw-`&str` `match tier_kind`: replaced by the
//!   typed arms below (the `%mor` / `%gra` / `%pho` / `%mod` / `%sin` / `%wor`
//!   gating lives in [`parsed`], the text tiers are decoded inline);
//! - the old four-applier `&str` cascade with its `InvalidDependentTier`
//!   fallthrough: the `match` is exhaustive over every concrete tier, so no
//!   fallthrough is reachable (both of the old fallthrough's triggers, an
//!   unknown concrete kind and a childless `dependent_tier` supertype, cannot
//!   occur once the tier is a typed concrete variant).
//!
//! The raw text tiers and the user-defined / unsupported tiers are still handled
//! by the appliers in [`raw`] / [`user_defined`], called with the concrete kind
//! CONSTANT taken from the typed variant (NOT `node.kind()`), so those modules'
//! established behavior is reused unchanged.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, UtteranceChild1Choice, extract_act_dependent_tier, extract_add_dependent_tier,
    extract_cod_dependent_tier, extract_com_dependent_tier, extract_exp_dependent_tier,
    extract_gpx_dependent_tier, extract_int_dependent_tier, extract_sit_dependent_tier,
    extract_spa_dependent_tier,
};
use crate::model::Utterance;
use crate::model::dependent_tier::{DependentTier, DependentTierEntry};
use crate::node_types::*;
use crate::parser::tier_parsers::act::parse_act_tier;
use crate::parser::tier_parsers::cod::parse_cod_tier;
use crate::parser::tier_parsers::text::{
    parse_add_tier, parse_com_tier, parse_exp_tier, parse_gpx_tier, parse_int_tier, parse_sit_tier,
    parse_spa_tier,
};

use super::{parsed, raw, user_defined};

/// Parse one dependent tier (already classified as a typed
/// [`UtteranceChild1Choice`]) and attach it to `utterance`.
///
/// Every concrete tier variant is handled: the `has_error`-gated structured
/// tiers via [`parsed`], the bullet/text tiers inline, the 15 raw text tiers via
/// [`raw::apply_raw_tier`], and the `%x*` / unsupported tiers via
/// [`user_defined::apply_user_defined_tier`]. The match is exhaustive (no
/// `_ =>`), so a future tier subtype is a compile error here rather than a
/// silently-dropped tier.
pub(crate) fn parse_and_attach_dependent_tier(
    mut utterance: Utterance,
    choice: UtteranceChild1Choice,
    input: &str,
    errors: &impl ErrorSink,
) -> Utterance {
    use UtteranceChild1Choice as C;
    match choice {
        // Structured tiers with dedicated parsers + `has_error` gating.
        C::MorDependentTier(n) => parsed::attach_mor(n, &mut utterance, input, errors),
        C::GraDependentTier(n) => parsed::attach_gra(n, &mut utterance, input, errors),
        C::PhoDependentTier(n) => parsed::attach_pho(n, &mut utterance, input, errors),
        C::ModDependentTier(n) => parsed::attach_mod(n, &mut utterance, input, errors),
        C::SinDependentTier(n) => parsed::attach_sin(n, &mut utterance, input, errors),
        C::WorDependentTier(n) => parsed::attach_wor(n, &mut utterance, input, errors),
        // Bullet/text tiers: parse directly, no `has_error` gate (unchanged).
        C::ComDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                &extract_com_dependent_tier(n).child_1.slot,
            );
            let tier = parse_com_tier(n.raw_node(), input, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Com(tier),
                    separator,
                ));
        }
        C::ExpDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                &extract_exp_dependent_tier(n).child_1.slot,
            );
            let tier = parse_exp_tier(n.raw_node(), input, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Exp(tier),
                    separator,
                ));
        }
        C::AddDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                &extract_add_dependent_tier(n).child_1.slot,
            );
            let tier = parse_add_tier(n.raw_node(), input, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Add(tier),
                    separator,
                ));
        }
        C::SpaDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                &extract_spa_dependent_tier(n).child_1.slot,
            );
            let tier = parse_spa_tier(n.raw_node(), input, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Spa(tier),
                    separator,
                ));
        }
        C::SitDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                &extract_sit_dependent_tier(n).child_1.slot,
            );
            let tier = parse_sit_tier(n.raw_node(), input, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Sit(tier),
                    separator,
                ));
        }
        C::IntDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                &extract_int_dependent_tier(n).child_1.slot,
            );
            let tier = parse_int_tier(n.raw_node(), input, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Int(tier),
                    separator,
                ));
        }
        C::GpxDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                &extract_gpx_dependent_tier(n).child_1.slot,
            );
            let tier = parse_gpx_tier(n.raw_node(), input, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Gpx(tier),
                    separator,
                ));
        }
        C::CodDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                &extract_cod_dependent_tier(n).child_1.slot,
            );
            let tier = parse_cod_tier(n.raw_node(), input, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Cod(tier),
                    separator,
                ));
        }
        C::ActDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                &extract_act_dependent_tier(n).child_1.slot,
            );
            let tier = parse_act_tier(n.raw_node(), input, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Act(tier),
                    separator,
                ));
        }
        // Raw text tiers: the `raw` applier keyed on the concrete kind CONST
        // taken from the typed variant (not `node.kind()`).
        C::OrtDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, ORT_DEPENDENT_TIER, node, input, errors);
        }
        C::EngDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, ENG_DEPENDENT_TIER, node, input, errors);
        }
        C::GlsDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, GLS_DEPENDENT_TIER, node, input, errors);
        }
        C::AltDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, ALT_DEPENDENT_TIER, node, input, errors);
        }
        C::CohDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, COH_DEPENDENT_TIER, node, input, errors);
        }
        C::DefDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, DEF_DEPENDENT_TIER, node, input, errors);
        }
        C::ErrDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, ERR_DEPENDENT_TIER, node, input, errors);
        }
        C::FacDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, FAC_DEPENDENT_TIER, node, input, errors);
        }
        C::FloDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, FLO_DEPENDENT_TIER, node, input, errors);
        }
        C::ParDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, PAR_DEPENDENT_TIER, node, input, errors);
        }
        C::TimDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, TIM_DEPENDENT_TIER, node, input, errors);
        }
        C::ModsylDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, MODSYL_DEPENDENT_TIER, node, input, errors);
        }
        C::PhosylDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, PHOSYL_DEPENDENT_TIER, node, input, errors);
        }
        C::PhoalnDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, PHOALN_DEPENDENT_TIER, node, input, errors);
        }
        C::XphointDependentTier(n) => {
            let node = n.raw_node();
            raw::apply_raw_tier(&mut utterance, XPHOINT_DEPENDENT_TIER, node, input, errors);
        }
        // User-defined `%x*` and unsupported catch-all tiers.
        C::XDependentTier(n) => {
            let node = n.raw_node();
            user_defined::apply_user_defined_tier(
                &mut utterance,
                X_DEPENDENT_TIER,
                node,
                input,
                errors,
            );
        }
        C::UnsupportedDependentTier(n) => {
            let node = n.raw_node();
            user_defined::apply_user_defined_tier(
                &mut utterance,
                UNSUPPORTED_DEPENDENT_TIER,
                node,
                input,
                errors,
            );
        }
    }

    utterance
}
