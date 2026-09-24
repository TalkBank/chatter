//! Rediarization policy over source-backed CHAT and admitted external timelines.
#![allow(clippy::expect_used)]

use std::collections::HashSet;
use talkbank_model::model::{Header, SemanticEq};
use talkbank_model::{ErrorCode, ParseValidateOptions, SpeakerCode, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{
    chat_corpus::ChatCorpus, repo_paths::workspace_root, test_error::strict_parse,
};
use talkbank_transform::rediarize::{
    ContestedThreshold, DiarizationTimeline, DiarizationTurn, FlagReason, RediarizeSummary,
    TimeSpanMs, rediarize, rediarize_content,
};

/// Absence of diarization and a single externally supplied track are different
/// claims. The latter's intervals are copied from actual source bullets.
enum TimelineCase {
    Absent,
    OneTrack(SpeakerCode),
}

#[test]
fn birth_header_spec_survives_reattribution_as_reported_error_not_silent_loss() {
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let parser = TreeSitterParser::new().expect("parser");
    let text = std::fs::read_to_string(root.join("E524_3.cha")).expect("timed birth control");
    let source = strict_parse(parser.parse_chat_file(&text)).expect("legal control parses");
    let utterance = source.utterances().next().expect("timed control utterance");
    let bullet = utterance.main.content.bullet.as_ref().expect("spec timing");
    let span =
        TimeSpanMs::new(bullet.timing.start_ms, bullet.timing.end_ms).expect("spec interval");
    let original = DiarizationTimeline::new(vec![DiarizationTurn {
        track: utterance.main.speaker.clone(),
        span,
    }]);
    let (unchanged, outcome) = rediarize_content(
        &text,
        &original,
        ParseValidateOptions::default().with_validation(),
        None,
    )
    .expect("identity attribution keeps valid birth reference");
    assert_eq!(outcome.reassigned, 0);
    let parsed = strict_parse(parser.parse_chat_file(&unchanged)).expect("identity output parses");
    assert!(source.semantic_eq(&parsed));

    let renamed = DiarizationTimeline::new(vec![DiarizationTurn {
        track: SpeakerCode::new("TRACK"),
        span,
    }]);
    let errors = talkbank_model::ErrorCollector::new();
    let (output, outcome) = rediarize(&source, &renamed, None, &errors);
    assert_eq!(outcome.reassigned, 1);
    let diagnostics = errors.into_vec();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, ErrorCode::BirthUnknownParticipant);
    let births = |file: &talkbank_model::model::ChatFile| {
        file.headers()
            .filter(|h| matches!(h, Header::Birth { .. }))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert!(
        births(&source).semantic_eq(&births(&output)),
        "birth metadata must not be dropped or retargeted"
    );
    match rediarize_content(
        &text,
        &renamed,
        ParseValidateOptions::default().with_validation(),
        None,
    ) {
        Err(talkbank_transform::PipelineError::Validation(actual)) => {
            assert_eq!(actual, diagnostics)
        }
        other => panic!("orphaned birth header must refuse serialized output: {other:?}"),
    }
    let invalid =
        std::fs::read_to_string(root.join("E524_4.cha")).expect("unknown participant mutation");
    match rediarize_content(
        &invalid,
        &original,
        ParseValidateOptions::default().with_validation(),
        None,
    ) {
        Err(talkbank_transform::PipelineError::Parse(errors)) => assert!(
            errors
                .errors
                .iter()
                .any(|e| e.code == ErrorCode::BirthUnknownParticipant)
        ),
        other => panic!("authored invalid birth reference must refuse input admission: {other:?}"),
    }
}

#[test]
fn reference_contested_ownership_unions_duplicates_and_preserves_cross_track_time() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let winner = SpeakerCode::new("TRACKA");
    let rival = SpeakerCode::new("TRACKB");
    let mut contested_witnesses = 0;
    for fixture in corpus.fixtures() {
        let source =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        let mut turns = Vec::new();
        for bullet in source
            .utterances()
            .filter_map(|u| u.main.content.bullet.as_ref())
        {
            let span = TimeSpanMs::new(bullet.timing.start_ms, bullet.timing.end_ms)
                .expect("reference interval");
            // Same-track duplication is redundant evidence, not extra held
            // time. Both distinct tracks retain the original full interval.
            for track in [&rival, &winner, &winner] {
                turns.push(DiarizationTurn {
                    track: track.clone(),
                    span,
                });
            }
        }
        let reversed = DiarizationTimeline::new(turns.iter().rev().cloned().collect());
        let timeline = DiarizationTimeline::new(turns);
        let errors = talkbank_model::ErrorCollector::new();
        let (unreported, plain) = rediarize(&source, &timeline, None, &errors);
        let (reported, outcome) = rediarize(
            &source,
            &reversed,
            Some(ContestedThreshold::new(0.5).expect("half share")),
            &errors,
        );
        let (above, excluded) = rediarize(
            &source,
            &timeline,
            Some(ContestedThreshold::new(0.75).expect("above half")),
            &errors,
        );
        assert!(
            !errors.has_errors(),
            "reference headers reconcile: {} {:?}",
            fixture.path().display(),
            errors.to_vec()
        );
        assert!(
            unreported.semantic_eq(&reported) && reported.semantic_eq(&above),
            "reporting thresholds and input order must not change attribution"
        );
        if source.utterances().next().is_none() {
            assert!(
                source.semantic_eq(&reported),
                "no speech is not evidence to prune declared participants"
            );
        }
        assert!(plain.contested.is_empty() && excluded.contested.is_empty());
        assert_eq!(
            (plain.reassigned, plain.unchanged),
            (outcome.reassigned, outcome.unchanged)
        );
        let expected: Vec<_> = source
            .utterances()
            .enumerate()
            .filter_map(|(index, u)| {
                u.main
                    .content
                    .bullet
                    .as_ref()
                    .filter(|b| b.timing.start_ms < b.timing.end_ms)
                    .map(|b| (index, b.timing.end_ms - b.timing.start_ms))
            })
            .collect();
        assert_eq!(outcome.contested.len(), expected.len());
        for (actual, (index, duration)) in outcome.contested.iter().zip(expected) {
            assert_eq!(actual.utterance_index, index);
            assert_eq!(
                actual.ownership.shares(),
                &[(winner.clone(), duration), (rival.clone(), duration)]
            );
            assert_eq!(
                actual.ownership.winner(),
                &winner,
                "equal ownership uses track-code tie break"
            );
            assert_eq!(
                actual.ownership.total_ms(),
                duration.checked_mul(2).expect("finite reference duration")
            );
            assert_eq!(actual.ownership.runner_up_share().as_f64(), 0.5);
            let wire = serde_json::to_value(actual).expect("contested ownership wire");
            assert_eq!(wire["utterance_index"], index);
            assert_eq!(wire["assigned"], winner.as_str());
            assert_eq!(wire["ownership"]["total_ms"], actual.ownership.total_ms());
            assert_eq!(
                wire["ownership"]["shares"],
                serde_json::json!([[winner.as_str(), duration], [rival.as_str(), duration],])
            );
            contested_witnesses += 1;
        }
        let parsed = strict_parse(parser.parse_chat_file(&reported.to_chat_string()))
            .expect("contested output parses");
        assert!(reported.semantic_eq(&parsed));
    }
    assert!(
        contested_witnesses > 0,
        "positive timed reference witnesses required"
    );
}

#[test]
fn reference_rediarization_preserves_payload_and_reconciles_used_tracks() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut changed = 0;
    let mut no_bullet = 0;
    let mut no_overlap = 0;
    for fixture in corpus.fixtures() {
        let source =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        let Some(first) = source.utterances().next() else {
            continue;
        };
        // The fresh anonymous label is external diarization input, not a claim
        // about anyone's real identity or a synthesized CHAT document.
        let fresh = SpeakerCode::new("NEWTRACK");
        assert!(!source.unique_utterance_speakers().contains(&fresh));
        for case in [
            TimelineCase::Absent,
            TimelineCase::OneTrack(first.main.speaker.clone()),
            TimelineCase::OneTrack(fresh),
        ] {
            let turns = match &case {
                TimelineCase::Absent => Vec::new(),
                TimelineCase::OneTrack(track) => source
                    .utterances()
                    .filter_map(|u| {
                        u.main.content.bullet.as_ref().map(|b| DiarizationTurn {
                            track: track.clone(),
                            span: TimeSpanMs::new(b.timing.start_ms, b.timing.end_ms)
                                .expect("reference bullet is not inverted"),
                        })
                    })
                    .collect(),
            };
            let timeline = DiarizationTimeline::new(turns);
            let errors = talkbank_model::ErrorCollector::new();
            let (output, outcome) = rediarize(&source, &timeline, None, &errors);
            assert!(
                !errors.has_errors(),
                "reference header reconciliation: {} {:?}",
                fixture.path().display(),
                errors.to_vec()
            );
            let mut expected_changed = 0;
            let mut expected_flags = Vec::new();
            assert_eq!(source.utterances().count(), output.utterances().count());
            for (index, (before, after)) in source.utterances().zip(output.utterances()).enumerate()
            {
                let mut expected = before.clone();
                match (&case, before.main.content.bullet.as_ref()) {
                    (TimelineCase::OneTrack(track), Some(b))
                        if b.timing.start_ms < b.timing.end_ms =>
                    {
                        expected.main.speaker = track.clone();
                        expected_changed += usize::from(before.main.speaker != *track);
                    }
                    (_, bullet) => expected_flags.push((
                        index,
                        before.main.speaker.clone(),
                        if bullet.is_none() {
                            FlagReason::NoBullet
                        } else {
                            FlagReason::NoOverlappingTurn
                        },
                    )),
                }
                assert!(
                    after.semantic_eq(&expected),
                    "only admitted speaker attribution may change: {}",
                    fixture.path().display()
                );
            }
            assert_eq!(outcome.reassigned, expected_changed);
            assert_eq!(
                outcome.unchanged + outcome.reassigned,
                source.utterances().count()
            );
            assert!(outcome.contested.is_empty());
            assert_eq!(outcome.flagged.len(), expected_flags.len());
            for (actual, (index, speaker, reason)) in outcome.flagged.iter().zip(expected_flags) {
                assert_eq!(
                    (actual.utterance_index, &actual.kept_speaker, actual.reason),
                    (index, &speaker, reason)
                );
                match reason {
                    FlagReason::NoBullet => no_bullet += 1,
                    FlagReason::NoOverlappingTurn => no_overlap += 1,
                }
            }
            changed += expected_changed;
            let used: HashSet<_> = output
                .utterances()
                .map(|u| u.main.speaker.clone())
                .collect();
            let participants: HashSet<_> = output
                .headers()
                .filter_map(|h| match h {
                    Header::Participants { entries } => {
                        Some(entries.iter().map(|e| e.speaker_code.clone()))
                    }
                    _ => None,
                })
                .flatten()
                .collect();
            let ids: HashSet<_> = output
                .headers()
                .filter_map(|h| match h {
                    Header::ID(id) => Some(id.speaker.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(
                participants, used,
                "participants declare exactly used tracks"
            );
            assert_eq!(ids, used, "ID rows declare exactly used tracks");
            let untouched =
                |header: &&Header| !matches!(header, Header::Participants { .. } | Header::ID(_));
            let before: Vec<_> = source.headers().filter(untouched).collect();
            let after: Vec<_> = output.headers().filter(untouched).collect();
            assert_eq!(before.len(), after.len());
            for (before, after) in before.iter().zip(after) {
                assert!(before.semantic_eq(after), "unrelated headers are preserved");
            }
            let summary =
                serde_json::to_value(RediarizeSummary::new(None, &outcome)).expect("summary wire");
            assert_eq!(summary["reassigned"], expected_changed);
            assert_eq!(
                summary["flagged"].as_array().expect("flag list").len(),
                outcome.flagged.len()
            );
            let parsed = strict_parse(parser.parse_chat_file(&output.to_chat_string()))
                .expect("rediarized output parses");
            assert!(
                output.semantic_eq(&parsed),
                "rediarized wire semantics: {}",
                fixture.path().display()
            );
        }
    }
    assert!(
        changed > 0 && no_bullet > 0 && no_overlap > 0,
        "all attribution outcomes need corpus witnesses"
    );
}
