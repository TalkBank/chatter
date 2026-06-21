//! Decision provenance for merge override entries.
//!
//! Every recorded merge decision carries a [`DecisionEngine`] saying
//! which engine produced it, and (for LLM decisions) a
//! [`JudgmentProvenance`] audit block. This lets an audit answer
//! "which speaker assignments were made by an LLM" straight from the
//! override file, and lets the three judgment modes (deterministic /
//! assisted / holistic) share one on-disk format.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Which engine produced a merge decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DecisionEngine {
    /// Jaccard reference-mode, spreadsheet, or operator adjudication.
    /// The default so pre-provenance override files load unchanged.
    #[default]
    Deterministic,
    /// A language-model judgment via the configured endpoint.
    Llm,
}

impl DecisionEngine {
    /// True for the [`DecisionEngine::Deterministic`] variant. Used as a
    /// serde `skip_serializing_if` predicate so deterministic pending entries
    /// omit the `engine` field, keeping pre-provenance pending files
    /// byte-identical on round-trip.
    pub fn is_deterministic(&self) -> bool {
        matches!(self, DecisionEngine::Deterministic)
    }
}

/// Model identifier reported to the endpoint (e.g. `deepseek-v4-flash`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelId(pub String);

/// OpenAI-compatible endpoint base URL the judgment was made against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointUrl(pub String);

/// Prompt-template version tag (e.g. `v1`). Bumping it marks stale
/// entries as produced by an older template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptVersion(pub String);

/// Model confidence in one decision field, in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Confidence(pub f64);

impl std::fmt::Display for Confidence {
    /// Render the bare score (e.g. `0.9`). No clamping or rounding is
    /// applied: the stored value is shown verbatim so audit output
    /// reflects exactly what the model reported.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Which decision field a [`Confidence`] score applies to.
///
/// The LLM structured output reports a confidence per judged field
/// (design spec `docs/superpowers/specs/2026-06-04-llm-in-the-loop-merge-design.md`,
/// the `confidence` object). The set is closed and model-controlled,
/// so it is an enum rather than a free string key: a stray field name
/// in an override file is a parse error, not a silently-stored key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceField {
    /// Confidence in the donor-to-CHAT speaker mapping.
    Mapping,
    /// Confidence in the assigned adult role(s) (INV/SLP/MOT/FAT).
    Roles,
    /// Confidence that the merge applies at all (vs. child-only).
    MergeApplicable,
}

/// Audit trail for an LLM-produced decision. Present only when the
/// entry's [`DecisionEngine`] is `Llm`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgmentProvenance {
    /// Model that produced the judgment.
    pub model: ModelId,
    /// Endpoint the judgment was made against.
    pub endpoint: EndpointUrl,
    /// Prompt-template version that produced it.
    pub prompt_version: PromptVersion,
    /// Per-field confidence, keyed by [`ConfidenceField`]. A holistic
    /// call reports all three fields; an assisted-mode escalation may
    /// report only the field it was asked to judge, so the map is
    /// allowed to be partial (and is omitted entirely when empty).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub confidence: BTreeMap<ConfidenceField, Confidence>,
    /// The model's merge-applicable verdict, recorded so an operator or
    /// audit can see when the model judged the session non-mergeable
    /// without parsing the free-text reasoning. Defaults to `true` for
    /// pre-provenance / deterministic entries that carry no judgment.
    #[serde(default = "default_merge_applicable")]
    pub merge_applicable: bool,
    /// One- or two-sentence model rationale.
    pub reasoning: String,
}

/// Serde default for [`JudgmentProvenance::merge_applicable`].
///
/// Returns `true` so that override files written before this field was
/// added (and deterministic entries that carry no judgment) deserialize
/// without error and are treated as "merge applies" by default.
fn default_merge_applicable() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wrapper that lets TOML treat `engine` as a table key. TOML
    /// requires a table context; bare enum values cannot be the
    /// top-level document.
    #[derive(Debug, Serialize, Deserialize)]
    struct WithEngine {
        #[serde(default)]
        engine: DecisionEngine,
    }

    #[test]
    fn decision_engine_serializes_lowercase() {
        let det = WithEngine {
            engine: DecisionEngine::Deterministic,
        };
        let llm = WithEngine {
            engine: DecisionEngine::Llm,
        };
        let det_toml = toml::to_string(&det).expect("serialize Deterministic");
        let llm_toml = toml::to_string(&llm).expect("serialize Llm");
        assert!(
            det_toml.contains("deterministic"),
            "expected lowercase 'deterministic'; got: {det_toml}"
        );
        assert!(
            llm_toml.contains("llm"),
            "expected lowercase 'llm'; got: {llm_toml}"
        );
    }

    #[test]
    fn decision_engine_round_trips() {
        let det: WithEngine =
            toml::from_str("engine = \"deterministic\"").expect("parse deterministic");
        let llm: WithEngine = toml::from_str("engine = \"llm\"").expect("parse llm");
        assert_eq!(det.engine, DecisionEngine::Deterministic);
        assert_eq!(llm.engine, DecisionEngine::Llm);
    }

    #[test]
    fn decision_engine_defaults_to_deterministic_when_field_absent() {
        let v: WithEngine = toml::from_str("").expect("parse empty TOML");
        assert_eq!(
            v.engine,
            DecisionEngine::Deterministic,
            "missing engine field must default to Deterministic"
        );
    }

    /// `ConfidenceField` must render snake_case so the on-disk keys match
    /// the spec's `confidence` object (`merge_applicable`, not
    /// `MergeApplicable`).
    #[test]
    fn confidence_field_serializes_snake_case() {
        let v = serde_json::to_string(&ConfidenceField::MergeApplicable)
            .expect("serialize MergeApplicable");
        assert_eq!(v, "\"merge_applicable\"");
    }

    /// A `BTreeMap<ConfidenceField, Confidence>` must round-trip through
    /// TOML with the enum acting as a table key. This is the contract the
    /// override file relies on; if TOML stops supporting unit-variant enum
    /// keys this test fails loudly rather than at a distant call site.
    #[test]
    fn confidence_map_round_trips_with_enum_keys_via_toml() {
        /// TOML needs a table context; the bare map cannot be the
        /// top-level document.
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct WithConfidence {
            confidence: BTreeMap<ConfidenceField, Confidence>,
        }
        let original = WithConfidence {
            confidence: BTreeMap::from([
                (ConfidenceField::Mapping, Confidence(0.9)),
                (ConfidenceField::MergeApplicable, Confidence(0.5)),
            ]),
        };
        let serialized = toml::to_string(&original).expect("serialize confidence map");
        assert!(
            serialized.contains("merge_applicable"),
            "expected snake_case key in TOML; got: {serialized}"
        );
        let back: WithConfidence = toml::from_str(&serialized).expect("parse confidence map");
        assert_eq!(original, back);
    }

    /// `Confidence` renders its bare score for audit output.
    #[test]
    fn confidence_displays_bare_score() {
        assert_eq!(Confidence(0.9).to_string(), "0.9");
    }
}
