//! Main-tier to `%sin` alignment.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::helpers::{PositionalDomain, TierPosition, to_chat_display_string as to_string};
use super::indices::{MainWordIndex, SinItemIndex};
use super::traits::{AlignableTier, TierAlignmentResult, positional_align};
use super::types::AlignmentPair;
use crate::model::{MainTier, SinTier};
use crate::{ErrorCode, ParseError, Span};
use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// Typed pair for main↔`%sin` alignment.
pub type SinAlignmentPair = AlignmentPair<MainWordIndex, SinItemIndex>;

/// Result of aligning main tier words to %sin tier gesture/sign tokens.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift)]
pub struct SinAlignment {
    /// Alignment pairs ([`MainWordIndex`]↔[`SinItemIndex`])
    pub pairs: Vec<SinAlignmentPair>,

    /// Errors produced while checking `%sin` count/position alignment.
    pub errors: Vec<ParseError>,
}

impl SinAlignment {
    /// Create an empty alignment with no pairs or errors.
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Append an alignment pair.
    pub fn with_pair(mut self, pair: SinAlignmentPair) -> Self {
        self.pairs.push(pair);
        self
    }

    /// Append an alignment error.
    pub fn with_error(mut self, error: ParseError) -> Self {
        self.errors.push(error);
        self
    }

    /// Returns `true` when no alignment diagnostics were emitted.
    pub fn is_error_free(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for SinAlignment {
    /// Builds an empty main-to-`%sin` alignment result.
    fn default() -> Self {
        Self::new()
    }
}

impl TierAlignmentResult for SinAlignment {
    type Pair = SinAlignmentPair;

    fn pairs(&self) -> &[SinAlignmentPair] {
        &self.pairs
    }

    fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    fn push_pair(&mut self, pair: SinAlignmentPair) {
        self.pairs.push(pair);
    }

    fn push_error(&mut self, error: ParseError) {
        self.errors.push(error);
    }
}

impl AlignableTier for SinTier {
    type Source = MainWordIndex;
    type Target = SinItemIndex;
    const DOMAIN: PositionalDomain = PositionalDomain::Sin;

    fn tier_name(&self) -> &str {
        "%sin tier"
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
        ErrorCode::SinCountMismatchTooFew
    }

    fn error_code_too_many(&self) -> ErrorCode {
        ErrorCode::SinCountMismatchTooMany
    }

    fn suggestion_too_few(&self) -> &str {
        "Add gesture/sign tokens to %sin tier to match main tier words"
    }

    fn suggestion_too_many(&self) -> &str {
        "Remove extra gesture/sign tokens from %sin tier"
    }
}

/// Align main-tier content to `%sin` tokens using 1:1 positional pairing.
///
/// Uses the generic [`positional_align`] algorithm via the [`AlignableTier`]
/// implementation on [`SinTier`].
pub fn align_main_to_sin(main: &MainTier, sin: &SinTier) -> SinAlignment {
    let (pairs, errors) = positional_align(main, sin);
    SinAlignment { pairs, errors }
}

// =============================================================================
// Tests for %sin alignment
// =============================================================================

#[cfg(test)]
mod sin_alignment_tests {
    //! The one `%sin` alignment state no parse produces.
    //!
    //! Five tests that used to sit here built both tiers by hand; they are
    //! `talkbank-parser-tests/tests/integration/sin_alignment_from_source.rs`
    //! now, over parsed tiers. This one stays: a `%sin:` line with no token
    //! is E342 at parse, so empty on empty is reachable only from a tier
    //! built by hand, and building it is the only way to reach it.
    use super::*;
    use crate::Span;
    use crate::model::Terminator;

    /// Accepts empty-on-empty alignment without diagnostics.
    #[test]
    fn test_sin_alignment_empty() {
        let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY });
        let sin = SinTier::new(vec![]);

        let alignment = align_main_to_sin(&main, &sin);

        assert_eq!(alignment.pairs.len(), 0);
        assert!(alignment.errors.is_empty());
    }
}
