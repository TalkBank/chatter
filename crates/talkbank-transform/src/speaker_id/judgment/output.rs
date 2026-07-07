//! Typed parse target for the holistic LLM judgment response.
//!
//! The model returns a single JSON object (design spec
//! `docs/superpowers/specs/2026-06-04-llm-in-the-loop-merge-design.md`, the
//! "holistic judgment call" section). The LLM returns DECISIONS, never CHAT
//! bytes; this module is the boundary where that JSON becomes a validated
//! Rust value. Unknown enum values and missing required fields are parse
//! errors (fail-closed), not silently-tolerated states.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::speaker_id::judgment::session_context::SampleTypeLabel;
use crate::speaker_id::{Confidence, ConfidenceField};

/// An anonymized ASR donor speaker code as it appears in the donor file and
/// in the LLM's `speaker_mapping` keys (e.g. `PAR0`, `PAR1`). A dedicated
/// newtype (rather than reusing `SpeakerCode`) keeps the JSON map-key
/// contract local and `Ord` for `BTreeMap`; the consumption step converts
/// it to a `SpeakerCode` at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(transparent)]
pub struct DonorCode(pub String);

/// The LLM's verdict for one donor speaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum SpeakerVerdict {
    /// This donor speaker IS the child; its donor utterances are dropped
    /// (the child is already hand-coded in the anchor file).
    #[serde(rename = "CHI")]
    Child,
    /// This donor speaker is the adult to merge in (renamed per `adult_roles`).
    #[serde(rename = "adult")]
    Adult,
    /// This donor speaker is noise / a third party; drop it.
    #[serde(rename = "drop")]
    Drop,
}

/// The CHAT speaker code the LLM assigns to a merged adult speaker. Closed
/// set per the spec (`INV` / `SLP` / `MOT` / `FAT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AdultRole {
    /// Investigator.
    Inv,
    /// Speech-language pathologist / clinician.
    Slp,
    /// Mother.
    Mot,
    /// Father.
    Fat,
}

impl AdultRole {
    /// Return the uppercase CHAT speaker code for this role, matching the
    /// on-wire serde serialization and the CHAT `@Participants` header format
    /// (e.g. `"INV"`, `"SLP"`, `"MOT"`, `"FAT"`). Used by the prompt renderer
    /// to list declared roles in the user message.
    pub fn as_code(&self) -> &'static str {
        match self {
            Self::Inv => "INV",
            Self::Slp => "SLP",
            Self::Mot => "MOT",
            Self::Fat => "FAT",
        }
    }

    /// The CHAT speaker code and `@ID` role tag for this adult role.
    ///
    /// Tags are drawn from chatter's valid participant-role set so that
    /// merged output passes `chatter validate`:
    ///   - INV -> Investigator
    ///   - SLP -> Therapist  (the only clinical-adult role in the valid set)
    ///   - MOT -> Mother
    ///   - FAT -> Father
    pub fn inserted_role_spec(self) -> crate::speaker_id::InsertedRoleSpec {
        let tag = match self {
            AdultRole::Inv => "Investigator",
            AdultRole::Slp => "Therapist",
            AdultRole::Mot => "Mother",
            AdultRole::Fat => "Father",
        };
        crate::speaker_id::InsertedRoleSpec {
            code: self.as_code().to_string(),
            tag: tag.to_string(),
            specific_role: None,
        }
    }
}

/// The LLM's verdict on the declared sample type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampleTypeVerdict {
    /// The declared sample type is correct.
    Confirmed,
    /// The declared sample type is wrong; the LLM's corrected label.
    /// Free vocabulary, matching the free-vocabulary declared sample
    /// type from the session-context file.
    Corrected(SampleTypeLabel),
    /// The LLM cannot determine the sample type.
    Uncertain,
}

impl<'de> Deserialize<'de> for SampleTypeVerdict {
    /// Parse the spec's tagged-string form: `"confirmed"`, `"uncertain"`,
    /// or `"corrected:<type>"` where `<type>` is a non-empty free-text
    /// sample-type label.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const CORRECTED_PREFIX: &str = "corrected:";
        const CONFIRMED: &str = "confirmed";
        const UNCERTAIN: &str = "uncertain";
        let raw = String::deserialize(deserializer)?;
        if raw == CONFIRMED {
            return Ok(Self::Confirmed);
        }
        if raw == UNCERTAIN {
            return Ok(Self::Uncertain);
        }
        if let Some(rest) = raw.strip_prefix(CORRECTED_PREFIX) {
            let label = rest.trim();
            if label.is_empty() {
                return Err(serde::de::Error::custom(
                    "corrected: requires a non-empty sample-type label",
                ));
            }
            return Ok(Self::Corrected(SampleTypeLabel(label.to_string())));
        }
        Err(serde::de::Error::custom(format!(
            "expected confirmed | uncertain | corrected:<type>, got {raw:?}"
        )))
    }
}

/// The full structured judgment returned by one holistic LLM call.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HolisticJudgment {
    /// Per-donor-speaker verdict (child / adult / drop).
    pub speaker_mapping: BTreeMap<DonorCode, SpeakerVerdict>,
    /// CHAT code for each donor speaker the LLM ruled an adult.
    pub adult_roles: BTreeMap<DonorCode, AdultRole>,
    /// Verdict on the declared sample type (from the session-context
    /// input, when one was supplied).
    pub sample_type: SampleTypeVerdict,
    /// Whether an adult merge applies at all (false for monologic samples).
    pub merge_applicable: bool,
    /// Per-field confidence in `0.0..=1.0`.
    pub confidence: BTreeMap<ConfidenceField, Confidence>,
    /// One- or two-sentence model rationale.
    pub reasoning: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
      "speaker_mapping": { "PAR0": "CHI", "PAR1": "adult" },
      "adult_roles": { "PAR1": "INV" },
      "sample_type": "confirmed",
      "merge_applicable": true,
      "confidence": { "mapping": 0.9, "roles": 0.8, "merge_applicable": 0.95 },
      "reasoning": "PAR1 prompts, PAR0 answers."
    }"#;

    #[test]
    fn parses_a_valid_holistic_response() {
        let j: HolisticJudgment = serde_json::from_str(VALID).expect("parse valid");
        assert_eq!(
            j.speaker_mapping.get(&DonorCode("PAR0".into())),
            Some(&SpeakerVerdict::Child)
        );
        assert_eq!(
            j.adult_roles.get(&DonorCode("PAR1".into())),
            Some(&AdultRole::Inv)
        );
        assert_eq!(j.sample_type, SampleTypeVerdict::Confirmed);
        assert!(j.merge_applicable);
        assert_eq!(
            j.confidence.get(&ConfidenceField::Mapping),
            Some(&crate::speaker_id::Confidence(0.9))
        );
    }

    #[test]
    fn parses_corrected_sample_type_as_free_label() {
        let s = VALID.replace("\"confirmed\"", "\"corrected:reading aloud\"");
        let j: HolisticJudgment = serde_json::from_str(&s).expect("parse corrected");
        assert_eq!(
            j.sample_type,
            SampleTypeVerdict::Corrected(SampleTypeLabel("reading aloud".to_string()))
        );
    }

    #[test]
    fn rejects_empty_corrected_sample_type() {
        let s = VALID.replace("\"confirmed\"", "\"corrected:\"");
        assert!(
            serde_json::from_str::<HolisticJudgment>(&s).is_err(),
            "corrected: with an empty label must be a parse error"
        );
    }

    #[test]
    fn rejects_missing_required_field() {
        let s = VALID.replace("\"merge_applicable\": true,", "");
        assert!(
            serde_json::from_str::<HolisticJudgment>(&s).is_err(),
            "missing merge_applicable must be a parse error"
        );
    }

    #[test]
    fn rejects_unknown_speaker_verdict() {
        let s = VALID.replace("\"adult\"", "\"alien\"");
        assert!(serde_json::from_str::<HolisticJudgment>(&s).is_err());
    }
}
