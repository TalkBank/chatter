//! L4 tests for the adjudication core.
//!
//! These tests exercise `run_adjudication` directly, with a
//! `ScriptedPrompter` standing in for human operator input. This
//! is the testability seam described in
//! `book/src/architecture/adjudication-workflow.md`, every
//! operator-decision path is exercisable without subprocess PTY
//! hackery.

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use talkbank_transform::adjudication::AdjudicationKind;
use talkbank_transform::adjudication::{
    OperatorDecision, PendingAdjudications, PendingEntry, PendingKindData, ScriptedPrompter,
    SuggestedSpeakerIdMapping, run_adjudication,
};
use talkbank_transform::speaker_id::{
    DecisionEngine, InsertedRoleSpec, OverrideFile, OverrideMode, SpeakerAction,
};

/// `AcceptSuggested` on a speaker-id-low-confidence entry: the
/// operator looks at the suggested mapping (the algorithm's choice
/// had the threshold been lower) and signs off on it. The
/// adjudication core records an `Explicit`-mode entry in the
/// override file carrying the suggested mapping verbatim, and
/// removes the pending entry.
#[test]
fn adjudicate_speaker_id_accepts_suggested() {
    // Build a pending entry: one speaker-id-low-confidence case
    // where the algorithm picked PAR0=drop, PAR1=rename(INV,
    // Investigator) but the margin fell below threshold.
    let suggested_mapping: BTreeMap<String, SpeakerAction> = [
        ("PAR0".to_string(), SpeakerAction::Drop),
        ("PAR1".to_string(), SpeakerAction::Rename),
    ]
    .into_iter()
    .collect();

    let mut pending = PendingAdjudications {
        schema_version: 1,
        entries: vec![PendingEntry {
            session_id: "session-102-t1".to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 5, 27, 11, 0, 0).unwrap(),
            data: PendingKindData::SpeakerIdLowConfidence {
                suggested: SuggestedSpeakerIdMapping {
                    mapping: suggested_mapping.clone(),
                    inserted_role: InsertedRoleSpec {
                        code: "INV".to_string(),
                        tag: "Investigator".to_string(),
                    },
                },
            },
            scores: BTreeMap::from([("PAR0".to_string(), 0.6286), ("PAR1".to_string(), 0.3457)]),
            margin: Some(1.82),
            threshold_used: Some(2.0),
            engine: DecisionEngine::Deterministic,
            judgment: None,
        }],
    };
    let mut overrides = OverrideFile::default();

    // ScriptedPrompter answers AcceptSuggested for this session.
    let mut prompter = ScriptedPrompter::from_decisions(vec![(
        "session-102-t1".to_string(),
        OperatorDecision::AcceptSuggested { note: None },
    )]);

    let outcome = run_adjudication(
        &mut pending,
        &mut overrides,
        &mut prompter,
        "test-operator".to_string(),
    )
    .expect("run_adjudication should succeed on a single AcceptSuggested decision");

    // The outcome reports one resolved decision.
    assert_eq!(
        outcome.resolved_count(),
        1,
        "one decision should have been resolved; outcome: {outcome:?}"
    );

    // The override file has the session's decision: mode=Explicit,
    // mapping matches the suggested mapping, inserted_role matches.
    let entry = overrides
        .get("session-102-t1")
        .expect("override file should contain the session's entry");
    assert_eq!(
        entry.mode,
        OverrideMode::Explicit,
        "AcceptSuggested decision should record as Explicit mode (operator signed off)"
    );
    assert_eq!(
        entry.mapping, suggested_mapping,
        "mapping should match the suggested mapping verbatim"
    );
    assert_eq!(
        entry.inserted_role.code, "INV",
        "inserted_role.code should match the suggested INV"
    );
    assert_eq!(
        entry.inserted_role.tag, "Investigator",
        "inserted_role.tag should match the suggested Investigator"
    );
    assert_eq!(
        entry.operator, "test-operator",
        "operator field should be the one supplied to run_adjudication"
    );

    // The pending file has no entries left.
    assert!(
        pending.entries.is_empty(),
        "pending entries should be empty after resolution; got: {:?}",
        pending.entries
    );
}

/// `OverrideMapping` on a speaker-id-low-confidence entry: the
/// operator looked at the algorithm's suggestion, listened to the
/// audio, and decided the OPPOSITE, PAR1 (not PAR0) is the
/// anchor-match, and PAR0 is the inserted speaker. The override
/// file records the operator's choice (not the algorithm's
/// suggestion) and ignores the suggested mapping entirely.
#[test]
fn adjudicate_speaker_id_override_mapping() {
    // Algorithm suggested PAR0=drop, PAR1=rename, but suppose the
    // operator listened to the recording and concluded the algorithm
    // got it backwards.
    let suggested_mapping: BTreeMap<String, SpeakerAction> = [
        ("PAR0".to_string(), SpeakerAction::Drop),
        ("PAR1".to_string(), SpeakerAction::Rename),
    ]
    .into_iter()
    .collect();
    // Operator's actual decision: PAR1=drop, PAR0=rename.
    let operator_mapping: BTreeMap<String, SpeakerAction> = [
        ("PAR0".to_string(), SpeakerAction::Rename),
        ("PAR1".to_string(), SpeakerAction::Drop),
    ]
    .into_iter()
    .collect();

    let mut pending = PendingAdjudications {
        schema_version: 1,
        entries: vec![PendingEntry {
            session_id: "session-204-t1".to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 5, 27, 11, 0, 0).unwrap(),
            data: PendingKindData::SpeakerIdLowConfidence {
                suggested: SuggestedSpeakerIdMapping {
                    mapping: suggested_mapping,
                    inserted_role: InsertedRoleSpec {
                        code: "INV".to_string(),
                        tag: "Investigator".to_string(),
                    },
                },
            },
            scores: BTreeMap::from([("PAR0".to_string(), 0.5500), ("PAR1".to_string(), 0.4700)]),
            margin: Some(1.17),
            threshold_used: Some(2.0),
            engine: DecisionEngine::Deterministic,
            judgment: None,
        }],
    };
    let mut overrides = OverrideFile::default();

    let mut prompter = ScriptedPrompter::from_decisions(vec![(
        "session-204-t1".to_string(),
        OperatorDecision::OverrideMapping {
            mapping: operator_mapping.clone(),
            inserted_role: InsertedRoleSpec {
                code: "MOT".to_string(),
                tag: "Mother".to_string(),
            },
            note: Some("audio review: PAR1 voice matches the child".to_string()),
        },
    )]);

    let outcome = run_adjudication(
        &mut pending,
        &mut overrides,
        &mut prompter,
        "test-operator".to_string(),
    )
    .expect("run_adjudication should succeed on a single OverrideMapping decision");

    assert_eq!(outcome.resolved_count(), 1);

    let entry = overrides
        .get("session-204-t1")
        .expect("override file should contain the session's entry");
    // The recorded mapping is the operator's, NOT the algorithm's
    // suggestion. This is the whole point of OverrideMapping.
    assert_eq!(
        entry.mapping, operator_mapping,
        "OverrideMapping should write the operator's mapping verbatim, not the suggestion"
    );
    // Inserted role also comes from the operator, not from the
    // pending entry's suggestion (the operator may pick a totally
    // different role).
    assert_eq!(
        entry.inserted_role.code, "MOT",
        "inserted_role.code should be the operator-supplied MOT"
    );
    assert_eq!(
        entry.inserted_role.tag, "Mother",
        "inserted_role.tag should be the operator-supplied Mother"
    );
    // OverrideMapping is recorded as Explicit mode (operator-made).
    assert_eq!(entry.mode, OverrideMode::Explicit);
    // The note rides through.
    assert!(
        entry
            .note
            .as_deref()
            .is_some_and(|s| s.contains("audio review")),
        "operator note should be recorded; got: {:?}",
        entry.note
    );
    // Scores and margin from the pending entry are preserved (audit
    // trail of WHY the operator was asked).
    assert!(
        !entry.scores.is_empty(),
        "scores from the pending entry should ride through to the override entry"
    );
    assert_eq!(entry.margin, Some(1.17));

    assert!(pending.entries.is_empty());
}

/// Parent-role-lookup adjudication: the donor file already has a
/// speaker identified as the parent, but the role tag (`MOT` vs
/// `FAT`) is unknown to the pipeline. The operator looks at the
/// contributor data sheet (or listens to the audio) and picks one.
/// No Jaccard work; no `suggested.mapping`, only an `InsertedRoleSpec`
/// gets recorded against the speaker.
#[test]
fn adjudicate_parent_role_lookup_chooses_role() {
    let speaker_mapping: BTreeMap<String, talkbank_transform::speaker_id::SpeakerAction> = [(
        "PAR".to_string(),
        talkbank_transform::speaker_id::SpeakerAction::Rename,
    )]
    .into_iter()
    .collect();

    let mut pending = PendingAdjudications {
        schema_version: 1,
        entries: vec![PendingEntry {
            session_id: "session-307-parent".to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
            data: PendingKindData::ParentRoleLookup {
                donor_speaker: "PAR".to_string(),
                speaker_mapping: speaker_mapping.clone(),
            },
            scores: BTreeMap::new(),
            margin: None,
            threshold_used: None,
            engine: DecisionEngine::Deterministic,
            judgment: None,
        }],
    };
    let mut overrides = OverrideFile::default();

    // Operator picks MOT (mother).
    let mut prompter = ScriptedPrompter::from_decisions(vec![(
        "session-307-parent".to_string(),
        OperatorDecision::ChooseRole {
            inserted_role: InsertedRoleSpec {
                code: "MOT".to_string(),
                tag: "Mother".to_string(),
            },
            note: Some("contributor data sheet: mother sample".to_string()),
        },
    )]);

    let outcome = run_adjudication(
        &mut pending,
        &mut overrides,
        &mut prompter,
        "test-operator".to_string(),
    )
    .expect("run_adjudication should succeed on a single ChooseRole decision");

    assert_eq!(outcome.resolved_count(), 1);

    let entry = overrides
        .get("session-307-parent")
        .expect("override file should contain the parent-role decision");
    // The chosen role is recorded.
    assert_eq!(entry.inserted_role.code, "MOT");
    assert_eq!(entry.inserted_role.tag, "Mother");
    // The pre-recorded mapping rides through.
    assert_eq!(entry.mapping, speaker_mapping);
    // Mode is Explicit (operator picked).
    assert_eq!(entry.mode, OverrideMode::Explicit);
    // Note rides through.
    assert!(
        entry
            .note
            .as_deref()
            .is_some_and(|s| s.contains("data sheet")),
        "operator note should be preserved; got: {:?}",
        entry.note
    );
    // No scores/margin/threshold, parent-role-lookup doesn't have them.
    assert!(entry.scores.is_empty());
    assert!(entry.margin.is_none());

    // Kind discriminator is also exposed on PendingEntry.
    let _ = AdjudicationKind::ParentRoleLookup; // ensures the variant exists

    assert!(pending.entries.is_empty());
}

/// `AcceptSuggested` on a sanity-scan misclassification entry: the
/// operator looks at the swap the post-merge scan flagged (e.g.,
/// anchor's mean utterance length exceeds INV's, child usually
/// shorter than adult), and signs off on the corrected mapping. The
/// adjudication core records an `Explicit`-mode override entry with
/// the scan's suggested mapping verbatim, ready for pass-2 re-merge.
#[test]
fn adjudicate_sanity_scan_accept_suggested() {
    // Suppose pass-1 ran reference mode and confidently merged with
    // PAR0=drop, PAR1=rename(INV). A post-merge scan computed mean
    // utterance length per surviving speaker and flagged the result
    //, CHI's mean is higher than INV's, which is the wrong way
    // around for typical child-adult transcripts. The suggested
    // mapping swaps the original.
    let suggested_mapping: BTreeMap<String, SpeakerAction> = [
        ("PAR0".to_string(), SpeakerAction::Rename),
        ("PAR1".to_string(), SpeakerAction::Drop),
    ]
    .into_iter()
    .collect();

    let mut pending = PendingAdjudications {
        schema_version: 1,
        entries: vec![PendingEntry {
            session_id: "session-455-t2".to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 5, 28, 12, 0, 0).unwrap(),
            data: PendingKindData::SanityScanMisclassification {
                suggested: SuggestedSpeakerIdMapping {
                    mapping: suggested_mapping.clone(),
                    inserted_role: InsertedRoleSpec {
                        code: "INV".to_string(),
                        tag: "Investigator".to_string(),
                    },
                },
                reason: "anchor CHI mean utterance length 8.4 exceeds INV mean 3.1; \
                         child typically shorter than adult, likely swap"
                    .to_string(),
            },
            scores: BTreeMap::new(),
            margin: None,
            threshold_used: None,
            engine: DecisionEngine::Deterministic,
            judgment: None,
        }],
    };
    let mut overrides = OverrideFile::default();

    let mut prompter = ScriptedPrompter::from_decisions(vec![(
        "session-455-t2".to_string(),
        OperatorDecision::AcceptSuggested {
            note: Some("audio review confirms scan's swap".to_string()),
        },
    )]);

    let outcome = run_adjudication(
        &mut pending,
        &mut overrides,
        &mut prompter,
        "test-operator".to_string(),
    )
    .expect("run_adjudication should succeed on AcceptSuggested for sanity-scan");

    assert_eq!(outcome.resolved_count(), 1);

    let entry = overrides
        .get("session-455-t2")
        .expect("override file should contain the sanity-scan decision");
    assert_eq!(
        entry.mapping, suggested_mapping,
        "mapping should match the scan's suggested (swapped) mapping verbatim"
    );
    assert_eq!(entry.inserted_role.code, "INV");
    assert_eq!(entry.inserted_role.tag, "Investigator");
    assert_eq!(
        entry.mode,
        OverrideMode::Explicit,
        "AcceptSuggested on sanity-scan still records as Explicit \
         (operator signed off; not the algorithm's auto-decision)"
    );
    assert!(
        entry
            .note
            .as_deref()
            .is_some_and(|s| s.contains("audio review")),
        "operator note should be preserved; got: {:?}",
        entry.note
    );

    // Kind discriminator is exposed.
    let _ = AdjudicationKind::SanityScanMisclassification;

    assert!(pending.entries.is_empty());
}
