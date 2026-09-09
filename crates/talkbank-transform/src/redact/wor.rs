//! `%wor` sanitizing: decided before the main tier is rewritten, applied
//! after.
//!
//! `%wor` lists the utterance's own words beside their timing bullets, so
//! writing it out untouched re-exposes every word the main tier hides.
//! Whether a `%wor` word may take its main-tier word's placeholder is the
//! model's question, answered by its one owner of `%wor` membership and
//! pairing: [`WorMainTierProjection::bind_timing`] (the counts) and then
//! [`corroborate_wor_timing`] (the words), which the model says a
//! count-matched pair "must then pass" because equal counts cannot see a
//! same-count edit. Corroboration compares the two tiers' TEXT, so the
//! verdict has to be taken while both still carry it: [`WorPlan::decide`]
//! runs before the main tier is sanitized and [`WorPlan::apply`] after.
//!
//! [`WorMainTierProjection::bind_timing`]: talkbank_model::alignment::WorMainTierProjection::bind_timing

use talkbank_model::alignment::{
    WorTimingBinding, WorTimingCorrespondence, corroborate_wor_timing,
};
use talkbank_model::dependent_tier::WorItem;
use talkbank_model::{DependentTier, MainTier, WorTier};

use super::placeholder::PlaceholderState;
use super::word::sanitize_word;

/// Whether a `%wor` tier agreed with the main tier, decided while both
/// still carried their text.
#[derive(Clone, Copy, Debug)]
enum WorAgreement {
    /// Count-matched and every word matched its main-tier word's display
    /// text: the tier is the main tier's words, and takes their placeholders.
    Corroborated,
    /// Drifted in count, or count-matched with a word that did not match:
    /// nothing pairs, so every word takes a fresh placeholder and the tier
    /// disagrees with the main tier after sanitizing as it did before.
    NotCorroborated,
}

/// The verdict for every `%wor` tier of one utterance, in tier order.
///
/// `decide` and `apply` walk the same dependent tiers through the same
/// `wor_tiers` filter, and only the main tier changes between them, so the
/// verdicts line up with the tiers by position.
#[derive(Debug)]
pub(crate) struct WorPlan {
    verdicts: Vec<WorAgreement>,
}

impl WorPlan {
    /// Decides every `%wor` tier's agreement against the unsanitized main
    /// tier.
    pub(crate) fn decide<'t>(
        main: &MainTier,
        tiers: impl Iterator<Item = &'t DependentTier>,
    ) -> Self {
        let verdicts = wor_tiers(tiers).map(|wor| agreement(main, wor)).collect();
        Self { verdicts }
    }

    /// Rewrites every `%wor` tier in the terms of the now-sanitized main
    /// tier, per its verdict.
    pub(crate) fn apply<'t>(
        self,
        main: &MainTier,
        tiers: impl Iterator<Item = &'t mut DependentTier>,
        state: &mut PlaceholderState,
    ) {
        for (wor, verdict) in wor_tiers_mut(tiers).zip(self.verdicts) {
            match verdict {
                WorAgreement::Corroborated => take_main_placeholders(wor, main, state),
                WorAgreement::NotCorroborated => take_fresh_placeholders(wor, state),
            }
        }
    }
}

fn wor_tiers<'t>(
    tiers: impl Iterator<Item = &'t DependentTier>,
) -> impl Iterator<Item = &'t WorTier> {
    tiers.filter_map(|tier| match tier {
        DependentTier::Wor(wor) => Some(wor),
        _ => None,
    })
}

fn wor_tiers_mut<'t>(
    tiers: impl Iterator<Item = &'t mut DependentTier>,
) -> impl Iterator<Item = &'t mut WorTier> {
    tiers.filter_map(|tier| match tier {
        DependentTier::Wor(wor) => Some(wor),
        _ => None,
    })
}

fn agreement(main: &MainTier, wor: &WorTier) -> WorAgreement {
    match main.wor_projection().bind_timing(Some(wor)) {
        WorTimingBinding::CountMatched(matched) => match corroborate_wor_timing(matched) {
            WorTimingCorrespondence::Corroborated(_) => WorAgreement::Corroborated,
            WorTimingCorrespondence::Uncorroborated(_) => WorAgreement::NotCorroborated,
        },
        // `Missing` is the binding of an absent tier; this one is in hand,
        // so the arm cannot arise, and it is spelled out rather than folded
        // into a catch-all because the enum has the state.
        WorTimingBinding::Drifted(_) | WorTimingBinding::Missing(_) => {
            WorAgreement::NotCorroborated
        }
    }
}

/// Each `%wor` word becomes its paired main-tier word's display text, now
/// that word's placeholders (`w1w1` for a sanitized compound): the form a
/// generated `%wor` carries and the form corroboration compares. Its
/// bullet stays. An untranscribed marker written on `%wor` passes through
/// as it does on the main tier, keeping the slot it holds in the pairing.
fn take_main_placeholders(wor: &mut WorTier, main: &MainTier, state: &mut PlaceholderState) {
    // The binding borrows the tier, so the paired texts are taken before
    // the tier is rewritten; one small string per `%wor` word.
    let paired: Vec<smol_str::SmolStr> = match main.wor_projection().bind_timing(Some(wor)) {
        WorTimingBinding::CountMatched(matched) => matched
            .pairs()
            .map(|(main_word, _wor_word)| smol_str::SmolStr::new(main_word.cleaned_text()))
            .collect(),
        // Sanitizing changes no word count, so a tier decided Corroborated
        // binds count-matched again; the arms the enum has take the
        // non-asserting action.
        WorTimingBinding::Drifted(_) | WorTimingBinding::Missing(_) => {
            take_fresh_placeholders(wor, state);
            return;
        }
    };
    for (word, text) in words_mut(wor).zip(paired) {
        if word.untranscribed().is_some() {
            continue;
        }
        word.replace_simple_text(text);
    }
}

/// Every word takes a fresh placeholder; untranscribed markers pass through
/// (`sanitize_word`'s own rule).
fn take_fresh_placeholders(wor: &mut WorTier, state: &mut PlaceholderState) {
    for word in words_mut(wor) {
        sanitize_word(word, state);
    }
}

fn words_mut(wor: &mut WorTier) -> impl Iterator<Item = &mut talkbank_model::Word> {
    wor.items.iter_mut().filter_map(|item| match item {
        WorItem::Word(word) => Some(word.as_mut()),
        WorItem::Separator { .. } => None,
    })
}
