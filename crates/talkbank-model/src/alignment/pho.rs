//! Main-tier to `%pho` alignment.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::helpers::{TierDomain, TierPosition, to_chat_display_string as to_string};
use super::indices::{MainWordIndex, PhoItemIndex};
use super::traits::{AlignableTier, TierAlignmentResult, positional_align};
use super::types::AlignmentPair;
use crate::model::{MainTier, PhoTier, PhoTierType};
use crate::{ErrorCode, ParseError, Span};
use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// Typed pair for main↔`%pho` (and main↔`%mod`) alignment.
pub type PhoAlignmentPair = AlignmentPair<MainWordIndex, PhoItemIndex>;

/// Result of aligning main-tier units to `%pho` tokens.
///
/// `pairs` always preserves positional intent, including placeholder entries for
/// mismatches. `errors` carries user-facing diagnostics explaining why those
/// placeholders were needed.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift)]
pub struct PhoAlignment {
    /// Positional mapping rows ([`MainWordIndex`]↔[`PhoItemIndex`]).
    pub pairs: Vec<PhoAlignmentPair>,

    /// Diagnostics produced while enforcing count/position invariants.
    pub errors: Vec<ParseError>,
}

impl PhoAlignment {
    /// Creates an empty alignment accumulator.
    ///
    /// Used by the builder-style alignment loop before rows and diagnostics are appended.
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Appends one positional alignment row.
    ///
    /// This consumes and returns `Self` so call sites can chain in tight loops.
    pub fn with_pair(mut self, pair: PhoAlignmentPair) -> Self {
        self.pairs.push(pair);
        self
    }

    /// Appends one diagnostic describing an alignment mismatch.
    ///
    /// Multiple mismatches can be accumulated when callers choose to continue.
    pub fn with_error(mut self, error: ParseError) -> Self {
        self.errors.push(error);
        self
    }

    /// Returns `true` when alignment completed without mismatch diagnostics.
    ///
    /// A `true` value implies all rows in `pairs` are complete one-to-one matches.
    pub fn is_error_free(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for PhoAlignment {
    /// Builds an empty main-to-`%pho` alignment result.
    fn default() -> Self {
        Self::new()
    }
}

impl TierAlignmentResult for PhoAlignment {
    type Pair = PhoAlignmentPair;

    fn pairs(&self) -> &[PhoAlignmentPair] {
        &self.pairs
    }

    fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    fn push_pair(&mut self, pair: PhoAlignmentPair) {
        self.pairs.push(pair);
    }

    fn push_error(&mut self, error: ParseError) {
        self.errors.push(error);
    }
}

impl AlignableTier for PhoTier {
    type Source = MainWordIndex;
    type Target = PhoItemIndex;
    const DOMAIN: TierDomain = TierDomain::Pho;

    /// `%pho` and `%mod` are one type with a `tier_type`; every fact that
    /// differs between them (name, codes, suggestions) is read from it, so
    /// the same algorithm serves both and no caller passes codes in.
    fn tier_name(&self) -> &str {
        match self.tier_type {
            PhoTierType::Pho => "%pho tier",
            PhoTierType::Mod => "%mod tier",
        }
    }

    fn target_count(&self) -> usize {
        self.len()
    }

    fn extract_target_items(&self) -> Vec<TierPosition> {
        self.items
            .iter()
            .map(|token| TierPosition {
                text: to_string(token),
                description: None,
            })
            .collect()
    }

    fn span(&self) -> Span {
        self.span
    }

    fn error_code_too_few(&self) -> ErrorCode {
        match self.tier_type {
            PhoTierType::Pho => ErrorCode::PhoCountMismatchTooFew,
            PhoTierType::Mod => ErrorCode::ModCountMismatchTooFew,
        }
    }

    fn error_code_too_many(&self) -> ErrorCode {
        match self.tier_type {
            PhoTierType::Pho => ErrorCode::PhoCountMismatchTooMany,
            PhoTierType::Mod => ErrorCode::ModCountMismatchTooMany,
        }
    }

    fn suggestion_too_few(&self) -> &str {
        match self.tier_type {
            PhoTierType::Pho => "Add phonological tokens to %pho tier to match main tier words",
            PhoTierType::Mod => "Add phonological tokens to %mod tier to match main tier words",
        }
    }

    fn suggestion_too_many(&self) -> &str {
        match self.tier_type {
            PhoTierType::Pho => "Remove extra phonological tokens from %pho tier",
            PhoTierType::Mod => "Remove extra phonological tokens from %mod tier",
        }
    }
}

/// Align main-tier content to `%pho` or `%mod` tokens using 1:1 positional
/// pairing; the tier's own `tier_type` decides which codes a mismatch reports.
///
/// This pass enforces the contract that each alignable main-tier unit has
/// exactly one corresponding phonological token.
///
/// Uses the generic [`positional_align`] algorithm via the [`AlignableTier`]
/// implementation on [`PhoTier`]. The one owner of the algorithm: until
/// 2026-09-03 `metadata/alignment/count_based.rs` carried a second copy,
/// taking the codes as parameters because this route hardcoded `%pho`'s.
pub fn align_main_to_pho(main: &MainTier, pho: &PhoTier) -> PhoAlignment {
    let (pairs, errors) = positional_align(main, pho);
    PhoAlignment { pairs, errors }
}

#[cfg(test)]
mod pho_alignment_tests {
    use super::*;
    use crate::Span;
    use crate::model::{PhoTierType, Terminator, UtteranceContent, Word};

    fn main_of(words: &[&str]) -> MainTier {
        MainTier::new(
            "CHI",
            words
                .iter()
                .map(|w| UtteranceContent::Word(Box::new(Word::new_unchecked(*w, *w))))
                .collect(),
            Terminator::Period { span: Span::DUMMY },
        )
    }

    fn codes(alignment: &PhoAlignment) -> Vec<&str> {
        alignment.errors.iter().map(|e| e.code.as_str()).collect()
    }

    /// One token per word: complete pairs, no diagnostic, for either tier type.
    #[test]
    fn one_token_per_word_aligns_cleanly_for_pho_and_mod() {
        for tier_type in [PhoTierType::Pho, PhoTierType::Mod] {
            let tier = PhoTier::from_tokens(tier_type, vec!["wʌn".into(), "tuː".into()]);
            let alignment = align_main_to_pho(&main_of(&["one", "two"]), &tier);
            assert_eq!(alignment.pairs.len(), 2);
            assert!(alignment.pairs.iter().all(|p| p.is_complete()));
            assert!(alignment.errors.is_empty(), "{:?}", codes(&alignment));
        }
    }

    /// A `%pho` tier reports the `%pho` codes: E714 when short, E715 when long.
    #[test]
    fn pho_tier_reports_e714_and_e715() {
        let short = PhoTier::from_tokens(PhoTierType::Pho, vec!["wʌn".into()]);
        let alignment = align_main_to_pho(&main_of(&["one", "two"]), &short);
        assert_eq!(codes(&alignment), ["E714"]);
        assert_eq!(
            alignment.pairs.len(),
            2,
            "one complete pair and one placeholder"
        );

        let long = PhoTier::from_tokens(PhoTierType::Pho, vec!["wʌn".into(), "tuː".into()]);
        let alignment = align_main_to_pho(&main_of(&["one"]), &long);
        assert_eq!(codes(&alignment), ["E715"]);
    }

    /// A `%mod` tier is the SAME type with `tier_type = Mod`, and reports the
    /// `%mod` codes: E733 when short, E734 when long. Until 2026-09-03 the
    /// trait route hardcoded the `%pho` codes, which is why `compute.rs` kept
    /// its own copy of the algorithm with the codes passed as parameters.
    #[test]
    fn mod_tier_reports_e733_and_e734_not_the_pho_codes() {
        let short = PhoTier::from_tokens(PhoTierType::Mod, vec!["wʌn".into()]);
        let alignment = align_main_to_pho(&main_of(&["one", "two"]), &short);
        assert_eq!(codes(&alignment), ["E733"]);
        assert!(
            alignment.errors[0].message.contains("%mod"),
            "{}",
            alignment.errors[0].message
        );

        let long = PhoTier::from_tokens(PhoTierType::Mod, vec!["wʌn".into(), "tuː".into()]);
        let alignment = align_main_to_pho(&main_of(&["one"]), &long);
        assert_eq!(codes(&alignment), ["E734"]);
    }
}
