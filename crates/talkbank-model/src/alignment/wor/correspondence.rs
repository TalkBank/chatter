//! Lexical corroboration for count-matched `%wor` timing.
//!
//! Count equality permits a positional comparison, but it cannot detect a
//! same-count edit. This module refines that weak state by comparing the
//! canonical display token for every selected main-tier word with the token
//! physically stored in the corresponding `%wor` slot. The `%wor` token is
//! corroborating evidence only. Lexical identity continues to come from the
//! typed main tier.

use super::{
    CountMatchedWorTimings, WorRecordedInterval, WorSlotIndex, WorSlotMembershipPolicy,
    WorSlotTiming,
};
use crate::model::Word;

/// Result of checking canonical display-token correspondence after counts match.
///
/// Only [`Corroborated`](Self::Corroborated) exposes positional timing slots.
/// This prevents a same-count edit from silently attaching an old `%wor`
/// interval to a different main-tier word. Corroboration is stronger than a
/// count match, but it still does not claim acoustic boundary quality.
#[derive(Debug, PartialEq)]
#[must_use = "lexical correspondence must be handled before `%wor` timing is used"]
pub enum WorTimingCorrespondence<'source> {
    /// Every `%wor` display token matched the canonical token derived from the
    /// corresponding main-tier word.
    Corroborated(CorroboratedWorTimings<'source>),
    /// At least one display token differed, so no timing slots are exposed.
    Uncorroborated(UncorroboratedWorTimings<'source>),
}

/// Main-tier slots with count-matched and lexically corroborated `%wor` timing.
///
/// Construction is private. The only public transition is
/// [`corroborate_wor_timing`].
#[derive(Debug, PartialEq)]
pub struct CorroboratedWorTimings<'source> {
    pub(super) policy: WorSlotMembershipPolicy,
    pub(super) slots: Vec<CorroboratedWorTimingSlot<'source>>,
}

impl<'source> CorroboratedWorTimings<'source> {
    /// Membership policy whose projection was corroborated.
    pub fn membership_policy(&self) -> WorSlotMembershipPolicy {
        self.policy
    }

    /// Corroborated slots in main-tier order.
    pub fn slots(&self) -> &[CorroboratedWorTimingSlot<'source>] {
        &self.slots
    }
}

/// One main-tier word plus timing from a canonically matching `%wor` slot.
#[derive(Debug, PartialEq)]
pub struct CorroboratedWorTimingSlot<'source> {
    pub(super) main_word: &'source Word,
    pub(super) timing: WorSlotTiming,
}

impl<'source> CorroboratedWorTimingSlot<'source> {
    /// Typed main-tier word that owns lexical identity.
    pub fn main_word(&self) -> &'source Word {
        self.main_word
    }

    /// Cleaned lexical text from the typed main-tier word.
    pub fn main_text(&self) -> &str {
        self.main_word.cleaned_text()
    }

    /// Timing observation from the corroborating `%wor` slot.
    pub fn timing(&self) -> WorSlotTiming {
        self.timing
    }
}

/// Evidence that a count-matched pair failed canonical lexical corroboration.
///
/// No positional timing slots are exposed from this state. The mismatches are
/// retained only to explain why reuse was refused.
#[derive(Debug, PartialEq)]
pub struct UncorroboratedWorTimings<'source> {
    policy: WorSlotMembershipPolicy,
    mismatches: Vec<WorLexicalMismatch<'source>>,
}

impl<'source> UncorroboratedWorTimings<'source> {
    /// Membership policy whose projection was compared.
    pub fn membership_policy(&self) -> WorSlotMembershipPolicy {
        self.policy
    }

    /// Exhaustive canonical display-token mismatches.
    pub fn mismatches(&self) -> &[WorLexicalMismatch<'source>] {
        &self.mismatches
    }
}

/// One count-matched position whose display token did not corroborate.
///
/// References are retained instead of copying strings. Accessors expose only
/// diagnostic text so the `%wor` word cannot become lexical processing input.
#[derive(Debug, PartialEq)]
pub struct WorLexicalMismatch<'source> {
    slot: WorSlotIndex,
    main_word: &'source Word,
    wor_word: &'source Word,
}

impl WorLexicalMismatch<'_> {
    /// Position under the selected main-tier membership policy.
    pub fn slot(&self) -> WorSlotIndex {
        self.slot
    }

    /// Canonical cleaned display text derived from the main tier.
    pub fn main_text(&self) -> &str {
        super::projection::canonical_wor_display_text(self.main_word)
    }

    /// Display text physically recorded in the parsed `%wor` slot.
    pub fn wor_text(&self) -> &str {
        self.wor_word.cleaned_text()
    }
}

/// Refine equal slot counts into canonical lexical correspondence.
///
/// The comparison uses the same display-token owner as `%wor` generation.
/// `%wor` text can therefore refuse unsafe reuse, but can never replace the
/// main-tier word as lexical identity.
pub fn corroborate_wor_timing(
    count_matched: CountMatchedWorTimings<'_>,
) -> WorTimingCorrespondence<'_> {
    let CountMatchedWorTimings {
        policy,
        main_slots,
        wor_slots,
    } = count_matched;
    let mut mismatches = Vec::new();

    for (raw_index, (main_word, wor_word)) in main_slots.iter().zip(&wor_slots).enumerate() {
        if super::projection::canonical_wor_display_text(main_word) != wor_word.cleaned_text() {
            mismatches.push(WorLexicalMismatch {
                slot: WorSlotIndex(raw_index),
                main_word,
                wor_word,
            });
        }
    }

    if !mismatches.is_empty() {
        return WorTimingCorrespondence::Uncorroborated(UncorroboratedWorTimings {
            policy,
            mismatches,
        });
    }

    let slots = main_slots
        .into_iter()
        .zip(wor_slots)
        .map(|(main_word, wor_word)| CorroboratedWorTimingSlot {
            main_word,
            timing: match wor_word.inline_bullet.as_ref() {
                Some(bullet) => WorSlotTiming::Timed(WorRecordedInterval::from_bullet(bullet)),
                None => WorSlotTiming::Unaligned,
            },
        })
        .collect();

    WorTimingCorrespondence::Corroborated(CorroboratedWorTimings { policy, slots })
}

#[cfg(test)]
mod tests;
