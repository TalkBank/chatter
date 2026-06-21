//! Post-merge sanity scan for `chatter merge` output.
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
/// `anchor_mean_words >= inserted_mean_words * threshold.0`. Values
/// below 1.0 would flag every session; the realistic operator range
/// is 1.2× (sensitive) to 2.0× (specific). The CLI default is 1.5×.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SanityScanThreshold(pub f64);

impl SanityScanThreshold {
    /// Operator-facing default. Picked at 1.5× as a balance between
    /// recall (catching real misclassifications) and precision
    /// (not flagging every typical child-adult session with mild
    /// asymmetry). Operators tune per-corpus via the CLI.
    pub const DEFAULT: Self = Self(1.5);
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
    /// The inserted-role spec passed through from the override
    /// entry. Pass-1 already knew what role label the inserted
    /// donor speakers were given; the scan preserves it on the
    /// suggested mapping.
    pub suggested_inserted_role: InsertedRoleSpec,
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
    let inserted_code = SpeakerCode::new(override_entry.inserted_role.code.as_str());
    let anchor_mean = mean_utterance_word_count(merged_chat, anchor)?;
    let inserted_mean = mean_utterance_word_count(merged_chat, &inserted_code)?;
    if inserted_mean <= 0.0 {
        return None;
    }
    if anchor_mean < inserted_mean * threshold.0 {
        return None;
    }
    let (drop_speaker, rename_speaker) = binary_mapping_pair(override_entry)?;
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
    Some(SanityScanFlag {
        anchor_mean_words: anchor_mean,
        inserted_mean_words: inserted_mean,
        suggested_mapping: swapped,
        suggested_inserted_role: override_entry.inserted_role.clone(),
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
