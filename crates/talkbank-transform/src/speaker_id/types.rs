//! Typed newtypes for reference-mode confidence values.
//!
//! Three closely-related `f64` values appear in reference-mode
//! identification, a raw Jaccard score, a winner/runner-up ratio
//! ("margin"), and the operator-supplied threshold the margin is
//! compared against. Sharing a primitive type makes them
//! interchangeable in signatures despite carrying different semantics;
//! the newtypes here enforce the distinction at the type-checker
//! level.

use std::fmt;

/// Multiset-Jaccard similarity score in `[0.0, 1.0]`. `0.0` means
/// the two bags share nothing in common; `1.0` means they are
/// multiset-equal. Higher is more similar.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct JaccardScore(pub f64);

impl fmt::Display for JaccardScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

/// Winner→runner-up score ratio (a multiplicative margin). A margin
/// of `2.0` means the winner scored twice as high as the runner-up;
/// `f64::INFINITY` means the runner-up scored 0 (winner takes
/// everything).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ConfidenceMargin(pub f64);

impl ConfidenceMargin {
    /// Construct a margin from a winner and runner-up score. When the
    /// runner-up scored 0, returns `INFINITY` if the winner is
    /// positive and `0.0` otherwise (no information either way).
    pub fn from_scores(winner: JaccardScore, runner_up: JaccardScore) -> Self {
        if runner_up.0 == 0.0 {
            ConfidenceMargin(if winner.0 > 0.0 { f64::INFINITY } else { 0.0 })
        } else {
            ConfidenceMargin(winner.0 / runner_up.0)
        }
    }

    /// True when this margin is at least as large as `threshold`,
    /// i.e. the auto-decision is confident enough to stand.
    pub fn meets(self, threshold: ConfidenceThreshold) -> bool {
        self.0 >= threshold.0
    }
}

impl fmt::Display for ConfidenceMargin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_infinite() {
            write!(f, "∞x")
        } else {
            write!(f, "{:.2}x", self.0)
        }
    }
}

/// Operator-supplied minimum margin for auto-deciding in reference
/// mode. The CLI default is [`crate::speaker_id::DEFAULT_CONFIDENCE_THRESHOLD`].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ConfidenceThreshold(pub f64);

impl fmt::Display for ConfidenceThreshold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}x", self.0)
    }
}
