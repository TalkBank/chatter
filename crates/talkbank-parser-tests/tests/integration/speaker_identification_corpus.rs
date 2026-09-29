//! Reference-mode lexical evidence is advisory, not speaker-identity authority.

use talkbank_model::{ChatFile, SpeakerCode};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{repo_paths::workspace_root, test_error::strict_parse};
use talkbank_transform::speaker_id::{
    ConfidenceMargin, ConfidenceThreshold, ParseConfidenceThresholdError,
    RecordedSpeakerIdentificationAttempt, SpeakerIdError, apply_mapping_chat, identify_mapping,
    parse_mapping_spec,
};

fn reference(parser: &TreeSitterParser, fixture: &str) -> ChatFile {
    let source = std::fs::read_to_string(workspace_root().join("corpus/reference").join(fixture))
        .expect("canonical reference CHAT");
    strict_parse(parser.parse_chat_file(&source)).expect("reference syntax")
}

#[test]
fn reference_speaker_advice_requires_pending_review_and_consistent_roles() {
    use talkbank_transform::adjudication::PendingKindData;
    use talkbank_transform::speaker_id::judgment::{
        CURRENT_PROMPT_VERSION, ConsumeError, HolisticJudgment, ProvenanceMeta, SampleBudget,
        judgment_to_pending, sample_session,
    };
    use talkbank_transform::speaker_id::{
        DecisionEngine, EndpointUrl, ModelId, PromptVersion, SpeakerAction,
    };
    let parser = TreeSitterParser::new().expect("parser");
    let chat = reference(&parser, "core/basic-conversation.cha");
    let anchor = chat
        .utterances()
        .next()
        .expect("reference speech")
        .main
        .speaker
        .clone();
    let samples = sample_session(
        &chat,
        &anchor,
        SampleBudget {
            head: 1,
            tail: 1,
            char_cap: 500,
        },
    );
    let mut codes: Vec<_> = samples
        .iter()
        .map(|speaker| speaker.code.as_str())
        .collect();
    codes.sort_unstable();
    assert_eq!(codes.len(), 2);
    let meta = ProvenanceMeta {
        model: ModelId("authored-response-control".into()),
        endpoint: EndpointUrl("https://example.invalid/no-network".into()),
        prompt_version: PromptVersion(CURRENT_PROMPT_VERSION.into()),
    };
    let timestamp = "2000-01-01T00:00:00Z".parse().expect("authored timestamp");
    // Responses are authored transport controls, not identity findings about
    // these participants. Even maximal declared confidence stays advisory.
    let base = serde_json::json!({
        "speaker_mapping": {codes[0]: "CHI", codes[1]: "adult"},
        "adult_roles": {codes[1]: "MOT"},
        "sample_type": "uncertain", "merge_applicable": true,
        "confidence": {"mapping": 1.0, "roles": 1.0, "merge_applicable": 1.0},
        "reasoning": "Authored control; human review required."
    });
    for sample_type in ["confirmed", "uncertain"] {
        let mut wire = base.clone();
        wire["sample_type"] = sample_type.into();
        let judgment: HolisticJudgment =
            serde_json::from_value(wire).expect("recognized response tag");
        assert!(matches!(
            (&judgment.sample_type, sample_type),
            (
                talkbank_transform::speaker_id::SampleTypeVerdict::Confirmed,
                "confirmed"
            ) | (
                talkbank_transform::speaker_id::SampleTypeVerdict::Uncertain,
                "uncertain"
            )
        ));
        let pending = judgment_to_pending("reference-control", &judgment, &meta, timestamp)
            .expect("recognized advice remains pending");
        assert!(matches!(
            pending.data,
            PendingKindData::SpeakerIdLowConfidence { .. }
        ));
    }
    for unsupported in ["CONFIRMED", "confirmed extra", "corrected", ""] {
        let mut wire = base.clone();
        wire["sample_type"] = unsupported.into();
        let error = serde_json::from_value::<HolisticJudgment>(wire)
            .expect_err("unknown response tag must not become uncertain silently");
        assert!(
            error
                .to_string()
                .contains("expected confirmed | uncertain | corrected:<type>")
        );
    }
    for (role, tag) in [
        ("INV", "Investigator"),
        ("SLP", "Therapist"),
        ("MOT", "Mother"),
        ("FAT", "Father"),
    ] {
        for shared_role in [false, true] {
            let mut wire = base.clone();
            wire["adult_roles"][codes[1]] = role.into();
            if shared_role {
                wire["speaker_mapping"][codes[0]] = "adult".into();
                wire["adult_roles"][codes[0]] = role.into();
            }
            let judgment: HolisticJudgment = serde_json::from_value(wire).expect("response wire");
            let entry = judgment_to_pending("reference-control", &judgment, &meta, timestamp)
                .expect("consistent advice yields pending review");
            assert_eq!(entry.engine, DecisionEngine::Llm);
            assert_eq!(entry.created_at, timestamp);
            assert!(
                entry.scores.is_empty() && entry.margin.is_none() && entry.threshold_used.is_none()
            );
            let provenance = entry.judgment.as_ref().expect("advisory provenance");
            assert_eq!(provenance.model, meta.model);
            assert_eq!(provenance.endpoint, meta.endpoint);
            assert_eq!(provenance.prompt_version, meta.prompt_version);
            assert_eq!(provenance.confidence, judgment.confidence);
            assert_eq!(provenance.reasoning, judgment.reasoning);
            assert!(provenance.merge_applicable);
            let PendingKindData::SpeakerIdLowConfidence { suggested } = entry.data else {
                panic!("advice must remain a pending speaker-mapping proposal");
            };
            assert_eq!(suggested.mapping[codes[1]], SpeakerAction::Rename);
            assert_eq!(
                suggested.mapping[codes[0]],
                if shared_role {
                    SpeakerAction::Rename
                } else {
                    SpeakerAction::Drop
                }
            );
            assert_eq!(suggested.adult_roles.len(), if shared_role { 2 } else { 1 });
            for (index, code) in codes.iter().enumerate() {
                if let Some(spec) = suggested.adult_roles.get(*code) {
                    assert_eq!(spec.tag, tag);
                    if shared_role {
                        assert_eq!(spec.code, format!("{role}{}", index + 1));
                        let ordinal = ["First", "Second"][index];
                        assert_eq!(
                            spec.specific_role.as_deref(),
                            Some(format!("{ordinal}_{tag}").as_str())
                        );
                    } else {
                        assert_eq!(spec.code, role);
                        assert!(spec.specific_role.is_none());
                    }
                }
            }
        }
    }
    let mut missing_role = base.clone();
    missing_role["adult_roles"] = serde_json::json!({});
    let judgment: HolisticJudgment =
        serde_json::from_value(missing_role).expect("role omission wire");
    assert!(
        matches!(judgment_to_pending("reference-control", &judgment, &meta, timestamp),
        Err(ConsumeError::AdultRoleMissing(code)) if code == codes[1])
    );
    for applicable in [false, true] {
        let mut wire = base.clone();
        wire["speaker_mapping"][codes[1]] = "drop".into();
        wire["adult_roles"] = serde_json::json!({});
        wire["merge_applicable"] = applicable.into();
        let judgment: HolisticJudgment = serde_json::from_value(wire).expect("all-drop response");
        match (
            applicable,
            judgment_to_pending("reference-control", &judgment, &meta, timestamp),
        ) {
            (true, Err(ConsumeError::NoAdultButMergeApplicable)) => {}
            (false, Ok(entry)) => {
                let PendingKindData::SpeakerIdLowConfidence { suggested } = entry.data else {
                    panic!("pending advice");
                };
                assert!(
                    suggested
                        .mapping
                        .values()
                        .all(|action| *action == SpeakerAction::Drop)
                );
                assert!(suggested.adult_roles.is_empty());
                assert!(!entry.judgment.expect("provenance").merge_applicable);
            }
            (_, result) => panic!("unexpected merge-applicability result: {result:?}"),
        }
    }
}

#[test]
fn reference_judgment_prompts_preserve_samples_and_explicit_context() {
    use talkbank_transform::speaker_id::judgment::{
        CURRENT_PROMPT_VERSION, JudgmentRequest, Role, SampleBudget, SessionContextFile, SessionId,
        render_messages, sample_session, session_context,
    };
    let parser = TreeSitterParser::new().expect("parser");
    for fixture in [
        "core/basic-conversation.cha",
        "audio/chinese-adult-conversation.cha",
    ] {
        let chat = reference(&parser, fixture);
        let anchor = chat
            .utterances()
            .next()
            .expect("reference speech")
            .main
            .speaker
            .clone();
        // Sidecar values are explicit caller controls, not demographic or
        // consent claims inferred from the reference transcript.
        for authored_age in [
            None,
            Some(17),
            Some(18),
            Some(35),
            Some(36),
            Some(71),
            Some(72),
        ] {
            let sidecar: Option<SessionContextFile> = authored_age.map(|months| {
                serde_json::from_value(serde_json::json!({fixture: {
                    "age_months": months,
                    "sample_type": "authored context",
                    "declared_roles": ["Mother", "authored role"],
                    "consent_tier": "authored consent"
                }}))
                .expect("admitted sidecar labels")
            });
            let context = session_context(sidecar.as_ref(), fixture, &chat);
            let samples = sample_session(
                &chat,
                &anchor,
                SampleBudget {
                    head: 1,
                    tail: 1,
                    char_cap: 500,
                },
            );
            let request = JudgmentRequest {
                session_id: SessionId(fixture.to_owned()),
                sample_type: context.sample_type,
                declared_roles: context.declared_roles,
                consent_tier: context.consent_tier,
                age_months: context.age_months,
                anchor: anchor.clone(),
                samples,
            };
            let (messages, version) = render_messages(&request);
            assert_eq!(version.0, CURRENT_PROMPT_VERSION);
            assert_eq!(messages.len(), 2);
            assert_eq!(messages[0].role, Role::System);
            assert_eq!(messages[1].role, Role::User);
            let user = &messages[1].content;
            assert!(user.starts_with(&format!("session_id: {fixture}\n")));
            match authored_age {
                Some(_) => {
                    assert!(user.contains("sample_type: authored context\n"));
                    assert!(user.contains("declared_adult_roles: Mother, authored role\n"));
                    assert!(user.contains("consent_tier: authored consent\n"));
                }
                None => {
                    assert!(user.contains("sample_type: unknown\n"));
                    assert!(user.contains("declared_adult_roles: none\n"));
                    assert!(user.contains("consent_tier: unknown\n"));
                }
            }
            match request.age_months {
                None => assert!(!user.contains("child_age_months:")),
                Some(age) => {
                    let band = match age.0 {
                        0..=17 => "babbling / pre-linguistic",
                        18..=35 => "early multiword",
                        36..=71 => "preschool",
                        _ => "school-age",
                    };
                    assert!(user.contains(&format!("child_age_months: {} ({band})\n", age.0)));
                }
            }
            let mut rendered_samples = String::new();
            for speaker in &request.samples {
                rendered_samples.push_str(&format!("--- speaker: {} ---\n", speaker.code));
                for (i, utterance) in speaker.utterances.iter().enumerate() {
                    rendered_samples.push_str(&format!("{}: {}\n", i + 1, utterance.0));
                }
            }
            assert!(!rendered_samples.is_empty());
            assert!(
                user.ends_with(&rendered_samples),
                "ordered samples and Unicode survive rendering"
            );
            assert_eq!(
                render_messages(&request),
                (messages, version),
                "deterministic prompt"
            );
        }
    }
}

#[test]
fn age_specs_do_not_promote_unsupported_age_into_judgment_context() {
    use talkbank_transform::speaker_id::{SessionContextFile, session_context};
    let parser = TreeSitterParser::new().expect("parser");
    for (case, months) in [
        (6, Some(30)),
        (7, Some(30)),
        (8, None),
        (12, None),
        (13, None),
    ] {
        let path = workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E517_{case}.cha"
        ));
        let source = std::fs::read_to_string(path).expect("generated age spec");
        // Syntax admission does not certify valid age metadata. Unsupported
        // input must stay unknown, not be reconstructed from its numeric prefix.
        let chat = strict_parse(parser.parse_chat_file(&source)).expect("age field syntax");
        assert_eq!(
            session_context(None, "age-spec", &chat)
                .age_months
                .map(|age| age.0),
            months,
            "E517_{case}"
        );
        let configured: SessionContextFile = serde_json::from_value(serde_json::json!({
            "age-spec": {"age_months": 48}
        }))
        .expect("explicit age");
        assert_eq!(
            session_context(Some(&configured), "age-spec", &chat)
                .age_months
                .map(|age| age.0),
            Some(48)
        );
    }
}

#[test]
fn reference_context_preserves_explicit_labels_age_precedence_and_unknowns() {
    use talkbank_transform::speaker_id::{SessionContextFile, session_context};
    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, header_months) in [
        ("core/basic-conversation.cha", Some(36)),
        ("core/headers-speaker-info.cha", Some(20)),
        ("audio/chinese-adult-conversation.cha", None),
    ] {
        // These controls have at most one declared age. They do not establish
        // which speaker should supply age when multiple participants have it.
        let chat = reference(&parser, fixture);
        let absent = session_context(None, fixture, &chat);
        assert_eq!(absent.age_months.map(|age| age.0), header_months);
        assert!(absent.sample_type.is_none());
        assert!(absent.consent_tier.is_none());
        assert!(absent.declared_roles.is_empty());
        let empty: SessionContextFile = serde_json::from_value(serde_json::json!({fixture: {}}))
            .expect("empty record means unknown fields");
        assert_eq!(session_context(Some(&empty), fixture, &chat), absent);
        assert_eq!(
            session_context(Some(&empty), "unlisted-session", &chat),
            absent
        );

        // Sidecar values are authored operator-input controls, not newly
        // inferred facts about these reference participants or media consent.
        let mut record = serde_json::json!({
            "sample_type": " authored sample label ",
            "declared_roles": ["Mother", " authored role label "],
            "consent_tier": " authored consent label "
        });
        for override_age in [None, Some(48)] {
            if let Some(age) = override_age {
                record["age_months"] = age.into();
            }
            let context: SessionContextFile =
                serde_json::from_value(serde_json::json!({fixture: record}))
                    .expect("admit explicitly configured labels");
            let resolved = session_context(Some(&context), fixture, &chat);
            assert_eq!(
                resolved.age_months.map(|age| age.0),
                override_age.or(header_months)
            );
            let sample = resolved
                .sample_type
                .as_ref()
                .expect("configured sample label");
            assert_eq!(sample.as_str(), " authored sample label ");
            assert_eq!(sample.to_string(), sample.as_str());
            // Authored response controls exercise the wire boundary, not an
            // external model judgment or an inferred fact about the recording.
            let corrected: talkbank_transform::speaker_id::SampleTypeVerdict =
                serde_json::from_value(serde_json::json!(format!("corrected:{}", sample.as_str())))
                    .expect("corrected label admission");
            let talkbank_transform::speaker_id::SampleTypeVerdict::Corrected(label) = corrected
            else {
                panic!("corrected wire must retain its tag");
            };
            assert_eq!(label.as_str(), sample.as_str().trim());
            let consent = resolved
                .consent_tier
                .as_ref()
                .expect("configured consent label");
            assert_eq!(consent.as_str(), " authored consent label ");
            assert_eq!(consent.to_string(), consent.as_str());
            assert_eq!(
                resolved
                    .declared_roles
                    .iter()
                    .map(|role| role.as_str())
                    .collect::<Vec<_>>(),
                ["Mother", " authored role label "]
            );
            assert_eq!(
                resolved.declared_roles[1].to_string(),
                " authored role label "
            );
        }
        for invalid in [
            serde_json::json!({"sample_type": ""}),
            serde_json::json!({"declared_roles": [" "]}),
            serde_json::json!({"consent_tier": "\t"}),
            serde_json::json!({"age_months": -1}),
            serde_json::json!({"age_months": 2.5}),
            serde_json::json!({"undeclared_field": "value"}),
        ] {
            assert!(
                serde_json::from_value::<SessionContextFile>(serde_json::json!({fixture: invalid}))
                    .is_err(),
                "invalid configured context must not become unknown silently"
            );
        }
        for blank in ["corrected:", "corrected: \t"] {
            let error =
                serde_json::from_value::<talkbank_transform::speaker_id::SampleTypeVerdict>(
                    serde_json::json!(blank),
                )
                .expect_err("blank correction cannot construct a label");
            assert!(
                error
                    .to_string()
                    .contains("corrected: requires a non-empty sample-type label")
            );
        }
    }
}

#[test]
fn reference_session_context_file_admission_preserves_evidence_and_refuses_bad_inputs() {
    use std::io::Write;
    use talkbank_transform::speaker_id::{
        SessionContextError, SessionContextFile, session_context,
    };

    let fixture = "core/basic-conversation.cha";
    let parser = TreeSitterParser::new().expect("parser");
    let chat = reference(&parser, fixture);
    // These labels are supplied by an operator, not inferred demographic or
    // consent facts about the reference recording.
    let wire = serde_json::json!({fixture: {
        "age_months": 48,
        "sample_type": " authored sample ",
        "declared_roles": [" authored role "],
        "consent_tier": " authored consent "
    }});
    let memory: SessionContextFile = serde_json::from_value(wire.clone()).expect("typed context");
    let mut file = tempfile::NamedTempFile::new().expect("context destination");
    serde_json::to_writer(&mut file, &wire).expect("write operator context");
    file.flush().expect("flush context");
    let loaded = SessionContextFile::read_json(file.path()).expect("read typed context");
    assert_eq!(
        session_context(Some(&loaded), fixture, &chat),
        session_context(Some(&memory), fixture, &chat)
    );
    assert_eq!(
        session_context(Some(&loaded), "unlisted", &chat),
        session_context(None, "unlisted", &chat)
    );

    enum Refusal {
        Syntax,
        Shape,
        Encoding,
    }
    let cases = [
        (b"{".to_vec(), Refusal::Syntax),
        (
            serde_json::to_vec(&serde_json::json!({fixture: {"age_months": -1}})).expect("JSON"),
            Refusal::Shape,
        ),
        (
            serde_json::to_vec(&serde_json::json!({fixture: {"consent_tier": "\t"}}))
                .expect("JSON"),
            Refusal::Shape,
        ),
        (
            serde_json::to_vec(&serde_json::json!({fixture: {"unknown_field": true}}))
                .expect("JSON"),
            Refusal::Shape,
        ),
        (vec![0xff], Refusal::Encoding),
    ];
    for (bytes, expected) in cases {
        let mut file = tempfile::NamedTempFile::new().expect("refusal destination");
        file.write_all(&bytes).expect("write external input");
        file.flush().expect("flush input");
        let error = SessionContextFile::read_json(file.path())
            .expect_err("configured input must fail loudly");
        assert!(
            error
                .to_string()
                .contains(&file.path().display().to_string())
        );
        match (expected, error) {
            (Refusal::Syntax, SessionContextError::Shape { path, source }) => {
                assert_eq!(path, file.path());
                assert!(source.is_eof() || source.is_syntax());
            }
            (Refusal::Shape, SessionContextError::Shape { path, source }) => {
                assert_eq!(path, file.path());
                assert!(source.is_data());
            }
            (Refusal::Encoding, SessionContextError::Io { path, source }) => {
                assert_eq!(path, file.path());
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
            }
            (_, error) => panic!("wrong context refusal category: {error}"),
        }
    }
    let directory = workspace_root().join("corpus/reference/core");
    let error =
        SessionContextFile::read_json(&directory).expect_err("directory is not context JSON");
    assert!(matches!(error, SessionContextError::Io { path, .. } if path == directory));
}

#[test]
fn reference_sampling_bounds_windows_without_overflow_or_utf8_truncation() {
    use talkbank_transform::speaker_id::{SampleBudget, sample_session};
    let parser = TreeSitterParser::new().expect("parser");
    let file = reference(&parser, "core/basic-conversation.cha");
    let anchor = SpeakerCode::new("MOT");
    let expected = [
        ("MOT", ["what kind of cookies", "we can make some together"]),
        ("CHI", ["I want some cookies", "the chocolate ones"]),
    ];
    for (head, tail, indices) in [
        (0, 0, &[][..]),
        (1, 0, &[0][..]),
        (0, 1, &[1][..]),
        (1, 1, &[0, 1][..]),
        (2, 2, &[0, 1][..]),
        (usize::MAX, 1, &[0, 1][..]),
        (1, usize::MAX, &[0, 1][..]),
        (usize::MAX, usize::MAX, &[0, 1][..]),
    ] {
        let samples = sample_session(
            &file,
            &anchor,
            SampleBudget {
                head,
                tail,
                char_cap: 500,
            },
        );
        assert_eq!(samples.len(), expected.len());
        for (actual, (speaker, turns)) in samples.iter().zip(expected) {
            assert_eq!(
                actual.code.as_str(),
                speaker,
                "anchor precedes document-first speaker"
            );
            let text: Vec<_> = actual
                .utterances
                .iter()
                .map(|turn| turn.0.as_str())
                .collect();
            let selected: Vec<_> = indices.iter().map(|index| turns[*index]).collect();
            assert_eq!(text, selected, "bounded head/tail window {head}/{tail}");
        }
    }
    let chinese = reference(&parser, "audio/chinese-adult-conversation.cha");
    for (cap, expected_text) in [(0, ""), (1, "好"), (2, "好 "), (3, "好 请")] {
        let samples = sample_session(
            &chinese,
            &SpeakerCode::new("PAR0"),
            SampleBudget {
                head: 1,
                tail: 0,
                char_cap: cap,
            },
        );
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].utterances.len(), 1);
        assert_eq!(samples[0].utterances[0].0, expected_text);
        assert_eq!(samples[0].utterances[0].0.chars().count(), cap);
    }
}

#[test]
fn reference_speaker_decision_survives_override_persistence_and_replay() {
    use std::collections::BTreeMap;
    use talkbank_model::ParticipantRole;
    use talkbank_transform::speaker_id::{
        DecisionEngine, InsertedRoleSpec, MergeOverride, OverrideFile, OverrideMode, SpeakerAction,
    };
    let parser = TreeSitterParser::new().expect("parser");
    let chat = reference(&parser, "core/basic-conversation.cha");
    let report = identify_mapping(
        &chat,
        &SpeakerCode::new("CHI"),
        &chat,
        ConfidenceThreshold::DEFAULT,
    )
    .expect("identity replay lexical control, not an identity adjudication");
    // Explicit test assignment: no demographic inference from lexical scores.
    let mapping = parse_mapping_spec("CHI=drop,MOT=INV:Investigator").expect("mapping control");
    let role = InsertedRoleSpec::new(
        &SpeakerCode::new("INV"),
        &ParticipantRole::new("Investigator"),
    );
    let timestamp = "2000-01-01T00:00:00Z".parse().expect("authored timestamp");
    let entry = MergeOverride::auto_decision(
        &mapping,
        &report,
        BTreeMap::from([("MOT".to_owned(), role)]),
        "operator".to_owned(),
        timestamp,
    );
    assert_eq!(entry.mapping["CHI"], SpeakerAction::Drop);
    assert_eq!(entry.mapping["MOT"], SpeakerAction::Rename);
    assert_eq!(
        entry.to_mapping_spec().expect("typed replay admission"),
        mapping
    );
    let mut record = OverrideFile::default();
    assert!(record.get("reference-control").is_none());
    record.upsert("reference-control".into(), entry);
    // A single temporary file exercises the public persistence boundary;
    // it is not a second CHAT fixture or a persistent workflow database.
    let file = tempfile::NamedTempFile::new().expect("override destination");
    record.write(file.path()).expect("atomic override write");
    let restored = OverrideFile::read_or_default(file.path()).expect("override read");
    assert_eq!(
        restored.session_ids().collect::<Vec<_>>(),
        ["reference-control"]
    );
    assert_eq!(
        restored
            .auto_entries()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        ["reference-control"]
    );
    assert_eq!(restored.llm_entries().count(), 0);
    let entry = restored
        .get("reference-control")
        .expect("persisted decision");
    assert_eq!(entry.mode, OverrideMode::Auto);
    assert_eq!(entry.engine, DecisionEngine::Deterministic);
    assert_eq!(entry.operator, "operator");
    assert_eq!(entry.decided_at, timestamp);
    assert_eq!(entry.scores, report.scores_to_serializable());
    assert_eq!(entry.margin, Some(6.5));
    assert!(entry.note.is_none() && entry.flags.is_empty() && entry.judgment.is_none());
    let replay = entry
        .to_mapping_spec()
        .expect("persisted mapping admission");
    assert_eq!(replay, mapping);
    assert_eq!(
        apply_mapping_chat(&chat, &replay),
        apply_mapping_chat(&chat, &mapping)
    );

    use talkbank_transform::speaker_id::OverrideFileError;
    let supported = record.schema_version;
    record.schema_version = u32::MAX;
    record
        .write(file.path())
        .expect("write deliberately unsupported external version");
    let error = OverrideFile::read_or_default(file.path())
        .expect_err("unknown schema is not an empty record");
    assert!(
        matches!(error, OverrideFileError::UnsupportedSchemaVersion {
        found: Some(u32::MAX), supported: actual,
    } if actual == supported)
    );
    assert!(
        error
            .to_string()
            .contains("unsupported override-file schema_version")
    );

    std::fs::write(file.path(), b"[broken").expect("malformed external TOML");
    let error =
        OverrideFile::read_or_default(file.path()).expect_err("malformed file must not default");
    assert!(matches!(error, OverrideFileError::Toml(_)));
    assert!(error.to_string().starts_with("override-file TOML error:"));
    std::fs::write(file.path(), [0xff]).expect("non-UTF-8 external bytes");
    let error =
        OverrideFile::read_or_default(file.path()).expect_err("encoding failure is not absence");
    assert!(matches!(&error, OverrideFileError::Io(cause)
        if cause.kind() == std::io::ErrorKind::InvalidData));
    assert!(error.to_string().starts_with("override-file I/O error:"));

    // A file cannot serve as a directory: exercise write refusal without
    // permission changes or touching any existing user destination.
    let impossible_child = file.path().join("decision.toml");
    assert!(matches!(
        record.write(&impossible_child),
        Err(OverrideFileError::Io(_))
    ));
    let missing = file.path().to_path_buf();
    file.close().expect("remove test-owned temporary input");
    let absent = OverrideFile::read_or_default(&missing).expect("only absent input defaults");
    assert_eq!(absent.schema_version, supported);
    assert_eq!(absent.session_ids().count(), 0);
}

#[test]
fn reference_lexical_ranking_retains_support_across_confidence_and_wire_boundaries() {
    let parser = TreeSitterParser::new().expect("parser");
    let basic = reference(&parser, "core/basic-conversation.cha");
    let phonological = reference(&parser, "annotation/groups-phonological.cha");
    let anchor = SpeakerCode::new("CHI");
    let mother = SpeakerCode::new("MOT");
    let threshold = ConfidenceThreshold::DEFAULT;
    assert_eq!(ConfidenceThreshold::default(), threshold);
    assert_eq!(threshold.to_string(), "2.00x");
    for text in ["NaN", "inf", "-inf", "0", "-1", "0.999"] {
        let error = text
            .parse::<ConfidenceThreshold>()
            .expect_err("invalid operator threshold");
        assert!(matches!(error, ParseConfidenceThresholdError::Invalid(_)));
        assert!(error.to_string().contains("finite and at least 1.0"));
    }
    let error = "not-a-number"
        .parse::<ConfidenceThreshold>()
        .expect_err("malformed threshold");
    assert!(matches!(error, ParseConfidenceThresholdError::Float(_)));
    assert!(
        error
            .to_string()
            .starts_with("confidence threshold is not a number:")
    );
    let minimum = "1"
        .parse::<ConfidenceThreshold>()
        .expect("inclusive threshold boundary");
    assert_eq!(minimum.value(), 1.0);
    // Identity replay is a lexical control, not a claim that matching speech
    // by itself proves that independently supplied tracks have one identity.
    let matched = identify_mapping(&basic, &anchor, &basic, threshold)
        .expect("identical anchor bag has a decisive lexical margin");
    assert_eq!(matched.winner(), &anchor);
    assert_eq!(matched.lexical_evidence().len(), 2);
    for (speaker, reference_tokens, donor_tokens, shared_tokens, union_tokens, display) in [
        (&anchor, 6, 6, 6, 6, "1.0000"),
        (&mother, 6, 9, 2, 13, "0.1538"),
    ] {
        let evidence = matched.evidence_for(speaker).expect("ranked speaker");
        assert_eq!(evidence.reference_tokens(), reference_tokens);
        assert_eq!(evidence.donor_tokens(), donor_tokens);
        assert_eq!(evidence.shared_tokens(), shared_tokens);
        assert_eq!(evidence.union_tokens(), union_tokens);
        assert_eq!(
            evidence.score().value(),
            shared_tokens as f64 / union_tokens as f64
        );
        assert_eq!(evidence.score().to_string(), display);
    }
    let winner_score = matched
        .evidence_for(&anchor)
        .expect("anchor evidence")
        .score();
    let runner_up_score = matched
        .evidence_for(&mother)
        .expect("mother evidence")
        .score();
    assert_eq!(
        ConfidenceMargin::from_scores(runner_up_score, winner_score),
        matched.margin()
    );
    assert!(matched.margin().meets(minimum));
    assert_eq!(matched.margin_to_serializable(), Some(6.5));
    assert_eq!(matched.margin().to_string(), "6.50x");
    assert_eq!(matched.scores_to_serializable()["CHI"], 1.0);
    assert_eq!(matched.scores_to_serializable()["MOT"], 2.0 / 13.0);
    let accepted = serde_json::to_value(RecordedSpeakerIdentificationAttempt::accepted(
        &matched, threshold,
    ))
    .expect("accepted attempt wire");
    assert_eq!(accepted["outcome"], "accepted");
    assert_eq!(
        accepted["match_report"]["speakers"]["MOT"]["shared_tokens"],
        2
    );

    // Comma, tag-question and vocative separators have no lexical weight.
    // The child adds "well think so" to the mother's otherwise identical bag;
    // single-letter I/a and the apostrophe-bearing wasn't are ineligible.
    let separators = reference(&parser, "content/separators.cha");
    let mut separator_count = 0;
    for utterance in separators.utterances() {
        talkbank_model::alignment::helpers::walk_words(
            &utterance.main.content.content,
            None,
            &mut |item| {
                if matches!(
                    item,
                    talkbank_model::alignment::helpers::WordItem::Separator(_)
                ) {
                    separator_count += 1;
                }
            },
        );
    }
    assert_eq!(
        separator_count, 3,
        "all three separator controls must be present"
    );
    let separated = identify_mapping(&separators, &anchor, &separators, minimum)
        .expect("identity control with separators");
    assert_eq!(separated.winner(), &anchor);
    for (speaker, donor_tokens, shared_tokens) in [(&anchor, 11, 11), (&mother, 8, 8)] {
        let evidence = separated.evidence_for(speaker).expect("separator speaker");
        assert_eq!(evidence.reference_tokens(), 11);
        assert_eq!(evidence.donor_tokens(), donor_tokens);
        assert_eq!(evidence.shared_tokens(), shared_tokens);
        assert_eq!(evidence.union_tokens(), 11);
    }
    let SpeakerIdError::LowConfidence { report, .. } =
        identify_mapping(&separators, &anchor, &separators, threshold)
            .expect_err("shared vocabulary is not decisive at the default threshold")
    else {
        panic!("expected separator-control confidence refusal");
    };
    assert_eq!(report.record(), separated.record());

    // Same observation, stricter operator policy: evidence survives refusal.
    let strict = "7"
        .parse::<ConfidenceThreshold>()
        .expect("finite threshold");
    let SpeakerIdError::LowConfidence {
        report,
        threshold: refused_at,
    } = identify_mapping(&basic, &anchor, &basic, strict).expect_err("stricter policy refuses")
    else {
        panic!("expected confidence refusal with evidence");
    };
    assert_eq!(refused_at, strict);
    assert_eq!(report.record(), matched.record());
    let refused = serde_json::to_value(RecordedSpeakerIdentificationAttempt::low_confidence(
        &report, refused_at,
    ))
    .expect("refusal wire");
    assert_eq!(refused["outcome"], "low_confidence");
    assert_eq!(refused["match_report"], accepted["match_report"]);

    // Nested phonological groups contribute non/il/pas. A/B/C are excluded
    // single-character controls, so the runner-up has no qualifying tokens.
    let unbounded = identify_mapping(&phonological, &anchor, &phonological, strict)
        .expect("positive winner against empty runner-up");
    assert!(matches!(unbounded.margin(), ConfidenceMargin::Unbounded));
    assert_eq!(unbounded.margin().to_string(), "∞x");
    assert_eq!(unbounded.margin_to_serializable(), Some(f64::INFINITY));
    let wire = serde_json::to_value(unbounded.record()).expect("nonfinite-free typed record");
    assert_eq!(wire["margin"]["kind"], "unbounded");
    assert_eq!(wire["speakers"]["CHI"]["shared_tokens"], 3);
    assert_eq!(wire["speakers"]["MOT"]["donor_tokens"], 0);

    // Cross-document disjoint vocabulary is not an accepted zero-score winner.
    let SpeakerIdError::LowConfidence {
        report,
        threshold: refused_at,
    } = identify_mapping(&basic, &anchor, &phonological, threshold)
        .expect_err("no lexical overlap")
    else {
        panic!("expected no-information refusal");
    };
    assert!(matches!(report.margin(), ConfidenceMargin::NoInformation));
    assert_eq!(report.margin_to_serializable(), None);
    assert_eq!(report.margin().to_string(), "no information");
    assert_eq!(
        report.winner(),
        &anchor,
        "zero-score ties retain document order"
    );
    assert_eq!(
        report
            .evidence_for(&anchor)
            .expect("anchor donor")
            .donor_tokens(),
        3
    );
    assert_eq!(
        report
            .evidence_for(&mother)
            .expect("mother donor")
            .donor_tokens(),
        0
    );
    let wire = serde_json::to_value(RecordedSpeakerIdentificationAttempt::low_confidence(
        &report, refused_at,
    ))
    .expect("no-information wire");
    assert_eq!(wire["match_report"]["margin"]["kind"], "no_information");
    assert_eq!(wire["match_report"]["speakers"]["CHI"]["shared_tokens"], 0);
    assert_eq!(wire["match_report"]["speakers"]["MOT"]["shared_tokens"], 0);

    // The current lexical matcher admits ASCII alphabetic tokens only. A
    // non-Latin transcript must yield no information, even against itself;
    // document order is not identity evidence when every bag is empty.
    let chinese = reference(&parser, "languages/zho-conversation.cha");
    let SpeakerIdError::LowConfidence {
        report,
        threshold: refused_at,
    } = identify_mapping(&chinese, &anchor, &chinese, threshold)
        .expect_err("non-Latin-only speech has no admitted lexical evidence")
    else {
        panic!("expected no-information refusal");
    };
    assert!(matches!(report.margin(), ConfidenceMargin::NoInformation));
    assert_eq!(report.lexical_evidence().len(), 2);
    for (_, evidence) in report.lexical_evidence() {
        assert_eq!(evidence.reference_tokens(), 0);
        assert_eq!(evidence.donor_tokens(), 0);
        assert_eq!(evidence.shared_tokens(), 0);
        assert_eq!(evidence.union_tokens(), 0);
    }
    let wire = serde_json::to_value(RecordedSpeakerIdentificationAttempt::low_confidence(
        &report, refused_at,
    ))
    .expect("non-Latin refusal wire");
    assert_eq!(wire["outcome"], "low_confidence");
    assert_eq!(wire["match_report"]["margin"]["kind"], "no_information");

    for removed in ["CHI", "MOT"] {
        let mapping = parse_mapping_spec(&format!("{removed}=drop")).expect("operator removal");
        let reduced = strict_parse(parser.parse_chat_file(&apply_mapping_chat(&basic, &mapping)))
            .expect("transformed reference syntax");
        let error = identify_mapping(&basic, &anchor, &reduced, threshold)
            .expect_err("one donor track cannot be discriminated");
        let SpeakerIdError::DonorTooFewSpeakers { speakers } = error else {
            panic!("expected donor population refusal");
        };
        assert_eq!(speakers.len(), 1);
        assert_ne!(speakers[0].as_str(), removed);
        let wire = serde_json::to_value(
            RecordedSpeakerIdentificationAttempt::donor_too_few_speakers(&speakers),
        )
        .expect("donor refusal wire");
        assert_eq!(wire["outcome"], "donor_too_few_speakers");
        assert_eq!(wire["speakers"][0], speakers[0].as_str());
        if removed == "CHI" {
            let SpeakerIdError::ReferenceMissingAnchor { anchor: missing } =
                identify_mapping(&reduced, &anchor, &basic, threshold)
                    .expect_err("missing reference anchor")
            else {
                panic!("expected anchor refusal");
            };
            assert_eq!(missing, anchor);
            let wire = serde_json::to_value(
                RecordedSpeakerIdentificationAttempt::reference_missing_anchor(&missing),
            )
            .expect("anchor refusal wire");
            assert_eq!(wire["outcome"], "reference_missing_anchor");
            assert_eq!(wire["anchor"], "CHI");
        }
    }
}
