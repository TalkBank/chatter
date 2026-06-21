//! Typed input to the holistic judgment call. Built deterministically from
//! the donor `ChatFile` plus optional session-context records; rendered into
//! the prompt by `prompt.rs`. No model or network here.

use serde::Deserialize;
use talkbank_model::SpeakerCode;

use crate::speaker_id::judgment::session_context::{
    AgeMonths, ConsentTierLabel, RoleLabel, SampleTypeLabel,
};

/// Stable session identifier (the donor file's base name, typically).
///
/// Also the key type of the session-context file's map, so it derives
/// `Ord` (for `BTreeMap`) and deserializes transparently from a JSON
/// object key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

// `Borrow<str>` is sound here because `SessionId`'s derived `Ord` is the
// inner `String`'s ordering, which agrees with `str`'s ordering, the
// invariant `Borrow` requires for map lookups (the session-context file
// keys its `BTreeMap` by `SessionId` and looks up by raw stem).
impl std::borrow::Borrow<str> for SessionId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// One sampled, char-capped, main-tier-only utterance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampledUtterance(pub String);

/// The deterministic sample of one speaker's utterances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerSamples {
    /// The speaker code as it appears in the donor file (anchor or donor).
    pub code: SpeakerCode,
    /// Head + tail sampled utterances (see `sample.rs`).
    pub utterances: Vec<SampledUtterance>,
}

/// Everything the holistic call needs as input. Absent session-context
/// fields are `None` / empty (the spec permits "unknown").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgmentRequest {
    /// Session identifier (for the prompt and provenance).
    pub session_id: SessionId,
    /// Declared sample-type label, or `None` if unknown. Free
    /// vocabulary; rendered verbatim into the prompt.
    pub sample_type: Option<SampleTypeLabel>,
    /// Declared adult role label(s); may be empty. Free vocabulary;
    /// rendered verbatim into the prompt.
    pub declared_roles: Vec<RoleLabel>,
    /// Declared media-consent tier label, or `None` if unknown. Free
    /// vocabulary; rendered verbatim into the prompt.
    pub consent_tier: Option<ConsentTierLabel>,
    /// Child age in months for developmental-stage awareness; `None` if
    /// unknown (a 14-month-old babbles, a 13-year-old does not; the prompt
    /// must condition on this, per the 2026-06-05 intake refinement).
    pub age_months: Option<AgeMonths>,
    /// The anchor (child) speaker code in the donor file (e.g. `CHI`).
    pub anchor: SpeakerCode,
    /// Per-speaker sampled utterances (anchor first, then donor speakers).
    pub samples: Vec<SpeakerSamples>,
}
