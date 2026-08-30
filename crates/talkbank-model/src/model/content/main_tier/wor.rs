//! `%wor` projection and generation for main tiers.

use super::MainTier;
use crate::alignment::WorMainTierProjection;
use crate::model::WorTier;

impl MainTier {
    /// Project this main tier through the canonical `%wor` membership policy.
    ///
    /// The returned capability is the single selection source for both
    /// serializable `%wor` generation and positional timing binding. Its
    /// constructor is not exposed independently, so a projection cannot be
    /// assembled from unrelated words.
    pub fn wor_projection(&self) -> WorMainTierProjection<'_> {
        WorMainTierProjection::from_main(self)
    }

    /// Generate a flat `%wor` tier from the canonical main-tier projection.
    ///
    /// Selected main-tier words supply display text and embedded timing.
    /// Tag-marker separators retain their order. The generated tier does not
    /// carry the utterance-level bullet because that remains on the main tier.
    pub fn generate_wor_tier(&self) -> WorTier {
        self.wor_projection().generate_tier()
    }
}
