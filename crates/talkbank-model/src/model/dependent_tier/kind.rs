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

/// Does a tier payload declare nothing: absent, or whitespace only?
///
/// ONE predicate, asked by every arm of [`DependentTier::declares_nothing`]
/// whose payload is an optional string.
fn declares_nothing_text(content: Option<&str>) -> bool {
    content.is_none_or(|text| text.trim().is_empty())
}

impl DependentTier {
    /// Does this tier line declare NO content?
    ///
    /// `true` means the file contains a tier line with nothing after the colon:
    /// `%eng:` or `%xfoo:` with an empty or whitespace-only payload. That is
    /// what E756 judges, and the rule was never `%x`-specific.
    ///
    /// `false` covers two cases on purpose, because E756 has nothing to say
    /// about either: the tier HAS content, or its type cannot represent
    /// emptiness at all. Every tier whose grammar body is free text is in the
    /// first group; the second is exactly the structured tiers, which parse
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
            Self::Wor(_) => false,

            // NOT a structured tier: `Unsupported` holds the same
            // `Option<NonEmptyString>` payload as `UserDefined` above, and its
            // body IS free text. It answers `false` for a different reason,
            // which is why it gets its own arm: E605 already rejects the tier
            // for having a name chatter does not know, and reporting E756 as
            // well would be a second code about the same unusable line.
            Self::Unsupported(_) => false,
        }
    }

    /// The span of a tier line that declares NO content.
    ///
    /// The two halves of the question are deliberately separate:
    /// [`Self::declares_nothing`] holds the exhaustive per-variant decision,
    /// and [`Self::span`] holds the per-variant span. Composing them here means
    /// there is exactly one of each to keep correct.
    #[must_use]
    pub fn empty_content_span(&self) -> Option<crate::Span> {
        self.declares_nothing().then(|| self.span())
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
            DependentTier::Mor(_) => "mor",
            DependentTier::Gra(_) => "gra",
            DependentTier::Pho(_) => "pho",
            DependentTier::Mod(_) => "mod",
            DependentTier::Sin(_) => "sin",
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
            DependentTier::Phoaln(_) => "phoaln",
            DependentTier::Xphoint(_) => "xphoint",
            DependentTier::Gls(_) => "gls",
            DependentTier::Ort(_) => "ort",
            DependentTier::Par(_) => "par",
            DependentTier::Tim(_) => "tim",
            DependentTier::Wor(_) => "wor",
            // User-defined tiers: label already includes 'x' prefix
            // e.g., %xmor stores label="xmor" to avoid collision with %mor
            DependentTier::UserDefined(tier) => &tier.label,
            // Unsupported tiers: label is the raw tier name (e.g., "foo" for %foo)
            DependentTier::Unsupported(tier) => &tier.label,
        }
    }

    /// Returns the source span associated with this dependent-tier value.
    ///
    /// This is used for diagnostics and provenance tracking; it does not affect
    /// semantic equality or serialization output. All variants expose spans
    /// through this accessor so caller code can stay enum-shape agnostic.
    pub fn span(&self) -> crate::Span {
        match self {
            DependentTier::Mor(t) => t.span,
            DependentTier::Gra(t) => t.span,
            DependentTier::Pho(t) => t.span,
            DependentTier::Mod(t) => t.span,
            DependentTier::Sin(t) => t.span,
            DependentTier::Act(t) => t.span,
            DependentTier::Cod(t) => t.span,
            DependentTier::Add(t) => t.span,
            DependentTier::Com(t) => t.span,
            DependentTier::Exp(t) => t.span,
            DependentTier::Gpx(t) => t.span,
            DependentTier::Int(t) => t.span,
            DependentTier::Sit(t) => t.span,
            DependentTier::Spa(t) => t.span,
            DependentTier::Alt(t) => t.span,
            DependentTier::Coh(t) => t.span,
            DependentTier::Def(t) => t.span,
            DependentTier::Eng(t) => t.span,
            DependentTier::Err(t) => t.span,
            DependentTier::Fac(t) => t.span,
            DependentTier::Flo(t) => t.span,
            DependentTier::Modsyl(t) => t.span,
            DependentTier::Phosyl(t) => t.span,
            DependentTier::Phoaln(t) => t.span,
            DependentTier::Xphoint(t) => t.span,
            DependentTier::Gls(t) => t.span,
            DependentTier::Ort(t) => t.span,
            DependentTier::Par(t) => t.span,
            DependentTier::Tim(t) => t.span(),
            DependentTier::Wor(t) => t.span,
            DependentTier::UserDefined(t) => t.span,
            DependentTier::Unsupported(t) => t.span,
        }
    }
}

#[cfg(test)]
mod empty_content_span_tests {
    use crate::model::NonEmptyString;
    use crate::model::dependent_tier::types::{DependentTier, TextTier, UserDefinedDependentTier};

    fn text(content: &str) -> DependentTier {
        DependentTier::Eng(TextTier::new(
            NonEmptyString::new(content).expect("test content is non-empty"),
        ))
    }

    /// The case the 2026-08-15 widening added, and the reason it existed:
    /// re2c read a file with an empty `%eng:` as VALID because nothing judged
    /// the state the model had just learned to represent.
    #[test]
    fn an_empty_standard_text_tier_is_empty() {
        assert!(
            DependentTier::Eng(TextTier::empty())
                .empty_content_span()
                .is_some()
        );
    }

    /// Whitespace-only counts, which is what the `%x` rule always meant by
    /// "declares nothing".
    ///
    /// Walks both routes to `declares_nothing`: a standard text tier and a
    /// `%x` tier, the case the rule was originally written for.
    #[test]
    fn whitespace_only_is_empty() {
        assert!(text(" \t").empty_content_span().is_some());

        let ws = UserDefinedDependentTier {
            label: NonEmptyString::new("xtst").expect("label is non-empty"),
            content: Some(NonEmptyString::new(" \t").expect("whitespace is non-empty")),
            span: crate::Span::DUMMY,
        };
        assert!(
            DependentTier::UserDefined(ws)
                .empty_content_span()
                .is_some(),
            "a whitespace-only %x tier declares nothing, which is the case E756 \
             was written for"
        );
    }

    #[test]
    fn a_tier_with_content_is_not_empty() {
        assert!(text("real annotation").empty_content_span().is_none());
    }

    /// The original `%x*` behaviour, unchanged by the widening. These three
    /// cases moved here from `validation::unparsed_tier` when the emptiness
    /// DECISION moved out of the reporter.
    #[test]
    fn user_defined_tiers_keep_their_original_verdicts() {
        let with_content = UserDefinedDependentTier {
            label: NonEmptyString::new("xfoo").expect("label is non-empty"),
            content: Some(NonEmptyString::new("test content").expect("content is non-empty")),
            span: crate::Span::DUMMY,
        };
        assert!(
            DependentTier::UserDefined(with_content)
                .empty_content_span()
                .is_none()
        );

        let declares_nothing = UserDefinedDependentTier {
            label: NonEmptyString::new("xfoo").expect("label is non-empty"),
            content: None,
            span: crate::Span::DUMMY,
        };
        assert!(
            DependentTier::UserDefined(declares_nothing)
                .empty_content_span()
                .is_some()
        );
    }

    /// A structured tier cannot BE empty, so the rule has nothing to say about
    /// it. Asserted rather than assumed, because the exhaustive match in
    /// `empty_content_span` is what routes it here and a future variant added
    /// to the wrong arm would silently change this.
    #[test]
    fn a_structured_tier_is_never_reported_empty() {
        use crate::model::Terminator;
        use crate::model::dependent_tier::mor::tier::MorTier;
        let tier = MorTier::new_mor(
            Vec::new(),
            Terminator::Period {
                span: crate::Span::DUMMY,
            },
        );
        assert!(DependentTier::Mor(tier).empty_content_span().is_none());
    }
}
