//! Builder-style constructors for assembling `Utterance` values.
//!
//! `add_dependent_tier` is the one route for appending a tier; the four
//! `with_*` helpers left are the ones with a caller, each one line over it.
//! Tiers append in call order, which is also the order preserved during CHAT
//! serialization, and each call returns a fresh `Utterance` so callers can
//! chain inside `map`. Until 2026-09-08 there was a `with_*` helper for every
//! tier kind plus `with_preceding_headers` and `with_user_defined`; none of
//! those had a caller in this repository or any known dependant, and a
//! whole-workspace coverage run showed them unreached, so they went.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::Utterance;
use crate::model::dependent_tier::*;
use crate::model::{
    MainTier, ParseHealthState, ParseHealthTier, UtteranceLanguage, UtteranceLanguageMetadata,
};
use smallvec::SmallVec;

impl Utterance {
    /// Create a new utterance with only its required main tier.
    ///
    /// Runtime-derived metadata fields start in their uncomputed/empty state.
    /// Callers can then append dependent tiers in explicit serialization order
    /// via the `with_*`/`add_dependent_tier` helpers.
    pub fn new(main: MainTier) -> Self {
        Self {
            preceding_headers: SmallVec::new(),
            main,
            dependent_tiers: SmallVec::new(),
            alignments: None,
            alignment_diagnostics: Vec::new(),
            parse_health: ParseHealthState::Clean,
            utterance_language: UtteranceLanguage::Uncomputed,
            language_metadata: UtteranceLanguageMetadata::Uncomputed,
        }
    }

    /// Marks one tier as parse-tainted for downstream alignment gating.
    ///
    /// Use this after parser recovery when a specific tier parsed with damage
    /// but the utterance should still flow through later pipeline stages.
    pub fn mark_parse_taint(&mut self, tier: ParseHealthTier) {
        self.parse_health.taint(tier);
    }

    /// Marks all alignment-relevant dependent tiers as parse-tainted.
    ///
    /// This is the coarse fallback used when recovery quality is unclear and
    /// alignments should prefer "skip with diagnostics" behavior.
    pub fn mark_all_dependent_alignment_taint(&mut self) {
        self.parse_health.taint_all_alignment_dependents();
    }

    /// Appends one dependent tier in serialization order.
    ///
    /// Ordering is significant for roundtrip fidelity and duplicate-tier diagnostics.
    pub fn add_dependent_tier(mut self, tier: DependentTier) -> Self {
        self.dependent_tiers.push(tier.into());
        self
    }

    // ========================================================================
    // Convenience appenders for specific tier types (backward compatibility).
    // ========================================================================

    /// Append a `%mor` tier.
    pub fn with_mor(self, mor: MorTier) -> Self {
        self.add_dependent_tier(DependentTier::Mor(mor))
    }

    /// Append a `%gra` tier.
    pub fn with_gra(self, gra: GraTier) -> Self {
        self.add_dependent_tier(DependentTier::Gra(gra))
    }

    /// Append a `%sin` tier.
    pub fn with_sin(self, sin: SinTier) -> Self {
        self.add_dependent_tier(DependentTier::Sin(sin))
    }

    /// Append a `%com` tier.
    pub fn with_com(self, com: ComTier) -> Self {
        self.add_dependent_tier(DependentTier::Com(com))
    }
}
