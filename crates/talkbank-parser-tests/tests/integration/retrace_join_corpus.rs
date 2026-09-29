//! Explicit repair-policy contracts over authored E370 turn-split specimens.

use talkbank_model::model::{
    BracketedContent, Line, SemanticEq, TranscriptName, UtteranceContent, WriteChat,
};
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;
use talkbank_transform::join_retrace::{RetraceJoinScope, join_dangling_retraces};

#[test]
fn spec_split_retraces_preserve_the_repair_policy_ladder() {
    let parser = TreeSitterParser::new().expect("parser");
    let load = |example| {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E370_split_retrace_{example}.cha",
        ))).expect("canonical split-retrace specimen");
        strict_parse(parser.parse_chat_file(&source)).expect("spec parses without recovery")
    };
    for (example, joins, control) in [
        (1, [0, 0, 0], None),
        (2, [1, 1, 1], Some(1)),
        (3, [0, 0, 1], None),
        (4, [0, 0, 1], None),
        (5, [0, 0, 1], None),
        (6, [0, 0, 0], None),
        (7, [0, 1, 1], Some(6)),
        (8, [0, 0, 0], None),
        (9, [0, 0, 0], None),
        (10, [2, 2, 2], Some(9)),
        (11, [0, 0, 0], None),
        (12, [0, 0, 1], Some(11)),
        (13, [0, 0, 0], None),
    ] {
        let original = load(example);
        for (scope, expected_joins) in [
            RetraceJoinScope::RepetitionOnly,
            RetraceJoinScope::RepetitionAndCorrections,
            RetraceJoinScope::AllSameSpeakerSuccessor,
        ]
        .into_iter()
        .zip(joins)
        {
            let mut repaired = original.clone();
            let stats = join_dangling_retraces(&mut repaired, scope);
            assert_eq!(
                stats.joined_utterances, expected_joins,
                "example {example}, {scope:?}"
            );
            assert_eq!(stats.is_empty(), expected_joins == 0);
            assert_eq!(stats.needs_remorphotag, 0);
            assert_eq!(stats.dependent_tiers_dropped, 0);
            if expected_joins > 0 {
                assert_eq!(
                    repaired.utterances().count() + expected_joins,
                    original.utterances().count()
                );
                if let Some(control) = control {
                    assert!(
                        repaired.semantic_eq(&load(control)),
                        "joined model must match authored unsplit control"
                    );
                }
            } else {
                assert!(
                    repaired.semantic_eq(&original),
                    "refusal must preserve all content and headers"
                );
            }
            let errors = ErrorCollector::new();
            repaired.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let remains_dangling = errors
                .into_vec()
                .iter()
                .any(|error| error.code == ErrorCode::StructuralOrderError);
            assert_eq!(
                remains_dangling,
                expected_joins == 0 && ![1, 6, 9, 11, 13].contains(&example)
            );
            assert!(
                join_dangling_retraces(&mut repaired, scope).is_empty(),
                "repair is idempotent"
            );
        }
    }
}

#[test]
fn reference_successor_attributes_and_tiers_follow_split_retrace_policy() {
    let parser = TreeSitterParser::new().expect("parser");
    let source = std::fs::read_to_string(workspace_root().join(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E370_split_retrace_2.cha",
    )).expect("canonical split retrace");
    let split = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax");
    // A mutable editing draft is not a validated CHAT document. The public
    // bracketed-content constructor admits deletion of every item; neither
    // conservative scope may treat the resulting empty prefix as a match.
    let mut empty_draft = split.clone();
    let previous = (&mut empty_draft.lines)
        .into_iter()
        .find_map(|line| match line {
            Line::Utterance(utterance) => Some(utterance),
            _ => None,
        })
        .expect("split previous");
    let Some(UtteranceContent::Retrace(retrace)) =
        (&mut previous.main.content.content).into_iter().last()
    else {
        panic!("spec ends in a retrace");
    };
    retrace.content = BracketedContent::new(Vec::new());
    let empty_before = serde_json::to_value(&empty_draft).expect("editing draft JSON");
    for scope in [
        RetraceJoinScope::RepetitionOnly,
        RetraceJoinScope::RepetitionAndCorrections,
    ] {
        let stats = join_dangling_retraces(&mut empty_draft, scope);
        assert!(stats.is_empty());
        assert_eq!(stats.dependent_tiers_dropped, 0);
        assert_eq!(stats.needs_remorphotag, 0);
        assert_eq!(
            serde_json::to_value(&empty_draft).expect("unchanged draft JSON"),
            empty_before
        );
    }
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut language = None;
    let mut linkers = None;
    let mut tiers = None;
    let mut other_speaker = None;
    let mut timing = Vec::new();
    let split_speaker = &split
        .utterances()
        .next()
        .expect("split previous")
        .main
        .speaker;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference syntax");
        for utterance in file.utterances() {
            if other_speaker.is_none() && &utterance.main.speaker != split_speaker {
                other_speaker = Some(utterance.main.speaker.clone());
            }
            if timing.len() < 2
                && let Some(bullet) = &utterance.main.content.bullet
            {
                timing.push(bullet.clone());
            }
            if language.is_none() {
                language = utterance.main.content.language_code.clone();
            }
            if linkers.is_none() && !utterance.main.content.linkers.is_empty() {
                linkers = Some(utterance.main.content.linkers.clone());
            }
            if tiers.is_none() && !utterance.dependent_tiers.is_empty() {
                tiers = Some(utterance.dependent_tiers.clone());
            }
        }
        if language.is_some()
            && linkers.is_some()
            && tiers.is_some()
            && other_speaker.is_some()
            && timing.len() == 2
        {
            break;
        }
    }
    let language = language.expect("reference utterance-scoped language witness");
    let linkers = linkers.expect("reference leading-linker witness");
    let tiers = tiers.expect("reference dependent-tier witness");
    let other_speaker = other_speaker.expect("reference different-speaker witness");
    let [first_bullet, second_bullet]: [_; 2] =
        timing.try_into().expect("two reference timing witnesses");

    enum ProtectedAttribute {
        Language,
        Linkers,
        Speaker,
    }
    for attribute in [
        ProtectedAttribute::Language,
        ProtectedAttribute::Linkers,
        ProtectedAttribute::Speaker,
    ] {
        let mut candidate = split.clone();
        let successor = (&mut candidate.lines)
            .into_iter()
            .filter_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance),
                _ => None,
            })
            .nth(1)
            .expect("split successor");
        match attribute {
            ProtectedAttribute::Language => {
                successor.main.content.language_code = Some(language.clone())
            }
            ProtectedAttribute::Linkers => successor.main.content.linkers = linkers.clone(),
            ProtectedAttribute::Speaker => successor.main.speaker = other_speaker.clone(),
        }
        let before = candidate.to_chat_string();
        for scope in [
            RetraceJoinScope::RepetitionOnly,
            RetraceJoinScope::RepetitionAndCorrections,
            RetraceJoinScope::AllSameSpeakerSuccessor,
        ] {
            let stats = join_dangling_retraces(&mut candidate, scope);
            assert!(stats.is_empty());
            assert_eq!(stats.dependent_tiers_dropped, 0);
            assert_eq!(stats.needs_remorphotag, 0);
            assert_eq!(
                candidate.to_chat_string(),
                before,
                "refusal preserves successor attributes"
            );
        }
    }

    // Optional timing is an editing-boundary contract, independent of whether
    // these transplanted reference times form a validated media timeline.
    for (previous, successor) in [
        (None, None),
        (Some(&first_bullet), None),
        (None, Some(&second_bullet)),
        (Some(&first_bullet), Some(&second_bullet)),
    ] {
        let mut candidate = split.clone();
        for (utterance, bullet) in (&mut candidate.lines)
            .into_iter()
            .filter_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance),
                _ => None,
            })
            .zip([previous, successor])
        {
            utterance.main.content.bullet = bullet.cloned();
        }
        let stats = join_dangling_retraces(&mut candidate, RetraceJoinScope::RepetitionOnly);
        assert_eq!(stats.joined_utterances, 1);
        let actual = candidate.utterances().next().expect("joined turn");
        let expected = match (previous, successor) {
            (Some(left), Some(right)) => Some((left.timing.start_ms, right.timing.end_ms)),
            (Some(bullet), None) | (None, Some(bullet)) => {
                Some((bullet.timing.start_ms, bullet.timing.end_ms))
            }
            (None, None) => None,
        };
        assert_eq!(
            actual
                .main
                .content
                .bullet
                .as_ref()
                .map(|bullet| (bullet.timing.start_ms, bullet.timing.end_ms)),
            expected
        );
        assert_eq!(stats.needs_remorphotag, 0);
        assert!(
            join_dangling_retraces(&mut candidate, RetraceJoinScope::RepetitionOnly).is_empty()
        );
    }

    // Typed editing inputs deliberately carry stale alignment. This repair's
    // contract discards both owners' tiers and requests remorphotagging; it
    // must not claim the transplanted tiers align with the split specimen.
    for owners in [vec![0], vec![1], vec![0, 1]] {
        let mut candidate = split.clone();
        for (index, utterance) in (&mut candidate.lines)
            .into_iter()
            .filter_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance),
                _ => None,
            })
            .enumerate()
        {
            if owners.contains(&index) {
                utterance.dependent_tiers = tiers.clone();
            }
        }
        let stats = join_dangling_retraces(&mut candidate, RetraceJoinScope::RepetitionOnly);
        assert_eq!(stats.joined_utterances, 1);
        assert_eq!(stats.needs_remorphotag, 1);
        assert_eq!(stats.dependent_tiers_dropped, owners.len() * tiers.len());
        assert!(
            candidate
                .utterances()
                .all(|utterance| utterance.dependent_tiers.is_empty())
        );
        let mut control = split.clone();
        join_dangling_retraces(&mut control, RetraceJoinScope::RepetitionOnly);
        assert!(
            candidate.semantic_eq(&control),
            "tier invalidation preserves the joined main tier"
        );
        assert!(
            join_dangling_retraces(&mut candidate, RetraceJoinScope::RepetitionOnly).is_empty()
        );
    }
}
