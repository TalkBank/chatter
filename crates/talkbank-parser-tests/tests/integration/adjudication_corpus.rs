//! Operator-input failures must preserve corpus-backed pending work.

use std::collections::BTreeMap;
use talkbank_model::SemanticEq;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{
    chat_corpus::ChatCorpus, repo_paths::workspace_root, test_error::strict_parse,
};
use talkbank_transform::adjudication::{
    AdjudicationError, OperatorDecision, PendingAdjudications, PendingEntry, PendingKindData,
    ScriptedPrompter, run_adjudication,
};
use talkbank_transform::speaker_id::{
    DecisionEngine, InsertedRoleSpec, OverrideFile, SpeakerAction,
};

fn choose_mother_role() -> OperatorDecision {
    OperatorDecision::ChooseRole {
        adult_roles: BTreeMap::from([(
            "MOT".to_owned(),
            InsertedRoleSpec {
                code: "MOT".to_owned(),
                tag: "Mother".to_owned(),
                specific_role: None,
            },
        )]),
        note: Some("Authored operator-input control".to_owned()),
    }
}

#[test]
fn reference_scripted_choice_shapes_require_admission_before_replay() {
    use talkbank_transform::adjudication::{AdjudicationKind, SuggestedSpeakerIdMapping};
    use talkbank_transform::speaker_id::apply_mapping_chat;
    let parser = TreeSitterParser::new().expect("parser");
    let text = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/basic-conversation.cha"),
    )
    .expect("reference");
    let source = strict_parse(parser.parse_chat_file(&text)).expect("reference syntax");
    let suggested = SuggestedSpeakerIdMapping {
        mapping: BTreeMap::from([("MOT".to_owned(), SpeakerAction::Rename)]),
        adult_roles: BTreeMap::from([(
            "MOT".to_owned(),
            InsertedRoleSpec {
                code: "MOT".to_owned(),
                tag: "Mother".to_owned(),
                specific_role: None,
            },
        )]),
    };
    struct Case {
        kind: AdjudicationKind,
        choice: &'static str,
        expected_code: &'static str,
        expected_role: &'static str,
    }
    for case in [
        Case {
            kind: AdjudicationKind::SpeakerIdLowConfidence,
            choice: "{ kind = 'accept-suggested', note = 'authored acceptance' }",
            expected_code: "MOT",
            expected_role: "Mother",
        },
        Case {
            kind: AdjudicationKind::SpeakerIdLowConfidence,
            choice: "{ kind = 'override-mapping', mapping = { MOT = 'rename' }, adult_roles = { MOT = { code = 'INV', tag = 'Investigator' } }, note = 'authored override' }",
            expected_code: "INV",
            expected_role: "Investigator",
        },
        Case {
            kind: AdjudicationKind::SanityScanMisclassification,
            choice: "{ kind = 'override-mapping', mapping = { MOT = 'rename' }, adult_roles = { MOT = { code = 'INV', tag = 'Investigator' } }, note = 'authored override' }",
            expected_code: "INV",
            expected_role: "Investigator",
        },
        Case {
            kind: AdjudicationKind::ParentRoleLookup,
            choice: "{ kind = 'choose-role', adult_roles = { MOT = { code = 'MOT', tag = 'Mother' } }, note = 'authored role choice' }",
            expected_code: "MOT",
            expected_role: "Mother",
        },
    ] {
        // Authored operator/wire controls use real source participants; they
        // make no identity or confidence finding about the recording.
        let data = match case.kind {
            AdjudicationKind::SpeakerIdLowConfidence => PendingKindData::SpeakerIdLowConfidence {
                suggested: suggested.clone(),
            },
            AdjudicationKind::SanityScanMisclassification => {
                PendingKindData::SanityScanMisclassification {
                    suggested: suggested.clone(),
                    reason: "authored review control".into(),
                }
            }
            AdjudicationKind::ParentRoleLookup => PendingKindData::ParentRoleLookup {
                donor_speaker: "MOT".into(),
                speaker_mapping: suggested.mapping.clone(),
            },
        };
        let mut pending = PendingAdjudications {
            entries: vec![PendingEntry {
                session_id: "reference-control".into(),
                created_at: "2026-01-01T00:00:00Z".parse().expect("fixed timestamp"),
                data,
                scores: BTreeMap::new(),
                margin: None,
                threshold_used: None,
                engine: DecisionEngine::Deterministic,
                judgment: None,
            }],
            ..PendingAdjudications::default()
        };
        assert_eq!(pending.entries[0].kind(), case.kind);
        let kind = serde_json::to_string(&case.kind).expect("wire discriminator");
        let script = tempfile::NamedTempFile::new().expect("decision file");
        std::fs::write(script.path(), format!(
            "schema_version = 2\n[[decisions]]\nsession_id = 'reference-control'\nkind = {kind}\nchoice = {}\n", case.choice
        )).expect("authored decision TOML");
        let mut prompter = ScriptedPrompter::read_toml(script.path()).expect("script admission");
        let mut overrides = OverrideFile::default();
        let outcome = run_adjudication(
            &mut pending,
            &mut overrides,
            &mut prompter,
            "operator".into(),
        )
        .expect("compatible explicit choice");
        assert_eq!(outcome.resolved_count(), 1);
        assert!(pending.entries.is_empty());
        let accepted = overrides
            .get("reference-control")
            .expect("admitted decision");
        assert_eq!(accepted.adult_roles["MOT"].code, case.expected_code);
        assert_eq!(accepted.adult_roles["MOT"].tag, case.expected_role);
        let mapping = accepted.to_mapping_spec().expect("typed replay admission");
        let output = apply_mapping_chat(&source, &mapping);
        let replay = strict_parse(parser.parse_chat_file(&output)).expect("replayed CHAT");
        assert_eq!(replay.utterances().count(), source.utterances().count());
        for (actual, original) in replay.utterances().zip(source.utterances()) {
            let expected_code = if original.main.speaker.as_str() == "MOT" {
                case.expected_code
            } else {
                original.main.speaker.as_str()
            };
            assert_eq!(actual.main.speaker.as_str(), expected_code);
            assert!(actual.main.content.semantic_eq(&original.main.content));
        }
    }
}

/// The scan is a review suggestion, never authority to relabel a speaker.
/// These authored operator controls use unchanged reference CHAT; they do not
/// assert that a child's longer turn is evidence of an actual identity swap.
#[test]
fn reference_sanity_scan_requires_operator_acceptance_before_mapping_replay() {
    use talkbank_model::SpeakerCode;
    use talkbank_transform::adjudication::SuggestedSpeakerIdMapping;
    use talkbank_transform::sanity_scan::{SanityScanThreshold, scan_session};
    use talkbank_transform::speaker_id::{MergeOverride, apply_mapping_chat};

    enum ReviewExpectation {
        NoSuggestion,
        SuggestSwap {
            anchor_mean: f64,
            inserted_mean: f64,
        },
    }
    let parser = TreeSitterParser::new().expect("parser");
    let role = InsertedRoleSpec {
        code: "MOT".to_owned(),
        tag: "Mother".to_owned(),
        specific_role: None,
    };
    let original = MergeOverride::operator_decision(
        BTreeMap::from([
            ("CHI".to_owned(), SpeakerAction::Drop),
            ("MOT".to_owned(), SpeakerAction::Rename),
        ]),
        BTreeMap::from([("MOT".to_owned(), role.clone())]),
        BTreeMap::new(),
        None,
        "operator".to_owned(),
        "2026-01-01T00:00:00Z".parse().expect("fixed timestamp"),
        None,
    );
    for (fixture, expectation) in [
        (
            "core/basic-conversation.cha",
            ReviewExpectation::NoSuggestion,
        ),
        (
            "annotation/groups-regular.cha",
            ReviewExpectation::NoSuggestion,
        ),
        (
            "annotation/groups-phonological.cha",
            ReviewExpectation::SuggestSwap {
                anchor_mean: 5.0,
                inserted_mean: 3.0,
            },
        ),
    ] {
        let source =
            std::fs::read_to_string(workspace_root().join("corpus/reference").join(fixture))
                .expect("canonical reference");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("reference syntax");
        let original_wire = serde_json::to_value(&original).expect("original decision");
        let result = scan_session(
            &file,
            &original,
            &SpeakerCode::new("CHI"),
            SanityScanThreshold::DEFAULT,
        );
        assert_eq!(
            serde_json::to_value(&original).expect("unchanged decision"),
            original_wire
        );
        match expectation {
            ReviewExpectation::NoSuggestion => assert!(result.is_none(), "{fixture}"),
            ReviewExpectation::SuggestSwap {
                anchor_mean,
                inserted_mean,
            } => {
                let flag = result.expect("nested word counts trigger advisory review");
                assert_eq!(flag.anchor_mean_words, anchor_mean);
                assert_eq!(flag.inserted_mean_words, inserted_mean);
                assert_eq!(
                    flag.suggested_mapping,
                    BTreeMap::from([
                        ("CHI".to_owned(), SpeakerAction::Rename),
                        ("MOT".to_owned(), SpeakerAction::Drop),
                    ])
                );
                assert_eq!(flag.suggested_adult_roles.len(), 1);
                assert_eq!(flag.suggested_adult_roles["CHI"], role);
                assert!(flag.reason.contains("5.00"));
                assert!(flag.reason.contains("3.00"));
                let mut pending = PendingAdjudications {
                    entries: vec![PendingEntry {
                        session_id: fixture.to_owned(),
                        created_at: original.decided_at,
                        data: PendingKindData::SanityScanMisclassification {
                            suggested: SuggestedSpeakerIdMapping {
                                mapping: flag.suggested_mapping,
                                adult_roles: flag.suggested_adult_roles,
                            },
                            reason: flag.reason,
                        },
                        scores: BTreeMap::new(),
                        margin: None,
                        threshold_used: None,
                        engine: DecisionEngine::Deterministic,
                        judgment: None,
                    }],
                    ..PendingAdjudications::default()
                };
                // Persisting a pending suggestion still grants no mapping.
                let absent = tempfile::NamedTempFile::new().expect("reserved first-run path");
                let absent_path = absent.path().to_path_buf();
                absent
                    .close()
                    .expect("remove only this test's empty reservation");
                let first_run = PendingAdjudications::read_or_default(&absent_path)
                    .expect("only a missing path admits the initial empty queue");
                assert_eq!(
                    first_run.schema_version,
                    PendingAdjudications::CURRENT_SCHEMA_VERSION
                );
                assert!(first_run.entries.is_empty());
                assert!(
                    matches!(PendingAdjudications::read(&absent_path),
                    Err(AdjudicationError::FileIo { path, source })
                    if path == absent_path && source.kind() == std::io::ErrorKind::NotFound),
                    "strict read must not silently initialize a missing queue"
                );
                let pending_file = tempfile::NamedTempFile::new().expect("pending file");
                let expected = serde_json::to_value(&pending).expect("pending fields");
                let mut unsupported = pending.clone();
                unsupported.schema_version = 0;
                unsupported
                    .write(pending_file.path())
                    .expect("external unsupported-version control");
                for result in [
                    PendingAdjudications::read(pending_file.path()),
                    PendingAdjudications::read_or_default(pending_file.path()),
                ] {
                    assert!(
                        matches!(
                            result,
                            Err(AdjudicationError::UnsupportedSchemaVersion {
                                found: Some(0),
                                supported: PendingAdjudications::CURRENT_SCHEMA_VERSION,
                            })
                        ),
                        "existing unsupported data must not become an empty queue: {result:?}"
                    );
                }
                std::fs::write(pending_file.path(), "schema_version = [")
                    .expect("truncated external queue control");
                for result in [
                    PendingAdjudications::read(pending_file.path()),
                    PendingAdjudications::read_or_default(pending_file.path()),
                ] {
                    assert!(
                        matches!(result, Err(AdjudicationError::Toml(_))),
                        "malformed existing data must not default away: {result:?}"
                    );
                }
                std::fs::write(pending_file.path(), [0xff]).expect("invalid UTF-8 control");
                for result in [
                    PendingAdjudications::read(pending_file.path()),
                    PendingAdjudications::read_or_default(pending_file.path()),
                ] {
                    assert!(
                        matches!(result, Err(AdjudicationError::FileIo { path, source })
                        if path == pending_file.path() && source.kind() == std::io::ErrorKind::InvalidData)
                    );
                }
                assert_eq!(
                    serde_json::to_value(&pending).expect("uncommitted queue"),
                    expected
                );
                pending
                    .write(pending_file.path())
                    .expect("write pending TOML");
                let saved = std::fs::read(pending_file.path()).expect("saved queue bytes");
                // A regular file cannot serve as the parent directory. This
                // portable failing destination is entirely owned by the test;
                // no permission changes or unrelated filesystem paths are used.
                let invalid_destination = pending_file.path().join("queue.toml");
                assert!(
                    matches!(pending.write(&invalid_destination),
                    Err(AdjudicationError::FileIo { path, .. })
                    if path == invalid_destination.with_extension("toml.tmp")),
                    "staging failure must identify the actual failed write path"
                );
                assert_eq!(
                    std::fs::read(pending_file.path()).expect("saved queue survives"),
                    saved
                );
                assert_eq!(
                    serde_json::to_value(&pending).expect("pending request survives"),
                    expected
                );
                pending =
                    PendingAdjudications::read(pending_file.path()).expect("read pending TOML");
                assert_eq!(
                    serde_json::to_value(&pending).expect("restored fields"),
                    expected
                );
                let existing = PendingAdjudications::read_or_default(pending_file.path())
                    .expect("existing queue cannot default away");
                assert_eq!(
                    serde_json::to_value(existing).expect("existing fields"),
                    expected
                );
                assert_eq!(
                    pending.entries[0].kind(),
                    talkbank_transform::adjudication::AdjudicationKind::SanityScanMisclassification
                );
                let mut overrides = OverrideFile::default();
                assert!(overrides.entries.is_empty());
                let decisions_file = tempfile::NamedTempFile::new().expect("decision file");
                assert!(
                    matches!(
                        ScriptedPrompter::read_toml(decisions_file.path()),
                        Err(AdjudicationError::Toml(_))
                    ),
                    "empty script cannot invent decisions"
                );
                std::fs::write(decisions_file.path(), [0xff])
                    .expect("invalid UTF-8 script control");
                assert!(matches!(ScriptedPrompter::read_toml(decisions_file.path()),
                    Err(AdjudicationError::FileIo { path, source })
                    if path == decisions_file.path() && source.kind() == std::io::ErrorKind::InvalidData));
                std::fs::write(decisions_file.path(), format!(
                    "schema_version = 2\n[[decisions]]\nsession_id = {fixture:?}\nkind = \"sanity-scan-misclassification\"\nchoice = {{ kind = \"accept-suggested\", note = \"Authored acceptance control, not an identity inference\" }}\n"
                )).expect("authored operator wire input");
                let mut prompter = ScriptedPrompter::read_toml(decisions_file.path())
                    .expect("read explicit operator acceptance");
                run_adjudication(
                    &mut pending,
                    &mut overrides,
                    &mut prompter,
                    "operator".to_owned(),
                )
                .expect("explicit acceptance");
                assert!(pending.entries.is_empty());
                let accepted = overrides.get(fixture).expect("accepted decision");
                let mapping = accepted.to_mapping_spec().expect("admitted replay mapping");
                let replay =
                    strict_parse(parser.parse_chat_file(&apply_mapping_chat(&file, &mapping)))
                        .expect("replayed CHAT syntax");
                let retained: Vec<_> = file
                    .utterances()
                    .filter(|turn| turn.main.speaker.as_str() == "CHI")
                    .collect();
                assert_eq!(replay.utterances().count(), retained.len());
                for (actual, expected) in replay.utterances().zip(retained) {
                    assert_eq!(actual.main.speaker.as_str(), "MOT");
                    assert!(actual.main.content.semantic_eq(&expected.main.content));
                }
                assert!(
                    scan_session(
                        &file,
                        &original,
                        &SpeakerCode::new("CHI"),
                        SanityScanThreshold(2.0)
                    )
                    .is_none()
                );
            }
        }
        // Incomplete or unsupported wire decisions must not manufacture
        // evidence or a role. All identifiers come from the reference headers.
        let mut incomplete = original.clone();
        incomplete.adult_roles.clear();
        assert!(
            scan_session(
                &file,
                &incomplete,
                &SpeakerCode::new("CHI"),
                SanityScanThreshold::DEFAULT
            )
            .is_none()
        );
        for action in [SpeakerAction::Drop, SpeakerAction::Rename] {
            let mut unsupported = original.clone();
            unsupported
                .mapping
                .values_mut()
                .for_each(|value| *value = action);
            assert!(
                scan_session(
                    &file,
                    &unsupported,
                    &SpeakerCode::new("CHI"),
                    SanityScanThreshold::DEFAULT
                )
                .is_none()
            );
        }
        let mut no_pair = original.clone();
        no_pair.mapping.remove("CHI");
        assert!(
            scan_session(
                &file,
                &no_pair,
                &SpeakerCode::new("CHI"),
                SanityScanThreshold::DEFAULT
            )
            .is_none()
        );

        // A successful speaker-removal transform leaves a parseable document,
        // but not the two observed populations required by this heuristic.
        // Exercise each missing side through the real wire/transform/parser
        // path rather than constructing an empty or fictitious AST speaker.
        for removed in ["CHI", "MOT"] {
            let mapping =
                talkbank_transform::speaker_id::parse_mapping_spec(&format!("{removed}=drop"))
                    .expect("explicit speaker-removal control");
            let remaining =
                strict_parse(parser.parse_chat_file(&apply_mapping_chat(&file, &mapping)))
                    .expect("speaker removal preserves CHAT syntax");
            let retained: Vec<_> = file
                .utterances()
                .filter(|turn| turn.main.speaker.as_str() != removed)
                .collect();
            assert!(!retained.is_empty(), "control retains the other population");
            assert_eq!(remaining.utterances().count(), retained.len());
            for (actual, expected) in remaining.utterances().zip(retained) {
                assert!(
                    actual.semantic_eq(expected),
                    "removal preserves retained turns and tiers"
                );
            }
            assert!(
                scan_session(
                    &remaining,
                    &original,
                    &SpeakerCode::new("CHI"),
                    SanityScanThreshold::DEFAULT,
                )
                .is_none(),
                "{fixture}: absent {removed} cannot justify a swap"
            );
        }
    }
}

#[test]
fn reference_adjudication_refusals_preserve_uncommitted_queue_and_resume() {
    enum Refusal {
        Prompt,
        Kind,
        Mapping,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("canonical corpus");
    let mut entries = Vec::new();
    let mut documents = Vec::new();
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference syntax");
        if !file
            .utterances()
            .any(|utterance| utterance.main.speaker.as_str() == "MOT")
        {
            continue;
        }
        // The corpus supplies actual speaker identities. The operator choice
        // below is an authored workflow input, not a new inference or ruling.
        entries.push(PendingEntry {
            session_id: fixture
                .path()
                .strip_prefix(workspace_root())
                .expect("repository fixture")
                .to_str()
                .expect("fixture identity")
                .to_owned(),
            created_at: "2026-01-01T00:00:00Z"
                .parse()
                .expect("fixed audit timestamp"),
            data: PendingKindData::ParentRoleLookup {
                donor_speaker: "MOT".to_owned(),
                speaker_mapping: BTreeMap::from([("MOT".to_owned(), SpeakerAction::Rename)]),
            },
            scores: BTreeMap::new(),
            margin: None,
            threshold_used: None,
            engine: DecisionEngine::Deterministic,
            judgment: None,
        });
        documents.push((fixture.source(), file));
        if entries.len() == 3 {
            break;
        }
    }
    assert_eq!(entries.len(), 3, "three distinct source-backed requests");
    let first = || (entries[0].session_id.clone(), choose_mother_role());
    for (decisions, committed, refusal) in [
        (vec![], 0, Refusal::Prompt),
        (vec![first()], 1, Refusal::Prompt),
        (
            vec![
                first(),
                (entries[2].session_id.clone(), choose_mother_role()),
            ],
            1,
            Refusal::Prompt,
        ),
        (
            vec![
                first(),
                (
                    entries[1].session_id.clone(),
                    OperatorDecision::AcceptSuggested { note: None },
                ),
            ],
            1,
            Refusal::Kind,
        ),
        (
            vec![
                first(),
                (
                    entries[1].session_id.clone(),
                    OperatorDecision::ChooseRole {
                        adult_roles: BTreeMap::new(),
                        note: None,
                    },
                ),
            ],
            1,
            Refusal::Mapping,
        ),
    ] {
        let mut pending = PendingAdjudications {
            entries: entries.clone(),
            ..PendingAdjudications::default()
        };
        let mut overrides = OverrideFile::default();
        let mut prompter = ScriptedPrompter::from_decisions(decisions);
        let error = run_adjudication(
            &mut pending,
            &mut overrides,
            &mut prompter,
            "operator".into(),
        )
        .expect_err("scripted refusal");
        assert!(
            match (refusal, &error) {
                (Refusal::Prompt, AdjudicationError::PrompterFailed { .. })
                | (Refusal::Kind, AdjudicationError::DecisionKindMismatch { .. }) => true,
                (
                    Refusal::Mapping,
                    AdjudicationError::InvalidDecisionMapping {
                        source:
                            talkbank_transform::speaker_id::SpeakerIdError::OverrideRenameMissingRole {
                                speaker,
                            },
                        ..
                    },
                ) => speaker.as_str() == "MOT",
                _ => false,
            },
            "specific refusal required: {error}"
        );
        assert_eq!(
            serde_json::to_value(&pending.entries).expect("pending wire"),
            serde_json::to_value(&entries[committed..]).expect("remaining wire"),
            "failed and later entries must survive intact and in order"
        );
        assert_eq!(overrides.entries.len(), committed);
        let accepted = serde_json::to_value(&overrides).expect("accepted prefix wire");
        let mut retry = ScriptedPrompter::from_decisions(
            pending
                .entries
                .iter()
                .map(|entry| (entry.session_id.clone(), choose_mother_role()))
                .collect(),
        );
        let outcome = run_adjudication(&mut pending, &mut overrides, &mut retry, "operator".into())
            .expect("resume remaining decisions");
        assert_eq!(outcome.resolved_count(), 3 - committed);
        assert!(pending.entries.is_empty());
        assert_eq!(overrides.entries.len(), 3);
        if committed != 0 {
            let resumed = serde_json::to_value(&overrides).expect("resumed wire");
            assert_eq!(
                accepted[&entries[0].session_id],
                resumed[&entries[0].session_id]
            );
        }
        for (entry, (source, file)) in entries.iter().zip(&documents) {
            let resolved = overrides.get(&entry.session_id).expect("resolved session");
            assert_eq!(resolved.adult_roles["MOT"].tag, "Mother");
            assert_eq!(resolved.mapping["MOT"], SpeakerAction::Rename);
            let mapping = resolved.to_mapping_spec().expect("admit recorded mapping");
            // Exercise the operator wire route independently of override-file
            // conversion, then send both admitted mappings to the same consumer.
            let role = &resolved.adult_roles["MOT"];
            let command = format!("MOT={}:{}", role.code, role.tag);
            for input in [
                command.clone(),
                format!(" MOT = {} : {} ", role.code, role.tag),
            ] {
                let parsed = talkbank_transform::speaker_id::parse_mapping_spec(&input)
                    .expect("authored identity mapping command");
                assert_eq!(parsed, mapping, "wire and record admission must agree");
                assert_eq!(
                    talkbank_transform::speaker_id::apply_mapping_chat(file, &parsed),
                    *source,
                    "operator mapping must preserve the original CHAT bytes"
                );
            }
            for malformed in [
                String::new(),
                format!("{command},"),
                format!("{command},,{command}"),
                format!("MOT{}:{}", role.code, role.tag),
                format!("MOT={}", role.code),
                format!("{command},{command}"),
                format!("{command}, MOT=drop"),
            ] {
                assert!(
                    matches!(
                        talkbank_transform::speaker_id::parse_mapping_spec(&malformed),
                        Err(talkbank_transform::speaker_id::SpeakerIdError::InvalidMappingSpec(_))
                    ),
                    "mapping syntax must be refused: {malformed:?}"
                );
            }
            let drop_mapping = talkbank_transform::speaker_id::parse_mapping_spec(" MOT = drop ")
                .expect("authored drop command");
            let dropped_source =
                talkbank_transform::speaker_id::apply_mapping_chat(file, &drop_mapping);
            let dropped = strict_parse(parser.parse_chat_file(&dropped_source))
                .expect("speaker removal retains parseable CHAT");
            let remaining: Vec<_> = file
                .utterances()
                .filter(|utterance| utterance.main.speaker.as_str() != "MOT")
                .collect();
            assert_eq!(dropped.utterances().count(), remaining.len());
            for (actual, expected) in dropped.utterances().zip(remaining) {
                assert!(
                    actual.semantic_eq(expected),
                    "drop preserves other speakers' turns"
                );
            }
            assert_eq!(
                talkbank_transform::speaker_id::apply_mapping_chat(file, &mapping),
                *source,
                "replaying the confirmed identity mapping preserves the source"
            );
        }
    }
}
