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
        header: Box<HeaderParsed<'a>>,
        #[serde(skip)]
        provenance: HeaderProvenance,
    },
    Utterance(Box<Utterance<'a>>),
}

/// An utterance: main tier + dependent tiers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Utterance<'a> {
    pub main_tier: LocatedMainTier<'a>,
    pub dependent_tiers: Vec<DependentTierEntryParsed<'a>>,
}

/// A main tier together with the locations the line machine measured for
/// it. The chumsky main-tier grammar sees tokens only, and after recovery it
/// may see a filtered stream whose indices no longer match the lexer's, so
/// only the line machine, which owns tokens and spans together, can place the
/// line, its speaker and its content. Constructed there and nowhere else.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LocatedMainTier<'a> {
    #[serde(flatten)]
    tier: MainTier<'a>,
    #[serde(skip)]
    locations: talkbank_model::MainTierLocations,
}

impl<'a> LocatedMainTier<'a> {
    pub(crate) fn new(tier: MainTier<'a>, locations: talkbank_model::MainTierLocations) -> Self {
        Self { tier, locations }
    }

    pub fn tier(&self) -> &MainTier<'a> {
        &self.tier
    }

    /// The line including its ending, the speaker and the content region
    /// (from the end of the separator's colon to the end of the line).
    pub fn locations(&self) -> talkbank_model::MainTierLocations {
        self.locations
    }
}

/// A parsed dependent tier retains the separator admitted with its prefix.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DependentTierEntryParsed<'a> {
    pub tier: DependentTierParsed<'a>,
    #[serde(skip)]
    pub separator: talkbank_model::model::TierSeparator,
    #[serde(skip)]
    pub(crate) content_range:
        Option<talkbank_model::model::dependent_tier::DependentTierContentSpan>,
    /// The tier line's extent, from its `%label` prefix through its last
    /// content token before the line ending.
    #[serde(skip)]
    pub(crate) extent: talkbank_model::Span,
}

/// Source extent and separator carried together from a lexed header line.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderProvenance {
    span: talkbank_model::Span,
    separator: talkbank_model::model::TierSeparator,
}

impl HeaderProvenance {
    pub(crate) fn from_lexed(
        span: talkbank_model::Span,
        separator: talkbank_model::model::TierSeparator,
    ) -> Self {
        Self { span, separator }
    }

    /// Complete logical header extent, including its line terminator.
    pub fn span(&self) -> talkbank_model::Span {
        self.span
    }

    /// Separator recognized with this header's prefix.
    pub fn separator(&self) -> talkbank_model::model::TierSeparator {
        self.separator
    }
}
