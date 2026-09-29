//! Explicit review-draft convention, never inferred chronological authority.
use super::*;

/// The unresolved source frontier preserved for later listening review.
#[derive(Debug, Clone)]
pub enum DraftOrderReason {
    /// Neither evidence source determined the utterance interleaving.
    Utterances {
        /// Reference frontier coordinate.
        reference: MergeOrigin,
        /// Selected donor frontier coordinate.
        donor: MergeOrigin,
    },
    /// A section marker could not be positioned against the other source.
    Section {
        /// Preserved section marker text.
        header: String,
        /// Utterance from the competing source.
        competing: MergeOrigin,
        /// Previous recorded end, if available.
        previous_end: Option<u64>,
        /// Following recorded start, if available.
        next_start: Option<u64>,
    },
    /// Two section markers have no determined cross-source precedence.
    Sections {
        /// Reference section marker text.
        reference: String,
        /// Donor section marker text.
        donor: String,
    },
}

/// A boundary counted in output utterances, never CHAT lines or headers.
///
/// Constructed by assembly from its emitted utterances. Zero denotes the
/// boundary before the first utterance; the terminal boundary is also legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputUtteranceBoundary(usize);

impl OutputUtteranceBoundary {
    /// Number of output utterances preceding this boundary.
    pub fn utterances_before(self) -> usize {
        self.0
    }
}

/// A recorded convention choice for presentation outside the transcript.
#[derive(Debug, Clone)]
pub struct DraftOrderReview {
    /// Output utterance boundary at which the unresolved frontier was encountered.
    pub before_output_utterance: OutputUtteranceBoundary,
    /// Unresolved frontier; no source speech was removed by this choice.
    pub reason: DraftOrderReason,
}

#[derive(Clone, Copy)]
pub(super) enum DraftOrderPolicy {
    Strict,
    FlaggedReferenceFirst,
}

impl DraftOrderPolicy {
    pub(super) fn resolve(
        self,
        error: MergeError,
        boundary: usize,
    ) -> Result<DraftOrderReview, MergeError> {
        if matches!(self, Self::Strict) {
            return Err(error);
        }
        let reason = match error {
            MergeError::AmbiguousUtteranceOrder { reference, donor } => {
                DraftOrderReason::Utterances { reference, donor }
            }
            MergeError::AmbiguousSectionPlacement {
                header,
                competing,
                previous_end,
                next_start,
            } => DraftOrderReason::Section {
                header,
                competing,
                previous_end,
                next_start,
            },
            MergeError::AmbiguousSectionOrder { reference, donor } => {
                DraftOrderReason::Sections { reference, donor }
            }
            other => return Err(other),
        };
        Ok(DraftOrderReview {
            before_output_utterance: OutputUtteranceBoundary(boundary),
            reason,
        })
    }
}
