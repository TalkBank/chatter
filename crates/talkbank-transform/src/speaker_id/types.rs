//! Typed newtypes for reference-mode confidence values.
//!
//! Reference-mode identification has a derived Jaccard score, a typed
//! winner/runner-up comparison, and an operator-supplied threshold. Bare
//! floating-point values would make these meanings interchangeable and would
//! require sentinels for no-information and unbounded comparisons. The types
//! here keep those states distinct.

use std::fmt;

/// Multiset-Jaccard similarity score in `[0.0, 1.0]`. `0.0` means
/// the two bags share nothing in common; `1.0` means they are
/// multiset-equal. Higher is more similar.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct JaccardScore(f64);

impl JaccardScore {
    pub(crate) fn from_ratio(shared: u64, union: u64) -> Self {
        if union == 0 {
            Self(0.0)
        } else {
            debug_assert!(shared <= union);
            Self(shared as f64 / union as f64)
        }
    }

    /// The derived score as a scalar for display or serialization.
    pub fn value(self) -> f64 {
        self.0
    }
}

impl fmt::Display for JaccardScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

/// A finite, positive winner→runner-up score ratio.
///
/// Its field is private so NaN, infinity, zero, and negative values cannot be
/// forged by consumers.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct FiniteConfidenceMargin(f64);

impl FiniteConfidenceMargin {
    /// The finite ratio as a scalar for display or serialization.
    pub fn value(self) -> f64 {
        self.0
    }
}

/// Winner→runner-up comparison as three states with different meanings.
///
/// A zero/zero comparison contains no lexical evidence. A positive winner
/// against a zero runner-up is unbounded. Neither state is represented by a
/// magic floating-point value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfidenceMargin {
    /// Both leading scores are zero.
    NoInformation,
    /// Both leading scores are positive.
    Finite(FiniteConfidenceMargin),
    /// The winner is positive and the runner-up is zero.
    Unbounded,
}

impl ConfidenceMargin {
    /// Construct a margin from two scores, ranking them inside the operation.
    pub fn from_scores(first: JaccardScore, second: JaccardScore) -> Self {
        let (winner, runner_up) = if first >= second {
            (first, second)
        } else {
            (second, first)
        };
        if runner_up.value() == 0.0 {
            if winner.value() > 0.0 {
                Self::Unbounded
            } else {
                Self::NoInformation
            }
        } else {
            let ratio = winner.value() / runner_up.value();
            debug_assert!(ratio.is_finite() && ratio >= 1.0);
            Self::Finite(FiniteConfidenceMargin(ratio))
        }
    }

    /// True when this margin is at least as large as `threshold`,
    /// i.e. the auto-decision is confident enough to stand.
    pub fn meets(self, threshold: ConfidenceThreshold) -> bool {
        match self {
            Self::NoInformation => false,
            Self::Finite(ratio) => ratio.value() >= threshold.0,
            Self::Unbounded => true,
        }
    }
}

impl fmt::Display for ConfidenceMargin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoInformation => write!(f, "no information"),
            Self::Finite(ratio) => write!(f, "{:.2}x", ratio.value()),
            Self::Unbounded => write!(f, "∞x"),
        }
    }
}

/// Operator-supplied minimum margin for auto-deciding in reference
/// mode. The CLI default is [`crate::speaker_id::DEFAULT_CONFIDENCE_THRESHOLD`].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ConfidenceThreshold(f64);

impl ConfidenceThreshold {
    /// Default threshold selected by the original calibration sweep.
    pub const DEFAULT: Self = Self(2.0);

    /// Admit a finite threshold of at least one.
    pub fn new(value: f64) -> Result<Self, ConfidenceThresholdError> {
        if value.is_finite() && value >= 1.0 {
            Ok(Self(value))
        } else {
            Err(ConfidenceThresholdError(value))
        }
    }

    /// Scalar form for CLI forwarding and serialization.
    pub fn value(self) -> f64 {
        self.0
    }
}

impl Default for ConfidenceThreshold {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::str::FromStr for ConfidenceThreshold {
    type Err = ParseConfidenceThresholdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let value = text.parse::<f64>()?;
        Self::new(value).map_err(ParseConfidenceThresholdError::Invalid)
    }
}

/// A scalar cannot represent a meaningful confidence threshold.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("confidence threshold must be finite and at least 1.0, got {0}")]
pub struct ConfidenceThresholdError(f64);

/// A command-line threshold is not a floating-point number or is outside the
/// valid confidence domain.
#[derive(Debug, thiserror::Error)]
pub enum ParseConfidenceThresholdError {
    /// Text was not a floating-point number.
    #[error("confidence threshold is not a number: {0}")]
    Float(#[from] std::num::ParseFloatError),
    /// Number parsed but is outside the confidence domain.
    #[error(transparent)]
    Invalid(ConfidenceThresholdError),
}

impl fmt::Display for ConfidenceThreshold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}x", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::ConfidenceThreshold;

    #[test]
    fn threshold_construction_rejects_values_that_cannot_mean_confidence() {
        assert!(ConfidenceThreshold::new(1.0).is_ok());
        assert!(ConfidenceThreshold::new(0.999).is_err());
        assert!(ConfidenceThreshold::new(f64::NAN).is_err());
        assert!(ConfidenceThreshold::new(f64::INFINITY).is_err());
    }
}
