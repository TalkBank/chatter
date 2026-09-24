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
//! The raw text tiers and the user-defined / unsupported tiers are handled by
//! the per-tier appliers in [`raw`] / [`user_defined`], each taking that tier's
//! OWN typed node, so those modules' established behavior is reused unchanged.
//! Both modules used to take a kind CONSTANT alongside a raw node and rebuild
//! the type from it; the constant is what the typed appliers removed.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    SourceBound, UtteranceChild1Choice, UtteranceChild1ChoiceBoundView, extract_act_dependent_tier,
    extract_add_dependent_tier, extract_cod_dependent_tier, extract_com_dependent_tier,
    extract_exp_dependent_tier, extract_gpx_dependent_tier, extract_int_dependent_tier,
    extract_sit_dependent_tier, extract_spa_dependent_tier,
};
use crate::model::Utterance;
use crate::model::dependent_tier::{DependentTier, DependentTierEntry};
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
/// the per-tier appliers in [`raw`], and the `%x*` / unsupported tiers via
/// [`user_defined`]. The match is exhaustive (no
/// `_ =>`), so a future tier subtype is a compile error here rather than a
/// silently-dropped tier.
pub(crate) fn parse_and_attach_dependent_tier<'tree>(
    mut utterance: Utterance,
    choice: SourceBound<'tree, '_, UtteranceChild1Choice<'tree>>,
    errors: &impl ErrorSink,
) -> Utterance {
    use UtteranceChild1ChoiceBoundView as C;
    let input = choice.source();
    // Selecting a concrete variant preserves the choice's admitted range.
    // Leaf adapters still receive the same owner's source during migration.
    match choice.view() {
        // Structured tiers with dedicated parsers + `has_error` gating.
        C::MorDependentTier(n) => {
            let n = n.node();
            parsed::attach_mor(n, &mut utterance, input, errors);
        }
        C::GraDependentTier(n) => {
            let n = n.node();
            parsed::attach_gra(n, &mut utterance, input, errors);
        }
        C::PhoDependentTier(n) => {
            let n = n.node();
            parsed::attach_pho(n, &mut utterance, input, errors);
        }
        C::ModDependentTier(n) => {
            let n = n.node();
            parsed::attach_mod(n, &mut utterance, input, errors);
        }
        C::SinDependentTier(n) => {
            let n = n.node();
            parsed::attach_sin(n, &mut utterance, input, errors);
        }
        C::WorDependentTier(n) => {
            let n = n.node();
            parsed::attach_wor(n, &mut utterance, input, errors);
        }
        // Bullet/text tiers: parse directly, no `has_error` gate (unchanged).
        C::ComDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                extract_com_dependent_tier(n.node()).child_1.slot(),
            );
            let tier = parse_com_tier(n, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Com(tier),
                    separator,
                ));
        }
        C::ExpDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                extract_exp_dependent_tier(n.node()).child_1.slot(),
            );
            let tier = parse_exp_tier(n, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Exp(tier),
                    separator,
                ));
        }
        C::AddDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                extract_add_dependent_tier(n.node()).child_1.slot(),
            );
            let tier = parse_add_tier(n, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Add(tier),
                    separator,
                ));
        }
        C::SpaDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                extract_spa_dependent_tier(n.node()).child_1.slot(),
            );
            let tier = parse_spa_tier(n, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Spa(tier),
                    separator,
                ));
        }
        C::SitDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                extract_sit_dependent_tier(n.node()).child_1.slot(),
            );
            let tier = parse_sit_tier(n, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Sit(tier),
                    separator,
                ));
        }
        C::IntDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                extract_int_dependent_tier(n.node()).child_1.slot(),
            );
            let tier = parse_int_tier(n, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Int(tier),
                    separator,
                ));
        }
        C::GpxDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                extract_gpx_dependent_tier(n.node()).child_1.slot(),
            );
            let tier = parse_gpx_tier(n, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Gpx(tier),
                    separator,
                ));
        }
        C::CodDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                extract_cod_dependent_tier(n.node()).child_1.slot(),
            );
            let tier = parse_cod_tier(n, errors);
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Cod(tier),
                    separator,
                ));
        }
        C::ActDependentTier(n) => {
            let separator = super::helpers::dependent_tier_separator(
                extract_act_dependent_tier(n.node()).child_1.slot(),
            );
            let tier = parse_act_tier(n, errors);
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
            let n = n.node();
            raw::apply_ort(&mut utterance, n, input, errors);
        }
        C::EngDependentTier(n) => {
            let n = n.node();
            raw::apply_eng(&mut utterance, n, input, errors);
        }
        C::GlsDependentTier(n) => {
            let n = n.node();
            raw::apply_gls(&mut utterance, n, input, errors);
        }
        C::AltDependentTier(n) => {
            let n = n.node();
            raw::apply_alt(&mut utterance, n, input, errors);
        }
        C::CohDependentTier(n) => {
            let n = n.node();
            raw::apply_coh(&mut utterance, n, input, errors);
        }
        C::DefDependentTier(n) => {
            let n = n.node();
            raw::apply_def(&mut utterance, n, input, errors);
        }
        C::ErrDependentTier(n) => {
            let n = n.node();
            raw::apply_err(&mut utterance, n, input, errors);
        }
        C::FacDependentTier(n) => {
            let n = n.node();
            raw::apply_fac(&mut utterance, n, input, errors);
        }
        C::FloDependentTier(n) => {
            let n = n.node();
            raw::apply_flo(&mut utterance, n, input, errors);
        }
        C::ParDependentTier(n) => {
            let n = n.node();
            raw::apply_par(&mut utterance, n, input, errors);
        }
        C::TimDependentTier(n) => {
            let n = n.node();
            raw::apply_tim(&mut utterance, n, input, errors);
        }
        C::ModsylDependentTier(n) => {
            let n = n.node();
            raw::apply_modsyl(&mut utterance, n, input, errors);
        }
        C::PhosylDependentTier(n) => {
            let n = n.node();
            raw::apply_phosyl(&mut utterance, n, input, errors);
        }
        C::PhoalnDependentTier(n) => {
            let n = n.node();
            raw::apply_phoaln(&mut utterance, n, input, errors);
        }
        C::XphointDependentTier(n) => {
            let n = n.node();
            raw::apply_xphoint(&mut utterance, n, input, errors);
        }
        // User-defined `%x*` and unsupported catch-all tiers.
        C::XDependentTier(n) => {
            user_defined::apply_x_tier(&mut utterance, n, errors);
        }
        C::UnsupportedDependentTier(n) => {
            let n = n.node();
            user_defined::apply_unsupported_tier(&mut utterance, n, input, errors);
        }
    }

    utterance
}
