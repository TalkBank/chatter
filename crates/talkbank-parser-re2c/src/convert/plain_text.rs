//! Admit the closed set of plain-text tier labels before shared-model construction.

use talkbank_model::{DependentTier, Location, RecoveryText, TextTier};

/// Only these dependent tiers carry the shared plain-text payload.
#[derive(Clone, Copy)]
pub(super) enum PlainTextTierKind {
    Alt,
    Coh,
    Def,
    Eng,
    Err,
    Fac,
    Flo,
    Gls,
    Ort,
    Par,
}

impl PlainTextTierKind {
    pub(super) fn from_label(label: &str) -> Option<Self> {
        Some(match label {
            "alt" => Self::Alt,
            "coh" => Self::Coh,
            "def" => Self::Def,
            "eng" => Self::Eng,
            "err" => Self::Err,
            "fac" => Self::Fac,
            "flo" => Self::Flo,
            "gls" => Self::Gls,
            "ort" => Self::Ort,
            "par" => Self::Par,
            _ => return None,
        })
    }

    /// Empty observed content remains explicit recovery, not invented whitespace.
    pub(super) fn lower(self, location: Location, content: &str) -> DependentTier {
        let text = match RecoveryText::new(content) {
            Ok(text) => TextTier::new(location, text),
            Err(_) => TextTier::empty(location),
        };
        match self {
            Self::Alt => DependentTier::Alt(text),
            Self::Coh => DependentTier::Coh(text),
            Self::Def => DependentTier::Def(text),
            Self::Eng => DependentTier::Eng(text),
            Self::Err => DependentTier::Err(text),
            Self::Fac => DependentTier::Fac(text),
            Self::Flo => DependentTier::Flo(text),
            Self::Gls => DependentTier::Gls(text),
            Self::Ort => DependentTier::Ort(text),
            Self::Par => DependentTier::Par(text),
        }
    }
}
