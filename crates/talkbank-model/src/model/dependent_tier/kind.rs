//! Tier-kind and span helpers for `DependentTier`.
//!
//! These helpers provide stable identifiers used by duplicate detection,
//! indexing, and diagnostics without exposing enum-internal pattern matching
//! at call sites.
//! The `kind` accessor normalizes user-defined labels so `%xfoo` tiers do not
//! collide with standard names.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::DependentTier;

/// Borrowed identity of a dependent-tier slot, independent of parse success.
///
/// Constructed only from a tier's canonical label. Parsed and recovery payloads
/// share a key; distinct custom labels remain distinct. This is an identity for
/// replacement and indexing, not a certificate that the payload is valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DependentTierKey<'a>(&'a str);

/// Does a tier payload declare nothing: absent, or whitespace only?
///
/// ONE predicate, asked by every arm of [`DependentTier::declares_nothing`]
/// whose payload is an optional string.
fn declares_nothing_text(content: Option<&str>) -> bool {
    content.is_none_or(|text| text.trim().is_empty())
}

impl DependentTier {
    /// Structured text payload and its enclosing diagnostic location.
    pub(crate) fn bullet_content(&self) -> Option<(&super::BulletContent, crate::Location)> {
        match self {
            Self::Act(tier) => Some((&tier.content, tier.location)),
            Self::Cod(tier) => Some((&tier.content, tier.location)),
            Self::Add(tier) => Some((&tier.content, tier.location)),
            Self::Com(tier) => Some((&tier.content, tier.location)),
            Self::Exp(tier) => Some((&tier.content, tier.location)),
            Self::Gpx(tier) => Some((&tier.content, tier.location)),
            Self::Int(tier) => Some((&tier.content, tier.location)),
            Self::Sit(tier) => Some((&tier.content, tier.location)),
            Self::Spa(tier) => Some((&tier.content, tier.location)),
            Self::UnparsedMor(_)
            | Self::UnparsedGra(_)
            | Self::UnparsedPho(_)
            | Self::UnparsedMod(_)
            | Self::UnparsedSin(_)
            | Self::UnparsedWor(_)
            | Self::UnparsedPhoaln(_)
            | Self::UnparsedText(_)
            | Self::UnparsedXphoint(_)
            | Self::Mor(_)
            | Self::Gra(_)
            | Self::Pho(_)
            | Self::Mod(_)
            | Self::Sin(_)
            | Self::Alt(_)
            | Self::Coh(_)
            | Self::Def(_)
            | Self::Eng(_)
            | Self::Err(_)
            | Self::Fac(_)
            | Self::Flo(_)
            | Self::Modsyl(_)
            | Self::Phosyl(_)
            | Self::Phoaln(_)
            | Self::Xphoint(_)
            | Self::Gls(_)
            | Self::Ort(_)
            | Self::Par(_)
            | Self::Tim(_)
            | Self::Wor(_)
            | Self::UserDefined(_)
            | Self::Unsupported(_) => None,
        }
    }

    /// One exhaustive owner for raw recovery and the alignment trust it withdraws.
    pub(crate) fn unparsed_recovery(&self) -> Option<super::unparsed::UnparsedRecovery<'_>> {
        use super::unparsed::UnparsedRecovery;
        use crate::model::ParseHealthTier;
        match self {
            Self::UnparsedText(text) => Some(UnparsedRecovery::Text(text.raw())),
            Self::UnparsedMor(raw) => Some(UnparsedRecovery::Aligned(ParseHealthTier::Mor, raw)),
            Self::UnparsedGra(raw) => Some(UnparsedRecovery::Aligned(ParseHealthTier::Gra, raw)),
            Self::UnparsedPho(raw) => Some(UnparsedRecovery::Aligned(ParseHealthTier::Pho, raw)),
            Self::UnparsedMod(raw) => Some(UnparsedRecovery::Aligned(ParseHealthTier::Mod, raw)),
            Self::UnparsedSin(raw) => Some(UnparsedRecovery::Aligned(ParseHealthTier::Sin, raw)),
            Self::UnparsedWor(raw) => Some(UnparsedRecovery::Aligned(ParseHealthTier::Wor, raw)),
            Self::UnparsedPhoaln(raw) => {
                Some(UnparsedRecovery::Aligned(ParseHealthTier::Phoaln, raw))
            }
            Self::UnparsedXphoint(raw) => {
                Some(UnparsedRecovery::Aligned(ParseHealthTier::Xphoint, raw))
            }
            Self::Mor(_)
            | Self::Gra(_)
            | Self::Pho(_)
            | Self::Mod(_)
            | Self::Sin(_)
            | Self::Act(_)
            | Self::Cod(_)
            | Self::Add(_)
            | Self::Com(_)
            | Self::Exp(_)
            | Self::Gpx(_)
            | Self::Int(_)
            | Self::Sit(_)
            | Self::Spa(_)
            | Self::Alt(_)
            | Self::Coh(_)
            | Self::Def(_)
            | Self::Eng(_)
            | Self::Err(_)
            | Self::Fac(_)
            | Self::Flo(_)
            | Self::Modsyl(_)
            | Self::Phosyl(_)
            | Self::Phoaln(_)
            | Self::Xphoint(_)
            | Self::Gls(_)
            | Self::Ort(_)
            | Self::Par(_)
            | Self::Tim(_)
            | Self::Wor(_)
            | Self::UserDefined(_)
            | Self::Unsupported(_) => None,
        }
    }

    /// Does this tier line declare NO content?
    ///
    /// `true` means the file contains a tier line with nothing after the colon:
    /// `%eng:` or `%xfoo:` with an empty or whitespace-only payload. That is
    /// what E756 judges, and the rule was never `%x`-specific.
    ///
    /// `false` covers contentful tiers, structured tiers whose emptiness is
    /// diagnosed earlier, and unsupported labels where E605 suppresses E756.
    /// Recognized free-text tiers can report emptiness; structured tiers parse
    /// their payload into typed items, so a tier with no payload fails earlier
    /// and more specifically than "you declared nothing".
    ///
    /// The match is exhaustive with no catch-all, so a tier variant added to
    /// [`DependentTier`] stops compiling here until someone decides which of
    /// the two answers it gives. It deliberately does NOT return the span:
    /// [`Self::span`] already owns the variant-to-span mapping, and a second
    /// copy of it here would be a list nothing keeps in step with the first.
    #[must_use]
    pub fn declares_nothing(&self) -> bool {
        match self {
            // The ten text-payload tiers. `TextTier::content` is an `Option`
            // precisely so this question can be asked; before it was, a parser
            // meeting `%eng:` had to invent a payload.
            Self::Alt(t)
            | Self::Coh(t)
            | Self::Def(t)
            | Self::Eng(t)
            | Self::Err(t)
            | Self::Fac(t)
            | Self::Flo(t)
            | Self::Gls(t)
            | Self::Ort(t)
            | Self::Par(t) => declares_nothing_text(t.content.as_deref()),

            // User-defined `%x*` tiers already modelled it this way, and were
            // the only ones E756 was wired to until 2026-08-15.
            Self::UserDefined(tier) => declares_nothing_text(tier.content.as_deref()),

            // The nine bullet-payload tiers. Each asks its OWN `is_empty`
            // rather than reaching through to `content`, so this arm routes and
            // the type decides; `define_text_tier!` generates seven of these
            // and `ActTier` / `CodTier` hand-write the same method.
            Self::Add(t) => t.is_empty(),
            Self::Com(t) => t.is_empty(),
            Self::Exp(t) => t.is_empty(),
            Self::Gpx(t) => t.is_empty(),
            Self::Int(t) => t.is_empty(),
            Self::Sit(t) => t.is_empty(),
            Self::Spa(t) => t.is_empty(),
            Self::Act(t) => t.is_empty(),
            Self::Cod(t) => t.is_empty(),

            // The five that finished the widening on 2026-08-16. They have
            // free-text grammar bodies exactly as every tier above does, so
            // E756's rule always reached them; they were excluded for one day
            // only because no payload here could yet SAY it was empty.
            // `TimTier` grew an `Empty` variant (its two content variants both
            // hold a `NonEmptyString`, so emptiness needed a state, not a
            // field), while the three Phon tiers derive `is_empty` from the
            // word or group count they already reported.
            Self::Tim(t) => declares_nothing_text(t.declared_content()),
            Self::Modsyl(t) | Self::Phosyl(t) => t.is_empty(),
            Self::Phoaln(t) => t.is_empty(),
            Self::Xphoint(t) => t.is_empty(),

            // Structured tiers: the grammar body is not free text
            // (`mor_contents`, `gra_contents`, `pho_groups`, `sin_groups`,
            // `wor_tier_body`), so a tier with no payload fails earlier and
            // more specifically than "you declared nothing".
            Self::Mor(_) | Self::Gra(_) | Self::Pho(_) | Self::Mod(_) | Self::Sin(_) => false,
            Self::Wor(_)
            | Self::UnparsedMor(_)
            | Self::UnparsedGra(_)
            | Self::UnparsedPho(_)
            | Self::UnparsedMod(_)
            | Self::UnparsedSin(_)
            | Self::UnparsedWor(_)
            | Self::UnparsedPhoaln(_)
            | Self::UnparsedText(_)
            | Self::UnparsedXphoint(_) => false,

            // NOT a structured tier: `Unsupported` holds the same
            // `Option<RecoveryText>` payload as `UserDefined` above, and its
            // body IS free text. It answers `false` for a different reason,
            // which is why it gets its own arm: E605 already rejects the tier
            // for having a name chatter does not know, and reporting E756 as
            // well would be a second code about the same unusable line.
            Self::Unsupported(_) => false,
        }
    }

    /// The same tier with its enclosing span re-expressed through `project`.
    ///
    /// For fragment owners that parse a tier inside a wrapper they built and
    /// then clamp its span to the caller's input before moving it into the
    /// caller's coordinates. Payload and child locations are moved by
    /// [`SpanShift`](crate::SpanShift). An unlocated tier stays unlocated by
    /// construction: there is no span to project, and so no way to attach a
    /// location to a tier built without one.
    #[must_use]
    pub fn map_location(self, project: impl FnOnce(crate::Span) -> crate::Span) -> Self {
        match self {
            Self::UnparsedText(t) => Self::UnparsedText(t.map_location(project)),
            Self::Mor(mut t) => {
                t.location = t.location.map(project);
                Self::Mor(t)
            }
            Self::UnparsedMor(mut t) => {
                t.location = t.location.map(project);
                Self::UnparsedMor(t)
            }
            Self::UnparsedGra(mut t) => {
                t.location = t.location.map(project);
                Self::UnparsedGra(t)
            }
            Self::UnparsedPho(mut t) => {
                t.location = t.location.map(project);
                Self::UnparsedPho(t)
            }
            Self::UnparsedMod(mut t) => {
                t.location = t.location.map(project);
                Self::UnparsedMod(t)
            }
            Self::UnparsedSin(mut t) => {
                t.location = t.location.map(project);
                Self::UnparsedSin(t)
            }
            Self::UnparsedWor(mut t) => {
                t.location = t.location.map(project);
                Self::UnparsedWor(t)
            }
            Self::UnparsedPhoaln(mut t) => {
                t.location = t.location.map(project);
                Self::UnparsedPhoaln(t)
            }
            Self::UnparsedXphoint(mut t) => {
                t.location = t.location.map(project);
                Self::UnparsedXphoint(t)
            }
            Self::Gra(mut t) => {
                t.location = t.location.map(project);
                Self::Gra(t)
            }
            Self::Pho(mut t) => {
                t.location = t.location.map(project);
                Self::Pho(t)
            }
            Self::Mod(mut t) => {
                t.location = t.location.map(project);
                Self::Mod(t)
            }
            Self::Sin(mut t) => {
                t.location = t.location.map(project);
                Self::Sin(t)
            }
            Self::Act(mut t) => {
                t.location = t.location.map(project);
                Self::Act(t)
            }
            Self::Cod(mut t) => {
                t.location = t.location.map(project);
                Self::Cod(t)
            }
            Self::Add(mut t) => {
                t.location = t.location.map(project);
                Self::Add(t)
            }
            Self::Com(mut t) => {
                t.location = t.location.map(project);
                Self::Com(t)
            }
            Self::Exp(mut t) => {
                t.location = t.location.map(project);
                Self::Exp(t)
            }
            Self::Gpx(mut t) => {
                t.location = t.location.map(project);
                Self::Gpx(t)
            }
            Self::Int(mut t) => {
                t.location = t.location.map(project);
                Self::Int(t)
            }
            Self::Sit(mut t) => {
                t.location = t.location.map(project);
                Self::Sit(t)
            }
            Self::Spa(mut t) => {
                t.location = t.location.map(project);
                Self::Spa(t)
            }
            Self::Alt(mut t) => {
                t.location = t.location.map(project);
                Self::Alt(t)
            }
            Self::Coh(mut t) => {
                t.location = t.location.map(project);
                Self::Coh(t)
            }
            Self::Def(mut t) => {
                t.location = t.location.map(project);
                Self::Def(t)
            }
            Self::Eng(mut t) => {
                t.location = t.location.map(project);
                Self::Eng(t)
            }
            Self::Err(mut t) => {
                t.location = t.location.map(project);
                Self::Err(t)
            }
            Self::Fac(mut t) => {
                t.location = t.location.map(project);
                Self::Fac(t)
            }
            Self::Flo(mut t) => {
                t.location = t.location.map(project);
                Self::Flo(t)
            }
            Self::Modsyl(mut t) => {
                t.location = t.location.map(project);
                Self::Modsyl(t)
            }
            Self::Phosyl(mut t) => {
                t.location = t.location.map(project);
                Self::Phosyl(t)
            }
            Self::Phoaln(mut t) => {
                t.location = t.location.map(project);
                Self::Phoaln(t)
            }
            Self::Xphoint(mut t) => {
                t.location = t.location.map(project);
                Self::Xphoint(t)
            }
            Self::Gls(mut t) => {
                t.location = t.location.map(project);
                Self::Gls(t)
            }
            Self::Ort(mut t) => {
                t.location = t.location.map(project);
                Self::Ort(t)
            }
            Self::Par(mut t) => {
                t.location = t.location.map(project);
                Self::Par(t)
            }
            Self::Wor(mut t) => {
                t.location = t.location.map(project);
                Self::Wor(t)
            }
            Self::UserDefined(mut t) => {
                t.location = t.location.map(project);
                Self::UserDefined(t)
            }
            Self::Unsupported(mut t) => {
                t.location = t.location.map(project);
                Self::Unsupported(t)
            }
            Self::Tim(t) => Self::Tim(t.map_location(project)),
        }
    }

    /// The location of a tier line that declares NO content.
    ///
    /// The two halves of the question are deliberately separate:
    /// [`Self::declares_nothing`] holds the exhaustive per-variant decision,
    /// and [`Self::span`] holds the per-variant span. Composing them here means
    /// there is exactly one of each to keep correct.
    #[must_use]
    pub fn empty_content_location(&self) -> Option<crate::Location> {
        self.declares_nothing().then(|| self.location())
    }

    /// Identity shared by parsed and recovery payloads of the same tier slot.
    pub fn key(&self) -> DependentTierKey<'_> {
        DependentTierKey(self.kind())
    }

    /// Returns the canonical dependent-tier identifier used in CHAT tags.
    ///
    /// For standard tiers this is the lowercase suffix (`"mor"`, `"gra"`, ...).
    /// For user-defined tiers this returns the stored custom label (including
    /// the leading `x`), preserving `%x*` namespace semantics. Callers should
    /// prefer this helper over ad-hoc pattern matching when building maps or
    /// duplicate-detection keys.
    pub fn kind(&self) -> &str {
        match self {
            DependentTier::UnparsedText(t) => t.label(),
            DependentTier::Mor(_) | DependentTier::UnparsedMor(_) => "mor",
            DependentTier::Gra(_) | DependentTier::UnparsedGra(_) => "gra",
            DependentTier::Pho(_) | DependentTier::UnparsedPho(_) => "pho",
            DependentTier::Mod(_) | DependentTier::UnparsedMod(_) => "mod",
            DependentTier::Sin(_) | DependentTier::UnparsedSin(_) => "sin",
            DependentTier::Act(_) => "act",
            DependentTier::Cod(_) => "cod",
            DependentTier::Add(_) => "add",
            DependentTier::Com(_) => "com",
            DependentTier::Exp(_) => "exp",
            DependentTier::Gpx(_) => "gpx",
            DependentTier::Int(_) => "int",
            DependentTier::Sit(_) => "sit",
            DependentTier::Spa(_) => "spa",
            DependentTier::Alt(_) => "alt",
            DependentTier::Coh(_) => "coh",
            DependentTier::Def(_) => "def",
            DependentTier::Eng(_) => "eng",
            DependentTier::Err(_) => "err",
            DependentTier::Fac(_) => "fac",
            DependentTier::Flo(_) => "flo",
            DependentTier::Modsyl(_) => "modsyl",
            DependentTier::Phosyl(_) => "phosyl",
            DependentTier::Phoaln(_) | DependentTier::UnparsedPhoaln(_) => "phoaln",
            DependentTier::Xphoint(_) | DependentTier::UnparsedXphoint(_) => "xphoint",
            DependentTier::Gls(_) => "gls",
            DependentTier::Ort(_) => "ort",
            DependentTier::Par(_) => "par",
            DependentTier::Tim(_) => "tim",
            DependentTier::Wor(_) | DependentTier::UnparsedWor(_) => "wor",
            // User-defined tiers: label already includes 'x' prefix
            // e.g., %xmor stores label="xmor" to avoid collision with %mor
            DependentTier::UserDefined(tier) => &tier.label,
            // Unsupported tiers: label is the raw tier name (e.g., "foo" for %foo)
            DependentTier::Unsupported(tier) => &tier.label,
        }
    }

    /// Returns enclosing source metadata without affecting semantic equality
    /// or serialization. All variants expose it through this owner.
    pub fn location(&self) -> crate::Location {
        match self {
            DependentTier::UnparsedText(t) => t.raw().location(),
            DependentTier::Mor(t) => t.location,
            DependentTier::UnparsedMor(t)
            | DependentTier::UnparsedGra(t)
            | DependentTier::UnparsedPho(t)
            | DependentTier::UnparsedMod(t)
            | DependentTier::UnparsedSin(t)
            | DependentTier::UnparsedWor(t)
            | DependentTier::UnparsedPhoaln(t)
            | DependentTier::UnparsedXphoint(t) => t.location(),
            DependentTier::Gra(t) => t.location,
            DependentTier::Pho(t) => t.location,
            DependentTier::Mod(t) => t.location,
            DependentTier::Sin(t) => t.location,
            DependentTier::Act(t) => t.location,
            DependentTier::Cod(t) => t.location,
            DependentTier::Add(t) => t.location,
            DependentTier::Com(t) => t.location,
            DependentTier::Exp(t) => t.location,
            DependentTier::Gpx(t) => t.location,
            DependentTier::Int(t) => t.location,
            DependentTier::Sit(t) => t.location,
            DependentTier::Spa(t) => t.location,
            DependentTier::Alt(t) => t.location,
            DependentTier::Coh(t) => t.location,
            DependentTier::Def(t) => t.location,
            DependentTier::Eng(t) => t.location,
            DependentTier::Err(t) => t.location,
            DependentTier::Fac(t) => t.location,
            DependentTier::Flo(t) => t.location,
            DependentTier::Modsyl(t) => t.location,
            DependentTier::Phosyl(t) => t.location,
            DependentTier::Phoaln(t) => t.location,
            DependentTier::Xphoint(t) => t.location,
            DependentTier::Gls(t) => t.location,
            DependentTier::Ort(t) => t.location,
            DependentTier::Par(t) => t.location,
            DependentTier::Tim(t) => t.location(),
            DependentTier::Wor(t) => t.location,
            DependentTier::UserDefined(t) => t.location,
            DependentTier::Unsupported(t) => t.location,
        }
    }
}
