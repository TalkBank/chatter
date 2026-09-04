//! Reference-mode identification: pick the donor speaker matching a
//! reference anchor by multiset-Jaccard text similarity.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use talkbank_model::SpeakerCode;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{ChatFile, Line};

use crate::PipelineError;

use super::error::SpeakerIdError;
use super::types::{ConfidenceMargin, ConfidenceThreshold, JaccardScore};

/// Minimum number of distinct donor speakers required for
/// reference-mode identification. With fewer than two speakers there
/// is nothing for the multiset-Jaccard step to choose between, the
/// operator should fall back to explicit-mapping mode.
const MIN_DONOR_SPEAKERS: usize = 2;

/// Default confidence threshold for reference mode: the
/// winner→runner-up Jaccard margin must be at least 2.0× for the
/// auto-decision to stand. Below threshold the operator is asked to
/// adjudicate. Picked empirically from a pilot validation sweep on
/// real two-speaker clinical recordings: clean-winner sessions sat at
/// margins >= 2.6x while ambiguous/mixed sessions sat <= 1.95x, so
/// 2.0x separates the two populations with headroom on both sides.
pub const DEFAULT_CONFIDENCE_THRESHOLD: ConfidenceThreshold = ConfidenceThreshold::DEFAULT;

/// Lexical-ranking outcome produced before the confidence policy accepts or
/// refuses an automatic speaker decision.
///
/// Carries the per-speaker support that derives every score, plus the
/// winner→runner-up margin. Fields are private so a winner, evidence map, and
/// margin from different ranking runs cannot be recombined into a report that
/// never existed.
#[derive(Debug, Clone)]
pub struct DonorMatchReport {
    /// Donor speaker whose token bag best matched the reference
    /// anchor. In the eventual mapping this speaker is marked for
    /// `Drop`, the reference covers them authoritatively, so the
    /// downstream merge will pull their utterances from the reference
    /// rather than the donor.
    winner: SpeakerCode,

    /// Absolute lexical evidence for every donor speaker against the
    /// reference anchor bag. The Jaccard score is derived from these
    /// counts, so callers cannot receive a score whose support has been
    /// discarded or drifted.
    evidence: HashMap<SpeakerCode, LexicalMatchEvidence>,

    /// Winner→runner-up ratio. On the success path always
    /// satisfies `margin.meets(threshold)`.
    margin: ConfidenceMargin,
}

impl DonorMatchReport {
    /// Donor speaker selected by the ranking step.
    pub fn winner(&self) -> &SpeakerCode {
        &self.winner
    }

    /// Winner-to-runner-up confidence state derived from this report's scores.
    pub fn margin(&self) -> ConfidenceMargin {
        self.margin
    }

    /// Lexical evidence for one donor speaker, when that speaker participated
    /// in the ranking.
    pub fn evidence_for(&self, speaker: &SpeakerCode) -> Option<LexicalMatchEvidence> {
        self.evidence.get(speaker).copied()
    }

    /// Iterate over every donor speaker's lexical support without exposing a
    /// mutable report representation.
    pub fn lexical_evidence(
        &self,
    ) -> impl ExactSizeIterator<Item = (&SpeakerCode, LexicalMatchEvidence)> + '_ {
        self.evidence
            .iter()
            .map(|(speaker, evidence)| (speaker, *evidence))
    }

    /// Render the typed Jaccard scores into the on-disk override-file
    /// shape (deterministic `BTreeMap<String, f64>`, sorted by
    /// speaker code).
    pub fn scores_to_serializable(&self) -> BTreeMap<String, f64> {
        self.evidence
            .iter()
            .map(|(speaker, evidence)| (speaker.as_str().to_string(), evidence.score().value()))
            .collect()
    }

    /// Render the typed margin into the on-disk override-file shape.
    /// `None` when the runner-up scored zero AND the winner also
    /// scored zero (no information); `Some(INFINITY)` when the
    /// winner alone took everything.
    pub fn margin_to_serializable(&self) -> Option<f64> {
        match self.margin {
            ConfidenceMargin::NoInformation => None,
            ConfidenceMargin::Finite(ratio) => Some(ratio.value()),
            ConfidenceMargin::Unbounded => Some(f64::INFINITY),
        }
    }

    /// Produce the stable, serialization-ready evidence view used by CLI and
    /// experiment tooling.
    pub fn record(&self) -> RecordedDonorMatchReport {
        RecordedDonorMatchReport {
            schema_version: 1,
            winner: self.winner.as_str().to_owned(),
            margin: RecordedConfidenceMargin::from(self.margin),
            speakers: self
                .evidence
                .iter()
                .map(|(speaker, evidence)| {
                    (
                        speaker.as_str().to_owned(),
                        RecordedLexicalMatchEvidence::from(*evidence),
                    )
                })
                .collect(),
        }
    }
}

/// Stable serialization boundary for one reference-mode match report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordedDonorMatchReport {
    /// Wire schema for this report shape.
    pub schema_version: u8,
    /// Donor speaker selected by the ranking step.
    pub winner: String,
    /// Typed winner-to-runner-up comparison.
    pub margin: RecordedConfidenceMargin,
    /// Per-speaker support keyed in deterministic speaker-code order.
    pub speakers: BTreeMap<String, RecordedLexicalMatchEvidence>,
}

/// Stable evidence boundary for every outcome reached after both CHAT files
/// have parsed successfully.
///
/// The tagged outcome keeps matched evidence, structural refusals, and their
/// outcome-specific fields from being combined into impossible records.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordedSpeakerIdentificationAttempt {
    /// Wire schema for the complete attempt record.
    pub schema_version: u8,
    /// Outcome-specific evidence.
    #[serde(flatten)]
    pub outcome: RecordedSpeakerIdentificationOutcome,
}

impl RecordedSpeakerIdentificationAttempt {
    /// Record an accepted lexical match.
    pub fn accepted(report: &DonorMatchReport, threshold: ConfidenceThreshold) -> Self {
        Self {
            schema_version: 1,
            outcome: RecordedSpeakerIdentificationOutcome::Accepted {
                threshold: threshold.value(),
                match_report: report.record(),
            },
        }
    }

    /// Record a match that was observed but refused by its threshold.
    pub fn low_confidence(report: &DonorMatchReport, threshold: ConfidenceThreshold) -> Self {
        Self {
            schema_version: 1,
            outcome: RecordedSpeakerIdentificationOutcome::LowConfidence {
                threshold: threshold.value(),
                match_report: report.record(),
            },
        }
    }

    /// Record that the requested reference anchor had no utterances.
    pub fn reference_missing_anchor(anchor: &SpeakerCode) -> Self {
        Self {
            schema_version: 1,
            outcome: RecordedSpeakerIdentificationOutcome::ReferenceMissingAnchor {
                anchor: anchor.as_str().to_owned(),
            },
        }
    }

    /// Record that the donor did not offer at least two speaker tracks.
    pub fn donor_too_few_speakers(speakers: &[SpeakerCode]) -> Self {
        Self {
            schema_version: 1,
            outcome: RecordedSpeakerIdentificationOutcome::DonorTooFewSpeakers {
                speakers: speakers
                    .iter()
                    .map(|speaker| speaker.as_str().to_owned())
                    .collect(),
            },
        }
    }

    /// Record that a parsed-input precondition failed before lexical matching
    /// could begin.
    pub fn input_rejected(
        input: RecordedSpeakerIdentificationInput,
        error: &PipelineError,
    ) -> Self {
        let (failure_kind, diagnostic_codes) = match error {
            PipelineError::Io(_) => (RecordedInputFailureKind::Io, Vec::new()),
            PipelineError::ParserCreation(_) => {
                (RecordedInputFailureKind::ParserCreation, Vec::new())
            }
            PipelineError::Parse(errors) => (
                RecordedInputFailureKind::Parse,
                errors
                    .errors
                    .iter()
                    .map(|error| error.code.to_string())
                    .collect(),
            ),
            PipelineError::Validation(errors) => (
                RecordedInputFailureKind::Validation,
                errors.iter().map(|error| error.code.to_string()).collect(),
            ),
            PipelineError::IncompleteValidation(failure) => (
                RecordedInputFailureKind::Validation,
                failure
                    .diagnostics()
                    .iter()
                    .map(|error| error.code.to_string())
                    .collect(),
            ),
            PipelineError::JsonSerialization(_) => {
                (RecordedInputFailureKind::JsonSerialization, Vec::new())
            }
            PipelineError::DroppedContent(_) => {
                (RecordedInputFailureKind::DroppedContent, Vec::new())
            }
        };
        Self {
            schema_version: 1,
            outcome: RecordedSpeakerIdentificationOutcome::InputRejected {
                input,
                failure_kind,
                diagnostic_codes,
            },
        }
    }
}

/// Which CHAT input failed before reference-mode matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedSpeakerIdentificationInput {
    /// Transcript whose anonymous speakers would be identified.
    Donor,
    /// Transcript providing the known anchor speaker.
    Reference,
}

/// Typed pipeline failure category retained by a rejected input attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedInputFailureKind {
    /// An I/O operation inside the typed pipeline failed.
    Io,
    /// The selected parser backend could not be created.
    ParserCreation,
    /// CHAT parsing emitted one or more diagnostics.
    Parse,
    /// The parsed CHAT model failed validation.
    Validation,
    /// An internal JSON serialization boundary failed.
    JsonSerialization,
    /// A rewrite-safety check found content that would be lost.
    DroppedContent,
}

/// Closed set of reference-mode attempt outcomes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RecordedSpeakerIdentificationOutcome {
    /// The match met its threshold.
    Accepted {
        /// Threshold applied to this observation.
        threshold: f64,
        /// Complete lexical evidence.
        match_report: RecordedDonorMatchReport,
    },
    /// The match was observed but fell below its threshold.
    LowConfidence {
        /// Threshold applied to this observation.
        threshold: f64,
        /// Complete lexical evidence.
        match_report: RecordedDonorMatchReport,
    },
    /// The requested anchor had no reference utterances.
    ReferenceMissingAnchor {
        /// Requested reference speaker code.
        anchor: String,
    },
    /// The donor had fewer than two speaker tracks.
    DonorTooFewSpeakers {
        /// Donor speaker codes encountered in document order.
        speakers: Vec<String>,
    },
    /// A donor or reference CHAT input did not pass parsing and validation.
    InputRejected {
        /// Which input was rejected.
        input: RecordedSpeakerIdentificationInput,
        /// Stable pipeline failure category.
        failure_kind: RecordedInputFailureKind,
        /// Machine-readable TalkBank diagnostic codes, when available.
        diagnostic_codes: Vec<String>,
    },
}

/// Serialization-safe form of the confidence-margin typestate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecordedConfidenceMargin {
    /// Both leading speakers had zero lexical overlap.
    NoInformation,
    /// Both leading speakers had positive scores.
    Finite {
        /// Winner score divided by runner-up score.
        ratio: f64,
    },
    /// The winner had positive overlap and the runner-up had none.
    Unbounded,
}

impl From<ConfidenceMargin> for RecordedConfidenceMargin {
    fn from(value: ConfidenceMargin) -> Self {
        match value {
            ConfidenceMargin::NoInformation => Self::NoInformation,
            ConfidenceMargin::Finite(ratio) => Self::Finite {
                ratio: ratio.value(),
            },
            ConfidenceMargin::Unbounded => Self::Unbounded,
        }
    }
}

/// Serialization-ready lexical support for one donor speaker.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RecordedLexicalMatchEvidence {
    /// Qualifying tokens in the reference anchor bag.
    pub reference_tokens: u64,
    /// Qualifying tokens in this donor speaker's bag.
    pub donor_tokens: u64,
    /// Multiset intersection size.
    pub shared_tokens: u64,
    /// Multiset union size.
    pub union_tokens: u64,
    /// Jaccard score derived from `shared_tokens / union_tokens`.
    pub score: f64,
}

impl From<LexicalMatchEvidence> for RecordedLexicalMatchEvidence {
    fn from(value: LexicalMatchEvidence) -> Self {
        Self {
            reference_tokens: value.reference_tokens(),
            donor_tokens: value.donor_tokens(),
            shared_tokens: value.shared_tokens(),
            union_tokens: value.union_tokens(),
            score: value.score().value(),
        }
    }
}

/// The multiset counts from which one donor speaker's Jaccard score is
/// derived.
///
/// Construction is private to the bag-comparison boundary. Consumers can
/// inspect support and derive the score, but cannot pair an arbitrary score
/// with unrelated counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LexicalMatchEvidence {
    reference_tokens: u64,
    donor_tokens: u64,
    shared_tokens: u64,
}

impl LexicalMatchEvidence {
    fn from_bags(a: &HashMap<String, u32>, b: &HashMap<String, u32>) -> Self {
        let reference_tokens = a.values().map(|count| u64::from(*count)).sum();
        let donor_tokens = b.values().map(|count| u64::from(*count)).sum();
        let mut vocab: HashSet<&str> = HashSet::with_capacity(a.len() + b.len());
        for token in a.keys().chain(b.keys()) {
            vocab.insert(token.as_str());
        }
        let shared_tokens = vocab
            .into_iter()
            .map(|token| {
                let reference_count = u64::from(*a.get(token).unwrap_or(&0));
                let donor_count = u64::from(*b.get(token).unwrap_or(&0));
                reference_count.min(donor_count)
            })
            .sum();
        Self {
            reference_tokens,
            donor_tokens,
            shared_tokens,
        }
    }

    /// Number of qualifying reference-anchor tokens, including repetitions.
    pub fn reference_tokens(self) -> u64 {
        self.reference_tokens
    }

    /// Number of qualifying tokens attributed to this donor speaker.
    pub fn donor_tokens(self) -> u64 {
        self.donor_tokens
    }

    /// Multiset intersection size.
    pub fn shared_tokens(self) -> u64 {
        self.shared_tokens
    }

    /// Multiset union size, derived from the three retained observations.
    pub fn union_tokens(self) -> u64 {
        self.reference_tokens + self.donor_tokens - self.shared_tokens
    }

    /// Multiset-Jaccard score derived from the retained counts.
    pub fn score(self) -> JaccardScore {
        JaccardScore::from_ratio(self.shared_tokens, self.union_tokens())
    }
}

/// Multiset Jaccard similarity over two token-count maps:
/// |A ∩ B| / |A ∪ B| where ∩ and ∪ use per-token min and max counts.
///
/// The returned evidence derives a `0.0` score when both bags are empty or
/// when their vocabularies are disjoint. Retaining both bag sizes keeps those
/// two semantically different states distinguishable to evidence consumers.
fn jaccard(a: &HashMap<String, u32>, b: &HashMap<String, u32>) -> LexicalMatchEvidence {
    LexicalMatchEvidence::from_bags(a, b)
}

/// Build the content-token bag for `speaker` across `chat`. Uses
/// `walk_words` to traverse each utterance's AST and collect Word
/// leaves' cleaned text. Separators and replaced-word annotations are
/// skipped (they carry no lexical content). Tokens are lowercased and
/// filtered to alphabetic-only forms of length ≥ 2, matching the
/// validated Python prototype's cleaner so the algorithm carries over
/// the same empirical recall guarantee.
///
/// A speaker with no utterances, or whose utterances contain no
/// qualifying tokens after filtering, yields an empty bag, that
/// speaker then scores 0.0 against any reference bag and will fail
/// any sane confidence threshold downstream.
fn speaker_bag(chat: &ChatFile, speaker: &SpeakerCode) -> HashMap<String, u32> {
    let mut bag: HashMap<String, u32> = HashMap::new();
    for line in chat.lines.as_slice().iter() {
        if let Line::Utterance(u) = line
            && &u.main.speaker == speaker
        {
            walk_words(&u.main.content.content, None, &mut |item| {
                if let WordItem::Word(w) = item {
                    let token = clean_token(w.cleaned_text());
                    if !token.is_empty() {
                        *bag.entry(token).or_insert(0) += 1;
                    }
                }
            });
        }
    }
    bag
}

/// Normalize a raw word's cleaned text to a Jaccard-comparable token:
/// lowercase, alphabetic-only, length ≥ 2. Returns the empty string
/// when the token doesn't qualify (the caller skips empties).
///
/// This matches the Python prototype's `clean_text_for_matching`
/// post-walk filter, relying on the AST's `cleaned_text` to have
/// already stripped CHAT markup means the regex pipeline collapses to
/// this one normalization step.
fn clean_token(raw: &str) -> String {
    let lowered = raw.trim().to_ascii_lowercase();
    if lowered.len() < 2 {
        return String::new();
    }
    if !lowered.chars().all(|c| c.is_ascii_alphabetic()) {
        return String::new();
    }
    lowered
}

/// Reference-mode identification: pick the donor speaker whose token
/// bag best matches the reference anchor's bag, refusing when the
/// winner→runner-up margin is below `threshold`.
///
/// On success returns a [`DonorMatchReport`] carrying the winner,
/// per-speaker lexical support, derived scores, and the margin (always ≥
/// `threshold` on the success path). On `margin < threshold` returns
/// [`SpeakerIdError::LowConfidence`] with the same evidence and computed
/// margin so the operator can adjudicate.
///
/// Callers using the empirically-picked default should pass
/// [`DEFAULT_CONFIDENCE_THRESHOLD`]; the CLI layer surfaces this as
/// `--confidence-threshold` so operators can override per-corpus.
pub fn identify_mapping(
    reference: &ChatFile,
    anchor: &SpeakerCode,
    donor: &ChatFile,
    threshold: ConfidenceThreshold,
) -> Result<DonorMatchReport, SpeakerIdError> {
    let ref_bag = speaker_bag(reference, anchor);
    if ref_bag.is_empty() {
        return Err(SpeakerIdError::ReferenceMissingAnchor {
            anchor: anchor.clone(),
        });
    }

    let donor_speakers = donor.unique_utterance_speakers();
    if donor_speakers.len() < MIN_DONOR_SPEAKERS {
        return Err(SpeakerIdError::DonorTooFewSpeakers {
            speakers: donor_speakers,
        });
    }

    let mut evidence: HashMap<SpeakerCode, LexicalMatchEvidence> = HashMap::new();
    for spk in donor_speakers.iter() {
        evidence.insert(spk.clone(), jaccard(&ref_bag, &speaker_bag(donor, spk)));
    }

    // Sort donors by descending Jaccard score. Ties break on the
    // donor's document-order position (already captured in
    // `donor_speakers`); the resulting order is deterministic across
    // runs given the same input.
    let mut ranked: Vec<&SpeakerCode> = donor_speakers.iter().collect();
    ranked.sort_by(|a, b| {
        evidence[*b]
            .score()
            .partial_cmp(&evidence[*a].score())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let winner = (*ranked[0]).clone();
    let winner_score = evidence[&winner].score();
    let runner_up_score = evidence[ranked[1]].score();
    let margin = ConfidenceMargin::from_scores(winner_score, runner_up_score);

    let report = DonorMatchReport {
        winner,
        evidence,
        margin,
    };

    if !margin.meets(threshold) {
        return Err(SpeakerIdError::LowConfidence { report, threshold });
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::jaccard;

    #[test]
    fn lexical_match_retains_the_counts_that_produce_its_score() {
        let reference = HashMap::from([("frog".to_owned(), 2), ("jumped".to_owned(), 1)]);
        let donor = HashMap::from([("frog".to_owned(), 1), ("pond".to_owned(), 2)]);

        let evidence = jaccard(&reference, &donor);

        assert_eq!(evidence.reference_tokens(), 3);
        assert_eq!(evidence.donor_tokens(), 3);
        assert_eq!(evidence.shared_tokens(), 1);
        assert_eq!(evidence.union_tokens(), 5);
        assert_eq!(evidence.score().value(), 0.2);
    }

    #[test]
    fn recorded_report_preserves_support_and_margin_state() {
        let reference = HashMap::from([("frog".to_owned(), 2)]);
        let winner_bag = HashMap::from([("frog".to_owned(), 1)]);
        let runner_up_bag = HashMap::from([("pond".to_owned(), 1)]);
        let winner = talkbank_model::SpeakerCode::new("PAR0");
        let runner_up = talkbank_model::SpeakerCode::new("PAR1");
        let winner_evidence = jaccard(&reference, &winner_bag);
        let runner_up_evidence = jaccard(&reference, &runner_up_bag);
        let report = super::DonorMatchReport {
            winner: winner.clone(),
            evidence: HashMap::from([(winner, winner_evidence), (runner_up, runner_up_evidence)]),
            margin: super::ConfidenceMargin::from_scores(
                winner_evidence.score(),
                runner_up_evidence.score(),
            ),
        };

        let recorded = report.record();

        assert_eq!(recorded.schema_version, 1);
        assert_eq!(recorded.winner, "PAR0");
        assert_eq!(recorded.margin, super::RecordedConfidenceMargin::Unbounded);
        assert_eq!(recorded.speakers["PAR0"].shared_tokens, 1);
        assert_eq!(recorded.speakers["PAR1"].shared_tokens, 0);
    }

    #[test]
    fn recorded_attempt_uses_outcome_specific_shapes() {
        let missing = super::RecordedSpeakerIdentificationAttempt::reference_missing_anchor(
            &talkbank_model::SpeakerCode::new("CHI"),
        );
        let too_few = super::RecordedSpeakerIdentificationAttempt::donor_too_few_speakers(&[
            talkbank_model::SpeakerCode::new("PAR0"),
        ]);

        let missing_json =
            serde_json::to_value(missing).expect("missing-anchor attempt serializes");
        let too_few_json = serde_json::to_value(too_few).expect("too-few attempt serializes");

        assert_eq!(missing_json["outcome"], "reference_missing_anchor");
        assert_eq!(missing_json["anchor"], "CHI");
        assert!(missing_json.get("match_report").is_none());
        assert_eq!(too_few_json["outcome"], "donor_too_few_speakers");
        assert_eq!(too_few_json["speakers"][0], "PAR0");
        assert!(too_few_json.get("threshold").is_none());

        let rejected = super::RecordedSpeakerIdentificationAttempt::input_rejected(
            super::RecordedSpeakerIdentificationInput::Reference,
            &crate::PipelineError::ParserCreation("unavailable".to_owned()),
        );
        let rejected_json =
            serde_json::to_value(rejected).expect("input-rejected attempt serializes");
        assert_eq!(rejected_json["outcome"], "input_rejected");
        assert_eq!(rejected_json["input"], "reference");
        assert_eq!(rejected_json["failure_kind"], "parser_creation");
        assert_eq!(rejected_json["diagnostic_codes"], serde_json::json!([]));
    }
}
