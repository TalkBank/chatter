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
    AsRawNode, SourceBound, UtteranceChild1Choice, UtteranceChild1ChoiceBoundView,
};
use crate::model::Utterance;
use crate::model::dependent_tier::DependentTier;
use crate::parser::tier_parsers::act::parse_act_tier_with_separator;
use crate::parser::tier_parsers::cod::parse_cod_tier_with_separator;
use crate::parser::tier_parsers::text::{
    parse_add_tier_with_separator, parse_com_tier_with_separator, parse_exp_tier_with_separator,
    parse_gpx_tier_with_separator, parse_int_tier_with_separator, parse_sit_tier_with_separator,
    parse_spa_tier_with_separator,
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
    let first_produced = utterance.dependent_tiers.len();
    // Selecting a concrete variant preserves the choice's admitted range.
    // Every leaf adapter retains that source-bound concrete variant.
    let reconstructed = (|| -> Result<(), crate::CstFailure> {
        let content_range = super::content_range::selected(choice)?;
        match choice.view() {
            // Structured tiers with dedicated parsers + `has_error` gating.
            C::MorDependentTier(n) => {
                parsed::attach_mor(n, &mut utterance, errors);
            }
            C::GraDependentTier(n) => {
                parsed::attach_gra(n, &mut utterance, errors);
            }
            C::PhoDependentTier(n) => {
                parsed::attach_pho(n, &mut utterance, errors);
            }
            C::ModDependentTier(n) => {
                parsed::attach_mod(n, &mut utterance, errors);
            }
            C::SinDependentTier(n) => {
                parsed::attach_sin(n, &mut utterance, errors);
            }
            C::WorDependentTier(n) => {
                parsed::attach_wor(n, &mut utterance, errors);
            }
            // Bullet/text tiers: parse directly, no `has_error` gate (unchanged).
            C::ComDependentTier(n) => {
                if let talkbank_model::ParseOutcome::Parsed(tier) =
                    parse_com_tier_with_separator(n, errors)?
                {
                    utterance
                        .dependent_tiers
                        .push(tier.into_entry(DependentTier::Com));
                }
            }
            C::ExpDependentTier(n) => {
                if let talkbank_model::ParseOutcome::Parsed(tier) =
                    parse_exp_tier_with_separator(n, errors)?
                {
                    utterance
                        .dependent_tiers
                        .push(tier.into_entry(DependentTier::Exp));
                }
            }
            C::AddDependentTier(n) => {
                if let talkbank_model::ParseOutcome::Parsed(tier) =
                    parse_add_tier_with_separator(n, errors)?
                {
                    utterance
                        .dependent_tiers
                        .push(tier.into_entry(DependentTier::Add));
                }
            }
            C::SpaDependentTier(n) => {
                if let talkbank_model::ParseOutcome::Parsed(tier) =
                    parse_spa_tier_with_separator(n, errors)?
                {
                    utterance
                        .dependent_tiers
                        .push(tier.into_entry(DependentTier::Spa));
                }
            }
            C::SitDependentTier(n) => {
                if let talkbank_model::ParseOutcome::Parsed(tier) =
                    parse_sit_tier_with_separator(n, errors)?
                {
                    utterance
                        .dependent_tiers
                        .push(tier.into_entry(DependentTier::Sit));
                }
            }
            C::IntDependentTier(n) => {
                if let talkbank_model::ParseOutcome::Parsed(tier) =
                    parse_int_tier_with_separator(n, errors)?
                {
                    utterance
                        .dependent_tiers
                        .push(tier.into_entry(DependentTier::Int));
                }
            }
            C::GpxDependentTier(n) => {
                if let talkbank_model::ParseOutcome::Parsed(tier) =
                    parse_gpx_tier_with_separator(n, errors)?
                {
                    utterance
                        .dependent_tiers
                        .push(tier.into_entry(DependentTier::Gpx));
                }
            }
            C::CodDependentTier(n) => {
                if let talkbank_model::ParseOutcome::Parsed(tier) =
                    parse_cod_tier_with_separator(n, errors)?
                {
                    utterance
                        .dependent_tiers
                        .push(tier.into_entry(DependentTier::Cod));
                }
            }
            C::ActDependentTier(n) => {
                if let talkbank_model::ParseOutcome::Parsed(tier) =
                    parse_act_tier_with_separator(n, errors)?
                {
                    utterance
                        .dependent_tiers
                        .push(tier.into_entry(DependentTier::Act));
                }
            }
            // Raw text tiers: the `raw` applier keyed on the concrete kind CONST
            // taken from the typed variant (not `node.kind()`).
            C::OrtDependentTier(n) => {
                raw::apply_ort(&mut utterance, n, errors);
            }
            C::EngDependentTier(n) => {
                raw::apply_eng(&mut utterance, n, errors);
            }
            C::GlsDependentTier(n) => {
                raw::apply_gls(&mut utterance, n, errors);
            }
            C::AltDependentTier(n) => {
                raw::apply_alt(&mut utterance, n, errors);
            }
            C::CohDependentTier(n) => {
                raw::apply_coh(&mut utterance, n, errors);
            }
            C::DefDependentTier(n) => {
                raw::apply_def(&mut utterance, n, errors);
            }
            C::ErrDependentTier(n) => {
                raw::apply_err(&mut utterance, n, errors);
            }
            C::FacDependentTier(n) => {
                raw::apply_fac(&mut utterance, n, errors);
            }
            C::FloDependentTier(n) => {
                raw::apply_flo(&mut utterance, n, errors);
            }
            C::ParDependentTier(n) => {
                raw::apply_par(&mut utterance, n, errors);
            }
            C::TimDependentTier(n) => {
                raw::apply_tim(&mut utterance, n, errors);
            }
            C::ModsylDependentTier(n) => {
                raw::apply_modsyl(&mut utterance, n, errors);
            }
            C::PhosylDependentTier(n) => {
                raw::apply_phosyl(&mut utterance, n, errors);
            }
            C::PhoalnDependentTier(n) => {
                raw::apply_phoaln(&mut utterance, n, errors);
            }
            C::XphointDependentTier(n) => {
                raw::apply_xphoint(&mut utterance, n, errors);
            }
            // User-defined `%x*` and unsupported catch-all tiers.
            C::XDependentTier(n) => {
                user_defined::apply_x_tier(&mut utterance, n, errors);
            }
            C::UnsupportedDependentTier(n) => {
                user_defined::apply_unsupported_tier(&mut utterance, n, errors);
            }
        }

        if let Some(body) = content_range {
            // Only this selected source tier's newly lowered entries receive
            // its evidence. Already attached siblings are never retargeted.
            for entry in &mut utterance.dependent_tiers[first_produced..] {
                entry.record_content_span(body);
            }
        }
        Ok(())
    })();
    if let Err(fault) = reconstructed {
        crate::parser::typed_cst::report_cst_failure(choice.raw_node(), input, fault, errors);
    }
    utterance
}
