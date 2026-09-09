//! Domain selector for alignment counting and matching helpers.
//!
//! Each dependent tier applies slightly different alignment eligibility rules
//! over the same main-tier content. This enum makes those policy branches
//! explicit so helper APIs can stay deterministic and auditable.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

/// The tiers aligned by POSITION against main-tier units: `%mor`, `%pho`
/// and `%sin`. This is the domain a count or an extraction takes.
///
/// `%wor` is deliberately not a value. Its correspondence with the main tier
/// is [`WorMainTierProjection`]'s, which answers the count through
/// `bind_timing` and the words through `corroborate_wor_timing`; a count or
/// an extraction that could be asked a `%wor` question would answer it with
/// a second walk, and on 2026-09-08 two such arms and one downstream walker
/// were found agreeing with the projection only by test. One such walk is
/// still open: `helpers::overlap` measures overlap-marker positions on the
/// `%wor` scale with its own traversal, and is the next closure. Converting
/// to [`TierDomain`] is how a positional domain reaches the descent and
/// membership rules, which serve every tier.
///
/// [`WorMainTierProjection`]: crate::alignment::WorMainTierProjection
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PositionalDomain {
    /// Morphological analysis alignment (`%mor`).
    Mor,
    /// Phonological alignment (`%pho`).
    Pho,
    /// Sign/speech-act alignment (`%sin`).
    Sin,
}

/// The failed conversion below: the domain was `%wor`, the one tier that is
/// not positional. A unit, because a payload could only ever hold that one
/// value.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NotPositional;

impl TryFrom<TierDomain> for PositionalDomain {
    type Error = NotPositional;

    /// `%wor` is the one tier a count cannot be asked about; see the type
    /// doc for who owns it.
    fn try_from(domain: TierDomain) -> Result<Self, NotPositional> {
        match domain {
            TierDomain::Mor => Ok(PositionalDomain::Mor),
            TierDomain::Pho => Ok(PositionalDomain::Pho),
            TierDomain::Sin => Ok(PositionalDomain::Sin),
            TierDomain::Wor => Err(NotPositional),
        }
    }
}

impl From<PositionalDomain> for TierDomain {
    fn from(domain: PositionalDomain) -> Self {
        match domain {
            PositionalDomain::Mor => TierDomain::Mor,
            PositionalDomain::Pho => TierDomain::Pho,
            PositionalDomain::Sin => TierDomain::Sin,
        }
    }
}

/// The tier whose eligibility rules a walk, a descent verdict or a word
/// membership question is asked under.
///
/// The same utterance content can produce different alignment-unit counts for
/// different tiers. For example, `%pho` may count pauses that `%mor` skips.
/// Counting and extraction take [`PositionalDomain`], which has no `%wor`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TierDomain {
    /// Morphological analysis alignment (`%mor`).
    ///
    /// Uses morpheme-oriented counting rules and skips content that has no
    /// morphological interpretation (for example, retraced material).
    Mor,
    /// Phonological alignment (`%pho`).
    ///
    /// Uses pronunciation-oriented counting rules, which are intentionally more
    /// permissive for produced speech content.
    Pho,
    /// Sign/speech-act alignment (`%sin`).
    ///
    /// Follows `%sin` grouping semantics while still mapping back to the same
    /// main-tier index domain used by other aligners.
    Sin,
    /// Word-timing membership (`%wor`).
    ///
    /// Primarily follows `%pho`-like word counting but keeps `%wor`-specific
    /// exclusions (such as timestamp-shaped tokens). Reached by the walks,
    /// the descent rule and the membership rule; the tier's count and
    /// pairing are
    /// [`WorMainTierProjection`](crate::alignment::WorMainTierProjection)'s,
    /// and `helpers::overlap` still measures marker positions on this scale
    /// with a traversal of its own (recorded as the next closure).
    Wor,
}
