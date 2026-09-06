//! File lines and dependent-tier entries retain source separator provenance.

use super::{DependentTierParsed, HeaderParsed, MainTier};
use serde::Serialize;

/// A parsed CHAT file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatFile<'a> {
    pub lines: Vec<Line<'a>>,
    /// Original source text, needed for lossless raw_text reconstruction via spans.
    pub source: &'a str,
}

/// A line in a CHAT file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Line<'a> {
    Header {
        header: HeaderParsed<'a>,
        #[serde(skip)]
        separator: talkbank_model::model::TierSeparator,
    },
    Utterance(Box<Utterance<'a>>),
}

/// An utterance: main tier + dependent tiers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Utterance<'a> {
    pub main_tier: MainTier<'a>,
    pub dependent_tiers: Vec<DependentTierEntryParsed<'a>>,
}

/// A parsed dependent tier retains the separator admitted with its prefix.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DependentTierEntryParsed<'a> {
    pub tier: DependentTierParsed<'a>,
    #[serde(skip)]
    pub separator: talkbank_model::model::TierSeparator,
}
