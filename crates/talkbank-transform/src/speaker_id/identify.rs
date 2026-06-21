//! Reference-mode identification: pick the donor speaker matching a
//! reference anchor by multiset-Jaccard text similarity.

use std::collections::{BTreeMap, HashMap, HashSet};

use talkbank_model::SpeakerCode;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{ChatFile, Line};

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
pub const DEFAULT_CONFIDENCE_THRESHOLD: ConfidenceThreshold = ConfidenceThreshold(2.0);

/// Outcome of [`identify_mapping`] on a clean (auto-decidable) donor.
///
/// Carries the per-speaker scores so the operator can audit the
/// decision, and the winner→runner-up margin so a confidence-threshold
/// check can refuse low-confidence auto-decisions.
#[derive(Debug, Clone)]
pub struct DonorMatchReport {
    /// Donor speaker whose token bag best matched the reference
    /// anchor. In the eventual mapping this speaker is marked for
    /// `Drop`, the reference covers them authoritatively, so the
    /// downstream merge will pull their utterances from the reference
    /// rather than the donor.
    pub winner: SpeakerCode,

    /// Multiset-Jaccard score for every donor speaker against the
    /// reference anchor bag.
    pub scores: HashMap<SpeakerCode, JaccardScore>,

    /// Winner→runner-up ratio. On the success path always
    /// satisfies `margin.meets(threshold)`.
    pub margin: ConfidenceMargin,
}

impl DonorMatchReport {
    /// Render the typed Jaccard scores into the on-disk override-file
    /// shape (deterministic `BTreeMap<String, f64>`, sorted by
    /// speaker code).
    pub fn scores_to_serializable(&self) -> BTreeMap<String, f64> {
        self.scores
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.0))
            .collect()
    }

    /// Render the typed margin into the on-disk override-file shape.
    /// `None` when the runner-up scored zero AND the winner also
    /// scored zero (no information); `Some(INFINITY)` when the
    /// winner alone took everything.
    pub fn margin_to_serializable(&self) -> Option<f64> {
        if self.margin.0 == 0.0 {
            None
        } else {
            Some(self.margin.0)
        }
    }
}

/// Multiset Jaccard similarity over two token-count maps:
/// |A ∩ B| / |A ∪ B| where ∩ and ∪ use per-token min and max counts.
///
/// Returns `0.0` when either bag is empty or both vocabularies are
/// disjoint (union > 0 by definition once either bag is non-empty,
/// so the union == 0 fallback only triggers on the empty cases the
/// early return already covers).
fn jaccard(a: &HashMap<String, u32>, b: &HashMap<String, u32>) -> JaccardScore {
    if a.is_empty() || b.is_empty() {
        return JaccardScore(0.0);
    }
    let mut vocab: HashSet<&str> = HashSet::with_capacity(a.len() + b.len());
    for k in a.keys().chain(b.keys()) {
        vocab.insert(k.as_str());
    }
    let mut intersection: u64 = 0;
    let mut union: u64 = 0;
    for token in vocab {
        let ac = u64::from(*a.get(token).unwrap_or(&0));
        let bc = u64::from(*b.get(token).unwrap_or(&0));
        intersection += ac.min(bc);
        union += ac.max(bc);
    }
    if union == 0 {
        JaccardScore(0.0)
    } else {
        JaccardScore(intersection as f64 / union as f64)
    }
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
    for line in chat.lines.0.iter() {
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
/// per-speaker scores, and the margin (always ≥ `threshold` on the
/// success path). On `margin < threshold` returns
/// [`SpeakerIdError::LowConfidence`] with the same scores and the
/// computed margin so the operator can adjudicate.
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

    let mut scores: HashMap<SpeakerCode, JaccardScore> = HashMap::new();
    for spk in donor_speakers.iter() {
        scores.insert(spk.clone(), jaccard(&ref_bag, &speaker_bag(donor, spk)));
    }

    // Sort donors by descending Jaccard score. Ties break on the
    // donor's document-order position (already captured in
    // `donor_speakers`); the resulting order is deterministic across
    // runs given the same input.
    let mut ranked: Vec<&SpeakerCode> = donor_speakers.iter().collect();
    ranked.sort_by(|a, b| {
        scores[*b]
            .partial_cmp(&scores[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let winner = (*ranked[0]).clone();
    let winner_score = scores[&winner];
    let runner_up_score = scores[ranked[1]];
    let margin = ConfidenceMargin::from_scores(winner_score, runner_up_score);

    let report = DonorMatchReport {
        winner,
        scores,
        margin,
    };

    if !margin.meets(threshold) {
        return Err(SpeakerIdError::LowConfidence { report, threshold });
    }

    Ok(report)
}
