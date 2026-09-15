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

/// A recorded convention choice, alongside a visible comment in the output.
#[derive(Debug, Clone)]
pub struct DraftOrderReview {
    /// Zero-based output utterance boundary where the review comment appears.
    pub before_output_utterance: usize,
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
            before_output_utterance: boundary,
            reason,
        })
    }
}

impl DraftOrderReview {
    pub(super) fn comment(&self) -> String {
        let source = |origin: MergeOrigin| match origin {
            MergeOrigin::Retained(index) => {
                format!("reference utterance {}", index.utterance().raw() + 1)
            }
            MergeOrigin::Inserted(index) => {
                format!("selected donor utterance {}", index.utterance().raw() + 1)
            }
        };
        let header = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        let time = |value: Option<u64>| match value {
            Some(ms) => format!("{}:{:02}.{:03}", ms / 60000, (ms / 1000) % 60, ms % 1000),
            None => "unavailable".into(),
        };
        let frontier = match &self.reason {
            DraftOrderReason::Utterances { reference, donor } => {
                format!("{} versus {}", source(*reference), source(*donor))
            }
            DraftOrderReason::Section {
                header: marker,
                competing,
                previous_end,
                next_start,
            } => format!(
                "{} versus {}; preceding utterance end {}, following utterance start {} (navigation bounds, not inferred marker times)",
                header(marker),
                source(*competing),
                time(*previous_end),
                time(*next_start)
            ),
            DraftOrderReason::Sections { reference, donor } => format!(
                "reference marker {} versus donor marker {}",
                header(reference),
                header(donor)
            ),
        };
        format!(
            "REVIEW GENERATED: Ambiguous cross-source ordering before output utterance {}. Reference-first draft serialization preserves both source sequences and all speech; it does not establish chronology or task membership. Listen and adjudicate this interleaving: {frontier}.",
            self.before_output_utterance + 1
        )
    }
}
