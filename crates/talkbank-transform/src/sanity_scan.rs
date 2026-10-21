//! Sanity scan for structurally assembled CHAT.
//!
//! The scan detects sessions where pass-1 reference-mode produced a
//! confident (above-threshold) auto-decision that nonetheless looks
//! suspicious by an out-of-band signal. Flagged sessions become
//! [`crate::adjudication::AdjudicationKind::SanityScanMisclassification`]
//! pending entries the operator resolves via `chatter adjudicate`.
//!
//! The heuristic carried in this module is **mean utterance word
//! count asymmetry**: if the anchor's mean utterance word count
//! exceeds the inserted speaker's by a configurable ratio, that's
//! suspicious for typical child-language transcripts (children
//! usually have shorter utterances than adults).
//!
//! **Limitations.** The current heuristic only handles the binary
//! case (one Drop, one Rename in the override entry's mapping).
//! Multi-rename sessions return `None`. A more sophisticated
//! signal (morphological complexity, lexical diversity, content
//! word ratio) could replace word-count-mean without changing the
//! scan's interface.

use std::collections::BTreeMap;

use talkbank_model::SpeakerCode;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::ChatFile;

use crate::speaker_id::{InsertedRoleSpec, MergeOverride, SpeakerAction};

/// Ratio threshold for the mean-utterance-word-count heuristic.
///
/// A scan flags a session when
/// `anchor_mean_words >= inserted_mean_words * threshold.value()`.
/// The factor must be finite and positive; choosing a useful sensitivity
/// remains the caller's responsibility. The CLI default is 1.5×.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SanityScanThreshold(f64);

impl SanityScanThreshold {
    /// Operator-facing default, not a calibrated accuracy guarantee.
    pub const DEFAULT: Self = Self(1.5);

    /// Admit a finite positive comparison factor.
    pub fn new(value: f64) -> Result<Self, SanityScanThresholdError> {
        match value {
            value if value.is_finite() && value > 0.0 => Ok(Self(value)),
            value => Err(SanityScanThresholdError::Invalid(value)),
        }
    }

    /// Scalar form for recording the admitted threshold in pending evidence.
    pub fn value(self) -> f64 {
        self.0
    }
}

/// Numeric syntax and domain refusals for a sanity-scan threshold.
#[derive(Debug, thiserror::Error)]
pub enum SanityScanThresholdError {
    /// The supplied text is not a floating-point number.
    #[error("sanity-scan threshold is not a number: {0}")]
    Float(#[from] std::num::ParseFloatError),
    /// The number cannot be used as a finite positive comparison factor.
    #[error("sanity-scan threshold must be finite and positive, got {0}")]
    Invalid(f64),
}

impl std::str::FromStr for SanityScanThreshold {
    type Err = SanityScanThresholdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::new(text.parse()?)
    }
}

impl std::fmt::Display for SanityScanThreshold {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Outcome of scanning one session for misclassification.
#[derive(Debug, Clone)]
pub struct SanityScanFlag {
    /// Mean utterance word count for the anchor speaker in the
    /// merged file. Used by the diagnostic `reason` and surfaced in
    /// operator-facing prompts.
    pub anchor_mean_words: f64,
    /// Mean utterance word count for the inserted speaker in the
    /// merged file.
    pub inserted_mean_words: f64,
    /// The swapped suggested mapping in donor coordinates. Equal to
    /// the override entry's mapping with Drop and Rename swapped.
    /// Ready to drop into a
    /// [`crate::adjudication::PendingKindData::SanityScanMisclassification`]
    /// entry's `suggested.mapping`.
    pub suggested_mapping: BTreeMap<String, SpeakerAction>,
    /// Per-donor-speaker-code role assignment for the swapped
    /// suggestion, keyed by the speaker whose swapped action is
    /// `Rename` (the original drop speaker, which the scan now believes
    /// is the adult to merge in). Pass-1 already knew what role label the
    /// inserted donor speaker was given; the scan carries it onto the
    /// swapped speaker. Ready to drop into a
    /// [`crate::adjudication::SuggestedSpeakerIdMapping`]'s `adult_roles`.
    pub suggested_adult_roles: BTreeMap<String, InsertedRoleSpec>,
    /// Free-form diagnostic explaining what triggered the flag.
    /// Surfaced to the operator via the pending entry's `reason`
    /// field.
    pub reason: String,
}

/// Scan one session for misclassification. Returns `Some` when the
/// asymmetry heuristic fires AND the override entry has the
/// binary-mapping shape (exactly one Drop, exactly one Rename) the
/// current heuristic supports.
///
/// Pre-conditions:
/// - `merged_chat` is the pass-1 merged output for this session.
/// - `override_entry` is the pass-1 override entry that produced it.
/// - `anchor` is the reference's anchor speaker code (typically
///   CHI).
///
/// Returns `None` when:
/// - The asymmetry is below threshold (the scan trusts the
///   auto-decision).
/// - The override entry doesn't have the binary-mapping shape.
/// - Either speaker has zero utterances in the merged file (can't
///   compute a meaningful mean).
pub fn scan_session(
    merged_chat: &ChatFile,
    override_entry: &MergeOverride,
    anchor: &SpeakerCode,
    threshold: SanityScanThreshold,
) -> Option<SanityScanFlag> {
    // Determine the binary Drop/Rename pair first: the single Rename
    // speaker's inserted-role spec is that donor code's entry in the
    // override's per-speaker `adult_roles` map (was a single shared
    // `inserted_role` field before the multi-adult representation). A
    // malformed entry missing that role makes the scan skip the session
    // (returns None) rather than crash.
    let (drop_speaker, rename_speaker) = binary_mapping_pair(override_entry)?;
    let inserted_role_spec = override_entry.adult_roles.get(&rename_speaker)?;
    let inserted_code = SpeakerCode::new(inserted_role_spec.code.as_str());
    let anchor_mean = mean_utterance_word_count(merged_chat, anchor)?;
    let inserted_mean = mean_utterance_word_count(merged_chat, &inserted_code)?;
    if inserted_mean <= 0.0 {
        return None;
    }
    if anchor_mean < inserted_mean * threshold.0 {
        return None;
    }
    let swapped = swapped_mapping(&drop_speaker, &rename_speaker);
    let reason = format!(
        "anchor {anchor} mean utterance word count {anchor_mean:.2} exceeds inserted {inserted} \
         mean {inserted_mean:.2} by {ratio:.2}× (threshold {threshold:.2}×), child typically \
         shorter than adult; likely swap of donor speakers {drop_speaker:?} (drop) and \
         {rename_speaker:?} (rename)",
        anchor = anchor.as_str(),
        inserted = inserted_code.as_str(),
        anchor_mean = anchor_mean,
        inserted_mean = inserted_mean,
        ratio = anchor_mean / inserted_mean,
        threshold = threshold.0,
    );
    // After the swap the original drop speaker takes the Rename action,
    // so it inherits the inserted role; key the suggested map by it.
    let mut suggested_adult_roles: BTreeMap<String, InsertedRoleSpec> = BTreeMap::new();
    suggested_adult_roles.insert(drop_speaker.clone(), inserted_role_spec.clone());
    Some(SanityScanFlag {
        anchor_mean_words: anchor_mean,
        inserted_mean_words: inserted_mean,
        suggested_mapping: swapped,
        suggested_adult_roles,
        reason,
    })
}

/// Mean utterance word count for `speaker` in `chat`, computed by
/// `walk_words` over each utterance's AST. Returns `None` when
/// `speaker` has zero utterances.
///
/// "Word" here matches the cleaner used by speaker-id Jaccard: a
/// content word leaf in the AST. Pure separators and replaced-word
/// annotations don't count.
fn mean_utterance_word_count(chat: &ChatFile, speaker: &SpeakerCode) -> Option<f64> {
    let mut utterances: u32 = 0;
    let mut total_words: u32 = 0;
    for u in chat.utterances().filter(|u| &u.main.speaker == speaker) {
        utterances += 1;
        walk_words(&u.main.content.content, None, &mut |item| {
            if let WordItem::Word(_) = item {
                total_words += 1;
            }
        });
    }
    if utterances == 0 {
        return None;
    }
    Some(f64::from(total_words) / f64::from(utterances))
}

/// Find the one-Drop / one-Rename binary pair in `override_entry`'s
/// mapping. Returns `None` when the mapping has any other shape
/// (zero or multiple Drop entries; zero or multiple Rename entries;
/// extra unsupported actions).
fn binary_mapping_pair(override_entry: &MergeOverride) -> Option<(String, String)> {
    let mut drop_speaker: Option<String> = None;
    let mut rename_speaker: Option<String> = None;
    for (spk, action) in override_entry.mapping.iter() {
        match action {
            SpeakerAction::Drop => {
                if drop_speaker.is_some() {
                    return None;
                }
                drop_speaker = Some(spk.clone());
            }
            SpeakerAction::Rename => {
                if rename_speaker.is_some() {
                    return None;
                }
                rename_speaker = Some(spk.clone());
            }
        }
    }
    match (drop_speaker, rename_speaker) {
        (Some(d), Some(r)) => Some((d, r)),
        _ => None,
    }
}

/// Build the swapped mapping: the original Drop becomes Rename, and
/// the original Rename becomes Drop. Suggestions in donor
/// coordinates, ready to embed in a pending entry's
/// `suggested.mapping`.
fn swapped_mapping(drop_speaker: &str, rename_speaker: &str) -> BTreeMap<String, SpeakerAction> {
    let mut out: BTreeMap<String, SpeakerAction> = BTreeMap::new();
    out.insert(drop_speaker.to_string(), SpeakerAction::Rename);
    out.insert(rename_speaker.to_string(), SpeakerAction::Drop);
    out
}

#[cfg(test)]
mod tests {
    use super::SanityScanThreshold;

    #[test]
    fn threshold_admission_rejects_nonfinite_and_nonpositive_values() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -0.0, -1.0] {
            assert!(SanityScanThreshold::new(value).is_err());
            assert!(value.to_string().parse::<SanityScanThreshold>().is_err());
        }
        assert!("not-a-number".parse::<SanityScanThreshold>().is_err());
    }

    #[test]
    fn threshold_admission_preserves_positive_factors_and_default() {
        for value in [f64::MIN_POSITIVE, 0.5, 1.0, 1.5, 2.0, f64::MAX] {
            let admitted = SanityScanThreshold::new(value).expect("positive finite factor");
            assert_eq!(admitted.value(), value);
            let reparsed = admitted
                .to_string()
                .parse::<SanityScanThreshold>()
                .expect("displayed factor");
            assert_eq!(reparsed, admitted);
        }
        assert_eq!(SanityScanThreshold::DEFAULT.value(), 1.5);
    }
}
