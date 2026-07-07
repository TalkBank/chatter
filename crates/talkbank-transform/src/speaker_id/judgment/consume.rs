//! Turn a holistic LLM judgment into a pending adjudication entry. The LLM
//! is an advisor; per the pre-calibration trust posture every LLM-influenced
//! decision lands in `pending.toml` for human review, stamped engine=llm.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::adjudication::{PendingEntry, PendingKindData, SuggestedSpeakerIdMapping};
use crate::speaker_id::{
    DecisionEngine, EndpointUrl, InsertedRoleSpec, JudgmentProvenance, ModelId, PromptVersion,
    SpeakerAction,
};

use super::output::{AdultRole, HolisticJudgment, SpeakerVerdict};

/// Endpoint, model, and prompt identity stamped onto the provenance block of
/// the resulting pending entry.
#[derive(Debug, Clone)]
pub struct ProvenanceMeta {
    /// Model identifier (e.g. `deepseek-v4-flash`).
    pub model: ModelId,
    /// Endpoint base URL the judgment was made against.
    pub endpoint: EndpointUrl,
    /// Prompt-template version that produced the judgment.
    pub prompt_version: PromptVersion,
}

/// Why a judgment could not be converted into a pending entry.
#[derive(Debug, thiserror::Error)]
pub enum ConsumeError {
    /// A speaker ruled `adult` had no corresponding entry in `adult_roles`.
    #[error("adult speaker {0} missing from adult_roles")]
    AdultRoleMissing(String),

    /// The judgment says a merge applies (`merge_applicable == true`) but no
    /// donor speaker was ruled an adult. A self-contradictory judgment; fail
    /// closed rather than emit a misleading placeholder suggestion.
    #[error("judgment has merge_applicable=true but no adult speaker")]
    NoAdultButMergeApplicable,
}

/// Build a `SpeakerIdLowConfidence` pending entry from a holistic judgment,
/// stamped `engine = "llm"` with the full provenance block.
///
/// Every LLM-influenced decision is routed to `pending.toml` (the
/// human-review path) rather than auto-applied; this function is the
/// pre-calibration trust posture's enforcement point.
///
/// The `session_id` identifies the CHAT session; `judgment` is the parsed
/// holistic response; `meta` carries the model/endpoint/prompt identity;
/// `created_at` is the creation timestamp recorded in the pending entry.
pub fn judgment_to_pending(
    session_id: &str,
    judgment: &HolisticJudgment,
    meta: &ProvenanceMeta,
    created_at: DateTime<Utc>,
) -> Result<PendingEntry, ConsumeError> {
    let mut mapping: BTreeMap<String, SpeakerAction> = BTreeMap::new();
    // Collect every adult (donor code + assigned role), in the
    // BTreeMap's alphabetical donor-code traversal order; multiple
    // adults are supported (the LLM may assign distinct roles per
    // adult speaker).
    let mut adults: Vec<(String, AdultRole)> = Vec::new();

    for (code, verdict) in &judgment.speaker_mapping {
        let action = match verdict {
            SpeakerVerdict::Adult => {
                // Look up the role the model assigned to this adult speaker.
                let role = judgment
                    .adult_roles
                    .get(code)
                    .ok_or_else(|| ConsumeError::AdultRoleMissing(code.0.clone()))?;
                adults.push((code.0.clone(), *role));
                SpeakerAction::Rename
            }
            // Both CHI and drop verdicts produce a Drop action: CHI utterances
            // are already in the anchor file and need no merge contribution;
            // drop means noise or a third party.
            SpeakerVerdict::Child | SpeakerVerdict::Drop => SpeakerAction::Drop,
        };
        mapping.insert(code.0.clone(), action);
    }

    // Fail closed on a self-contradictory judgment: a merge is said to apply
    // but the model named no adult to merge in.
    if judgment.merge_applicable && adults.is_empty() {
        return Err(ConsumeError::NoAdultButMergeApplicable);
    }

    // Empty when there is no adult (the legitimate merge_applicable == false
    // case): the suggested mapping is all-Drop, so there is no adult to assign
    // a role to and no placeholder is emitted.
    let adult_roles = disambiguate_adult_roles(adults);

    let suggested = SuggestedSpeakerIdMapping {
        mapping,
        adult_roles,
    };

    let provenance = JudgmentProvenance {
        model: meta.model.clone(),
        endpoint: meta.endpoint.clone(),
        prompt_version: meta.prompt_version.clone(),
        confidence: judgment.confidence.clone(),
        merge_applicable: judgment.merge_applicable,
        reasoning: judgment.reasoning.clone(),
    };

    Ok(PendingEntry {
        session_id: session_id.to_string(),
        created_at,
        data: PendingKindData::SpeakerIdLowConfidence { suggested },
        scores: BTreeMap::new(),
        margin: None,
        threshold_used: None,
        engine: DecisionEngine::Llm,
        judgment: Some(provenance),
    })
}

/// Build the on-disk `adult_roles` map from `(donor_code, AdultRole)`
/// pairs, auto-disambiguating any `AdultRole` shared by 2+ donor codes
/// per the CHAT manual's `CHI1`/`CHI2` convention (numbered speaker
/// codes, a `First_`/`Second_`/... specific-role label, the shared
/// standard role tag unchanged). Codes within a colliding group keep
/// their existing (already-alphabetical, from the BTreeMap traversal in
/// `judgment_to_pending`) input order, so the first donor code gets
/// `First_`.
///
/// This function is total and never panics: a same-role group larger
/// than the named ordinals falls back to a plain 1-based numeral label
/// (`5_Investigator`) rather than failing, and the numbered code is
/// always `<CODE><n>` regardless of group size.
fn disambiguate_adult_roles(
    adults: Vec<(String, AdultRole)>,
) -> BTreeMap<String, InsertedRoleSpec> {
    // Human-readable ordinal labels for the common small collisions;
    // positions beyond this fall back to a bare numeral (see below).
    const ORDINALS: &[&str] = &["First", "Second", "Third", "Fourth"];

    // Group donor codes by their assigned role code, preserving the
    // input (alphabetical) order within each group.
    let mut by_role: BTreeMap<&'static str, Vec<(String, AdultRole)>> = BTreeMap::new();
    for (code, role) in adults {
        by_role
            .entry(role.as_code())
            .or_default()
            .push((code, role));
    }

    let mut result: BTreeMap<String, InsertedRoleSpec> = BTreeMap::new();
    for group in by_role.into_values() {
        // A group of one is the ordinary single-adult-per-role case: no
        // numbering, no disambiguation label. A group of 2+ collides and
        // needs the CHI1/CHI2 treatment.
        let shared = group.len() >= 2;
        for (position, (donor_code, role)) in group.into_iter().enumerate() {
            let tag = role.inserted_role_spec().tag;
            let spec = if shared {
                // `position` is 0-based; CHAT codes and ordinals are 1-based.
                let ordinal_label = match ORDINALS.get(position) {
                    Some(word) => format!("{word}_{tag}"),
                    None => format!("{}_{tag}", position + 1),
                };
                InsertedRoleSpec {
                    code: format!("{}{}", role.as_code(), position + 1),
                    tag,
                    specific_role: Some(ordinal_label),
                }
            } else {
                InsertedRoleSpec {
                    code: role.as_code().to_string(),
                    tag,
                    specific_role: None,
                }
            };
            result.insert(donor_code, spec);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::{TimeZone, Utc};

    use crate::adjudication::{PendingAdjudications, PendingKindData};
    use crate::speaker_id::judgment::output::{
        AdultRole, DonorCode, HolisticJudgment, SampleTypeVerdict, SpeakerVerdict,
    };
    use crate::speaker_id::{
        Confidence, ConfidenceField, DecisionEngine, EndpointUrl, ModelId, PromptVersion,
        SpeakerAction,
    };

    use super::{ConsumeError, ProvenanceMeta, judgment_to_pending};

    // -----------------------------------------------------------------------
    // Fixtures
    // -----------------------------------------------------------------------

    fn test_meta() -> ProvenanceMeta {
        ProvenanceMeta {
            model: ModelId("test-model-v1".to_string()),
            endpoint: EndpointUrl("https://api.example.com/v1".to_string()),
            prompt_version: PromptVersion("v1".to_string()),
        }
    }

    fn test_judgment_with_one_adult() -> HolisticJudgment {
        HolisticJudgment {
            speaker_mapping: BTreeMap::from([
                (DonorCode("PAR0".to_string()), SpeakerVerdict::Child),
                (DonorCode("PAR1".to_string()), SpeakerVerdict::Adult),
                (DonorCode("PAR2".to_string()), SpeakerVerdict::Drop),
            ]),
            adult_roles: BTreeMap::from([(DonorCode("PAR1".to_string()), AdultRole::Inv)]),
            sample_type: SampleTypeVerdict::Confirmed,
            merge_applicable: true,
            confidence: BTreeMap::from([
                (ConfidenceField::Mapping, Confidence(0.9)),
                (ConfidenceField::Roles, Confidence(0.85)),
                (ConfidenceField::MergeApplicable, Confidence(0.95)),
            ]),
            reasoning: "PAR0 produces child-like short turns; PAR1 prompts.".to_string(),
        }
    }

    fn fixed_ts() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 6, 10, 0, 0).unwrap()
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    /// PAR0=Child -> Drop, PAR1=Adult(INV) -> Rename, PAR2=Drop -> Drop.
    /// The inserted_role should be INV / Investigator.
    #[test]
    fn adult_verdict_becomes_rename_child_and_drop_become_drop() {
        let judgment = test_judgment_with_one_adult();
        let entry = judgment_to_pending("sess-001", &judgment, &test_meta(), fixed_ts())
            .expect("judgment_to_pending should succeed for a valid single-adult judgment");

        let PendingKindData::SpeakerIdLowConfidence { suggested } = &entry.data else {
            panic!("expected SpeakerIdLowConfidence kind; got something else");
        };

        assert_eq!(
            suggested.mapping.get("PAR0"),
            Some(&SpeakerAction::Drop),
            "Child verdict must map to Drop"
        );
        assert_eq!(
            suggested.mapping.get("PAR1"),
            Some(&SpeakerAction::Rename),
            "Adult verdict must map to Rename"
        );
        assert_eq!(
            suggested.mapping.get("PAR2"),
            Some(&SpeakerAction::Drop),
            "Drop verdict must map to Drop"
        );

        let role = suggested
            .adult_roles
            .get("PAR1")
            .expect("PAR1 (the sole adult) must have a role entry");
        assert_eq!(role.code, "INV", "role code must be INV for AdultRole::Inv");
        assert_eq!(
            role.tag, "Investigator",
            "role tag must be Investigator for AdultRole::Inv"
        );
        assert!(
            role.specific_role.is_none(),
            "a single adult needs no disambiguation label"
        );
    }

    /// The returned entry must have engine == Llm and a populated judgment
    /// block carrying the reasoning and confidence through.
    #[test]
    fn engine_is_llm_with_populated_judgment() {
        let judgment = test_judgment_with_one_adult();
        let entry = judgment_to_pending("sess-002", &judgment, &test_meta(), fixed_ts())
            .expect("judgment_to_pending should succeed");

        assert_eq!(
            entry.engine,
            DecisionEngine::Llm,
            "engine field must be Llm for LLM-produced pending entries"
        );

        let prov = entry
            .judgment
            .as_ref()
            .expect("judgment provenance block must be Some for an Llm entry");

        assert_eq!(
            prov.reasoning, "PAR0 produces child-like short turns; PAR1 prompts.",
            "reasoning must carry through from the judgment"
        );
        assert_eq!(
            prov.confidence.get(&ConfidenceField::Mapping),
            Some(&Confidence(0.9)),
            "confidence[mapping] must carry through from the judgment"
        );
        assert_eq!(
            prov.model.0, "test-model-v1",
            "model identifier must match the ProvenanceMeta"
        );
        assert_eq!(
            prov.prompt_version.0, "v1",
            "prompt_version must match the ProvenanceMeta"
        );
    }

    /// The kind discriminator on the returned entry must be
    /// SpeakerIdLowConfidence.
    #[test]
    fn kind_is_speaker_id_low_confidence() {
        let judgment = test_judgment_with_one_adult();
        let entry = judgment_to_pending("sess-003", &judgment, &test_meta(), fixed_ts())
            .expect("judgment_to_pending should succeed");

        assert!(
            matches!(entry.data, PendingKindData::SpeakerIdLowConfidence { .. }),
            "data kind must be SpeakerIdLowConfidence; got {:?}",
            entry.data.kind()
        );
    }

    /// Two adult verdicts with two DIFFERENT roles must both appear in
    /// `adult_roles`, no error (this is the behavior change from the
    /// prior `MultipleAdults` refusal).
    #[test]
    fn two_adults_with_different_roles_both_map_to_rename() {
        let judgment = HolisticJudgment {
            speaker_mapping: BTreeMap::from([
                (DonorCode("PAR0".to_string()), SpeakerVerdict::Adult),
                (DonorCode("PAR1".to_string()), SpeakerVerdict::Adult),
            ]),
            adult_roles: BTreeMap::from([
                (DonorCode("PAR0".to_string()), AdultRole::Inv),
                (DonorCode("PAR1".to_string()), AdultRole::Fat),
            ]),
            sample_type: SampleTypeVerdict::Confirmed,
            merge_applicable: true,
            confidence: BTreeMap::new(),
            reasoning: "two adults, different roles".to_string(),
        };

        let entry = judgment_to_pending("sess-two-adults", &judgment, &test_meta(), fixed_ts())
            .expect("two adults with distinct roles must succeed, not MultipleAdults");

        let PendingKindData::SpeakerIdLowConfidence { suggested } = &entry.data else {
            panic!(
                "expected SpeakerIdLowConfidence; got: {:?}",
                entry.data.kind()
            );
        };
        assert_eq!(suggested.mapping.get("PAR0"), Some(&SpeakerAction::Rename));
        assert_eq!(suggested.mapping.get("PAR1"), Some(&SpeakerAction::Rename));
        assert_eq!(
            suggested
                .adult_roles
                .get("PAR0")
                .map(|r| (r.code.as_str(), r.tag.as_str())),
            Some(("INV", "Investigator")),
        );
        assert_eq!(
            suggested
                .adult_roles
                .get("PAR1")
                .map(|r| (r.code.as_str(), r.tag.as_str())),
            Some(("FAT", "Father")),
        );
        assert!(
            suggested
                .adult_roles
                .get("PAR0")
                .unwrap()
                .specific_role
                .is_none(),
            "distinct-role adults need no specific-role disambiguation label"
        );
    }

    /// Two adult verdicts assigned the SAME role must auto-disambiguate:
    /// numbered codes (INV1/INV2) and specific-role labels
    /// (First_Investigator/Second_Investigator), per the CHAT manual's
    /// CHI1/CHI2 convention. Disambiguation order follows BTreeMap
    /// iteration order over the donor codes (alphabetical: PAR0 before
    /// PAR1), so PAR0 becomes "First".
    #[test]
    fn two_adults_with_same_role_auto_disambiguate() {
        let judgment = HolisticJudgment {
            speaker_mapping: BTreeMap::from([
                (DonorCode("PAR0".to_string()), SpeakerVerdict::Adult),
                (DonorCode("PAR1".to_string()), SpeakerVerdict::Adult),
            ]),
            adult_roles: BTreeMap::from([
                (DonorCode("PAR0".to_string()), AdultRole::Inv),
                (DonorCode("PAR1".to_string()), AdultRole::Inv),
            ]),
            sample_type: SampleTypeVerdict::Confirmed,
            merge_applicable: true,
            confidence: BTreeMap::new(),
            reasoning: "two adults, same role".to_string(),
        };

        let entry = judgment_to_pending("sess-same-role", &judgment, &test_meta(), fixed_ts())
            .expect("two adults with the same role must succeed via auto-disambiguation");

        let PendingKindData::SpeakerIdLowConfidence { suggested } = &entry.data else {
            panic!(
                "expected SpeakerIdLowConfidence; got: {:?}",
                entry.data.kind()
            );
        };
        let par0 = suggested
            .adult_roles
            .get("PAR0")
            .expect("PAR0 must have a role");
        let par1 = suggested
            .adult_roles
            .get("PAR1")
            .expect("PAR1 must have a role");
        assert_eq!(par0.code, "INV1");
        assert_eq!(par1.code, "INV2");
        assert_eq!(par0.tag, "Investigator");
        assert_eq!(par1.tag, "Investigator");
        assert_eq!(par0.specific_role.as_deref(), Some("First_Investigator"));
        assert_eq!(par1.specific_role.as_deref(), Some("Second_Investigator"));
    }

    /// An adult verdict with no matching adult_roles entry must return
    /// AdultRoleMissing.
    #[test]
    fn adult_without_role_is_error() {
        let judgment = HolisticJudgment {
            speaker_mapping: BTreeMap::from([(
                DonorCode("PAR0".to_string()),
                SpeakerVerdict::Adult,
            )]),
            // adult_roles deliberately empty; PAR0 has no role assigned.
            adult_roles: BTreeMap::new(),
            sample_type: SampleTypeVerdict::Confirmed,
            merge_applicable: true,
            confidence: BTreeMap::new(),
            reasoning: "adult with no role".to_string(),
        };

        let err = judgment_to_pending("sess-005", &judgment, &test_meta(), fixed_ts())
            .expect_err("adult with no role must produce an error");
        assert!(
            matches!(err, ConsumeError::AdultRoleMissing(ref code) if code == "PAR0"),
            "error must be AdultRoleMissing(PAR0); got: {err}"
        );
    }

    /// merge_applicable=true with no adult speaker is a self-contradictory
    /// judgment; judgment_to_pending must fail closed with NoAdultButMergeApplicable.
    #[test]
    fn merge_applicable_true_without_adult_is_error() {
        let judgment = HolisticJudgment {
            speaker_mapping: BTreeMap::from([
                (DonorCode("PAR0".to_string()), SpeakerVerdict::Child),
                (DonorCode("PAR1".to_string()), SpeakerVerdict::Drop),
            ]),
            adult_roles: BTreeMap::new(),
            sample_type: SampleTypeVerdict::Confirmed,
            // Model says merge applies but provided no adult verdict: contradiction.
            merge_applicable: true,
            confidence: BTreeMap::new(),
            reasoning: "all drop but merge_applicable true".to_string(),
        };

        let err = judgment_to_pending("sess-na-ma", &judgment, &test_meta(), fixed_ts())
            .expect_err("merge_applicable=true with no adult must produce an error");
        assert!(
            matches!(err, ConsumeError::NoAdultButMergeApplicable),
            "error must be NoAdultButMergeApplicable; got: {err}"
        );
    }

    /// merge_applicable=false with no adult succeeds and the typed
    /// `merge_applicable` field on the provenance block is `false`, while
    /// `reasoning` carries the original model text with NO prefix prepended.
    #[test]
    fn merge_not_applicable_recorded_as_typed_field() {
        let judgment = HolisticJudgment {
            speaker_mapping: BTreeMap::from([
                (DonorCode("PAR0".to_string()), SpeakerVerdict::Drop),
                (DonorCode("PAR1".to_string()), SpeakerVerdict::Child),
            ]),
            adult_roles: BTreeMap::new(),
            sample_type: SampleTypeVerdict::Confirmed,
            merge_applicable: false,
            confidence: BTreeMap::new(),
            reasoning: "monologic reading, no adult".to_string(),
        };

        let entry = judgment_to_pending("sess-nma-reason", &judgment, &test_meta(), fixed_ts())
            .expect("merge_applicable=false with no adult must succeed");

        let prov = entry
            .judgment
            .as_ref()
            .expect("judgment provenance block must be Some");

        assert!(
            !prov.merge_applicable,
            "merge_applicable typed field must be false when judgment reports non-mergeable"
        );
        assert_eq!(
            prov.reasoning, "monologic reading, no adult",
            "reasoning must carry the original model text verbatim, with no prefix"
        );
    }

    /// merge_applicable=false with no adult succeeds (not an error) and
    /// produces an all-Drop mapping with an empty `adult_roles` map (no
    /// adult means nothing to assign a role to; there is no placeholder).
    #[test]
    fn merge_not_applicable_with_no_adult_yields_empty_adult_roles() {
        let judgment = HolisticJudgment {
            speaker_mapping: BTreeMap::from([
                (DonorCode("PAR0".to_string()), SpeakerVerdict::Drop),
                (DonorCode("PAR1".to_string()), SpeakerVerdict::Child),
            ]),
            adult_roles: BTreeMap::new(),
            sample_type: SampleTypeVerdict::Confirmed,
            merge_applicable: false,
            confidence: BTreeMap::new(),
            reasoning: "monologic, no adult".to_string(),
        };

        let entry = judgment_to_pending("sess-nma-ok", &judgment, &test_meta(), fixed_ts())
            .expect("merge_applicable=false no-adult case must succeed (not an error)");

        let PendingKindData::SpeakerIdLowConfidence { suggested } = &entry.data else {
            panic!(
                "expected SpeakerIdLowConfidence; got: {:?}",
                entry.data.kind()
            );
        };

        // All verdicts are Drop/Child, so all actions must be Drop.
        for (code, action) in &suggested.mapping {
            assert_eq!(
                action,
                &SpeakerAction::Drop,
                "speaker {code} must map to Drop when merge_applicable=false and no adult"
            );
        }

        // No adult was named, so adult_roles must be empty, not a
        // placeholder entry (there is no speaker to assign a role to;
        // mapping is all-Drop).
        assert!(
            suggested.adult_roles.is_empty(),
            "adult_roles must be empty when there is no adult, not a placeholder entry"
        );
    }

    /// AdultRole::Slp.inserted_role_spec() must map to tag "Therapist", the
    /// only clinical-adult role in chatter's valid participant-role set.
    #[test]
    fn slp_maps_to_therapist() {
        let spec = AdultRole::Slp.inserted_role_spec();
        assert_eq!(spec.code, "SLP", "SLP inserted_role code must be 'SLP'");
        assert_eq!(
            spec.tag, "Therapist",
            "SLP inserted_role tag must be 'Therapist' (the valid chatter role for a clinician)"
        );
    }

    /// Round-trip: build a PendingEntry via judgment_to_pending, wrap it in a
    /// PendingAdjudications, serialize to TOML, confirm `engine = "llm"` and a
    /// reasoning field are present, and confirm it parses back equal to the
    /// original entry.
    #[test]
    fn round_trip_pending_adjudications_contains_llm_engine_and_reasoning() {
        let judgment = test_judgment_with_one_adult();
        let entry = judgment_to_pending("sess-rt", &judgment, &test_meta(), fixed_ts())
            .expect("judgment_to_pending should succeed");

        let doc = PendingAdjudications {
            schema_version: PendingAdjudications::CURRENT_SCHEMA_VERSION,
            entries: vec![entry.clone()],
        };

        let toml_str = toml::to_string_pretty(&doc).expect("PendingAdjudications must serialize");

        assert!(
            toml_str.contains("engine = \"llm\""),
            "serialized TOML must contain 'engine = \"llm\"'; got:\n{toml_str}"
        );
        assert!(
            toml_str.contains("reasoning"),
            "serialized TOML must contain a 'reasoning' field; got:\n{toml_str}"
        );

        let back: PendingAdjudications =
            toml::from_str(&toml_str).expect("serialized TOML must parse back");

        assert_eq!(
            back.entries.len(),
            1,
            "round-tripped document must have exactly one entry"
        );

        let back_entry = &back.entries[0];
        assert_eq!(
            back_entry.engine,
            DecisionEngine::Llm,
            "engine must round-trip as Llm"
        );
        assert_eq!(
            back_entry.judgment.as_ref().map(|p| p.reasoning.as_str()),
            Some("PAR0 produces child-like short turns; PAR1 prompts."),
            "reasoning must round-trip verbatim"
        );
        assert_eq!(
            back_entry.judgment.as_ref().map(|p| p.merge_applicable),
            Some(true),
            "merge_applicable must round-trip (true for the standard one-adult fixture)"
        );
        assert_eq!(
            back_entry.session_id, entry.session_id,
            "session_id must round-trip unchanged"
        );
    }
}
