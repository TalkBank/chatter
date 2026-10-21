//! Public split admission and execution over reference CHAT.
use talkbank_model::model::dependent_tier::wor::WorItem;
use talkbank_model::model::{Bullet, DependentTier, Utterance};
use talkbank_model::{ParseValidateOptions, WriteChat};
use talkbank_transform::rediarize::{DiarizationTimeline, DiarizationTurn, TimeSpanMs};
use talkbank_transform::utterance_split::{
    SplitChildren, SplitOutcome, SplitRefusal, TierInvalidationReason, UtteranceSplitPlan,
    WordSpeakerPartition, WordSpeakerSource, WordSpeakerSplitOutcome, WordSpeakerSplitPlan,
    WordSpeakerSplitRefusal, build_word_to_content_map,
};

const WOR: &str = include_str!("../../../../corpus/reference/tiers/wor.cha");
const GROUPS: &str = include_str!("../../../../corpus/reference/annotation/groups-regular.cha");
const REPLACEMENTS: &str =
    include_str!("../../../../corpus/reference/annotation/errors-and-replacements.cha");
const RETRACES: &str = include_str!("../../../../corpus/reference/annotation/retrace.cha");
const SEPARATORS: &str = include_str!("../../../../corpus/reference/content/separators.cha");
const CA_INTONATION: &str = include_str!("../../../../corpus/reference/ca/intonation.cha");
const PHON_INTERVALS: &str = include_str!("../../../../corpus/reference/tiers/phon-intervals.cha");
const WOR_REPLACEMENTS: &str =
    include_str!("../../../../corpus/reference/tiers/wor-replacements.cha");

#[path = "speaker_partition_corpus.rs"]
mod speaker_partition_contracts;

fn fixture(source: &str) -> talkbank_model::model::ChatFile {
    talkbank_transform::parse_and_validate(
        source,
        ParseValidateOptions::default().with_validation(),
    )
    .expect("admitted reference CHAT")
}

/// The children of a partition the test expects to split.
fn split(outcome: SplitOutcome<'_>) -> SplitChildren<'_> {
    match outcome {
        SplitOutcome::Split(split) => split,
        SplitOutcome::Unchanged => panic!("expected a split"),
    }
}

/// The children of a word-speaker partition the test expects to split.
fn speaker_split<'a>(outcome: &'a WordSpeakerSplitOutcome<'_>) -> &'a SplitChildren<'a> {
    match outcome.partition() {
        WordSpeakerPartition::Split(split) => split,
        WordSpeakerPartition::Relabeled { .. } => panic!("expected a speaker split"),
    }
}

#[test]
fn corroborated_word_timing_measures_each_child() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("timed utterance");
    let outcome = split(
        UtteranceSplitPlan::for_word_timing(source, &[0, 1])
            .expect("admitted split")
            .execute(),
    );
    assert_eq!(outcome.children().len(), 2);
    assert_eq!(outcome.invalidated_tiers().len(), 0);
    let text = outcome
        .children()
        .iter()
        .map(WriteChat::to_chat_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("corroborated_split_word_timing", text);
    let bullets: Vec<_> = outcome
        .children()
        .iter()
        .map(|child| child.main.content.bullet.as_ref().expect("measured child"))
        .collect();
    assert_eq!(
        bullets[0],
        &Bullet::new(talkbank_model::Location::Unlocated, 100, 300)
    );
    assert_eq!(
        bullets[1],
        &Bullet::new(talkbank_model::Location::Unlocated, 300, 600)
    );
}

#[test]
fn an_assignment_naming_one_child_leaves_the_source_unchanged() {
    let chat = fixture(RETRACES);
    let source = chat.utterances().nth(5).expect("morphology reference");
    let assignments =
        vec![usize::MAX; build_word_to_content_map(&source.main.content.content).len()];
    let outcome = UtteranceSplitPlan::for_morphology(source, &assignments)
        .expect("unchanged plan")
        .execute();
    assert!(matches!(outcome, SplitOutcome::Unchanged));
}

#[test]
fn count_drift_and_disjoint_labels_cannot_reorder_content() {
    let chat = fixture(WOR);
    let source = chat.utterances().nth(1).expect("three words");
    assert!(matches!(
        UtteranceSplitPlan::for_word_timing(source, &[0, 1]),
        Err(SplitRefusal::SlotCount {
            expected: 3,
            actual: 2
        })
    ));
    assert!(matches!(
        UtteranceSplitPlan::for_word_timing(source, &[0, 1, 0]),
        Err(SplitRefusal::DisjointGroup { group: 0 })
    ));
}

fn crosses_atomic_content(source: &Utterance) -> Result<UtteranceSplitPlan<'_>, SplitRefusal> {
    let map = build_word_to_content_map(&source.main.content.content);
    let boundary = map
        .windows(2)
        .position(|pair| pair[0] == pair[1])
        .expect("multiple domain slots in one content item")
        + 1;
    let mut assignments = vec![0; map.len()];
    assignments[boundary..].fill(1);
    UtteranceSplitPlan::for_morphology(source, &assignments)
}

#[test]
fn annotated_groups_cannot_be_silently_given_to_the_first_word() {
    let chat = fixture(GROUPS);
    let source = chat.utterances().next().expect("annotated group");
    assert!(matches!(
        crosses_atomic_content(source),
        Err(SplitRefusal::IndivisibleContent { content_index: 2 })
    ));
}

#[test]
fn an_indivisible_group_kept_in_one_child_preserves_every_source_item() {
    let chat = fixture(GROUPS);
    let source = chat.utterances().next().expect("annotated group");
    let assignments: Vec<_> = build_word_to_content_map(&source.main.content.content)
        .into_iter()
        .map(|index| usize::from(index >= 2))
        .collect();
    let outcome = split(
        UtteranceSplitPlan::for_morphology(source, &assignments)
            .expect("the boundary precedes, not crosses, the annotated group")
            .execute(),
    );
    assert_eq!(outcome.children().len(), 2);
    let retained: Vec<_> = outcome
        .children()
        .iter()
        .flat_map(|child| child.main.content.content.iter())
        .collect();
    let original: Vec<_> = source.main.content.content.iter().collect();
    assert_eq!(retained, original);
    assert!(
        outcome.children()[1]
            .to_chat_string()
            .contains("<what in the world> [= bar]")
    );
}

#[test]
fn a_morphology_assignment_cannot_strand_a_separator_in_a_child() {
    let chat = fixture(SEPARATORS);
    let source = chat.utterances().next().expect("well, I think so");
    assert!(matches!(
        UtteranceSplitPlan::for_morphology(source, &[0, 1, 1, 1, 1]),
        Err(SplitRefusal::LeadingSeparator { content_index: 1 })
    ));
    let kept = split(
        UtteranceSplitPlan::for_morphology(source, &[0, 0, 1, 1, 1])
            .expect("the comma travels with its preceding word")
            .execute(),
    );
    assert_eq!(kept.children().len(), 2);
    assert!(kept.children()[0].to_chat_string().contains("well , ."));
}

#[test]
fn a_ca_split_preserves_the_authored_unmarked_last_ending() {
    let chat = fixture(CA_INTONATION);
    let source = chat.utterances().next().expect("rising to high");
    assert!(
        source.main.content.terminator.is_none(),
        "the CA contour is content, not a terminator"
    );
    let outcome = split(
        UtteranceSplitPlan::for_word_timing(source, &[0, 1, 1])
            .expect("two source-bounded children")
            .execute(),
    );
    assert_eq!(outcome.children().len(), 2);
    assert_eq!(
        outcome.children()[1].main.content.terminator,
        source.main.content.terminator
    );
    assert!(
        outcome.children()[1]
            .to_chat_string()
            .ends_with("to high ⇗\n")
    );
}

#[test]
fn phonological_intervals_are_invalidated_against_their_actual_source_tier() {
    let chat = fixture(PHON_INTERVALS);
    let source = chat.utterances().next().expect("phonological intervals");
    let outcome = split(
        UtteranceSplitPlan::for_word_timing(source, &[0, 0, 1, 1, 1, 1])
            .expect("a boundary after the second spoken word")
            .execute(),
    );
    assert_eq!(outcome.children().len(), 2);
    let interval_loss = outcome
        .invalidated_tiers()
        .iter()
        .find(|loss| matches!(loss.tier(), DependentTier::Xphoint(_)))
        .expect("the actual interval tier has a loss receipt");
    assert_eq!(
        interval_loss.reason(),
        TierInvalidationReason::BoundaryDependent
    );
    assert!(std::ptr::eq(
        interval_loss.tier(),
        &source.dependent_tiers[interval_loss.index()].tier
    ));
    assert!(outcome.children().iter().all(|child| {
        child
            .dependent_tiers
            .iter()
            .all(|entry| !matches!(entry.tier, DependentTier::Xphoint(_)))
    }));
}

#[test]
fn replacement_word_timing_partitions_spoken_originals_not_replacement_targets() {
    let chat = fixture(WOR_REPLACEMENTS);
    assert_eq!(
        chat.utterances().count(),
        2,
        "single- and multiple-target controls"
    );
    for (source, start) in chat.utterances().zip([100, 500]) {
        let outcome = split(
            UtteranceSplitPlan::for_word_timing(source, &[0, 0, 1, 1])
                .expect("four spoken slots, regardless of target length")
                .execute(),
        );
        assert_eq!(outcome.children().len(), 2);
        assert!(outcome.invalidated_tiers().is_empty());
        assert_eq!(
            outcome.children()[0].main.content.bullet,
            Some(Bullet::new(
                talkbank_model::Location::Unlocated,
                start,
                start + 200
            ))
        );
        assert_eq!(
            outcome.children()[1].main.content.bullet,
            Some(Bullet::new(
                talkbank_model::Location::Unlocated,
                start + 200,
                start + 400
            ))
        );
        let last = &outcome.children()[1];
        assert_eq!(
            last.wor_tier()
                .expect("corroborated originals")
                .words()
                .map(|word| word.cleaned_text())
                .collect::<Vec<_>>(),
            vec!["child", "speaking"]
        );
        assert!(last.to_chat_string().contains("child [:"));
    }
}

#[test]
fn replacement_targets_stay_atomic_and_domains_are_not_interchangeable() {
    let chat = fixture(REPLACEMENTS);
    let source = chat.utterances().nth(5).expect("two-target replacement");
    assert!(matches!(
        crosses_atomic_content(source),
        Err(SplitRefusal::IndivisibleContent { content_index: 2 })
    ));
    assert!(matches!(
        UtteranceSplitPlan::for_morphology(source, &[0, 0, 1, 1]),
        Err(SplitRefusal::SlotCount {
            expected: 5,
            actual: 4
        })
    ));
    let spoken = split(
        UtteranceSplitPlan::for_word_timing(source, &[0, 0, 1, 1])
            .expect("four spoken original slots")
            .execute(),
    );
    assert_eq!(spoken.children().len(), 2);
    assert!(
        spoken.children()[1]
            .to_chat_string()
            .contains("child [: a+er b] speaking")
    );
}

#[test]
fn boundary_dependent_analysis_has_explicit_source_bound_loss_receipts() {
    let chat = fixture(RETRACES);
    let source = chat.utterances().nth(5).expect("mor and gra reference");
    let outcome = split(
        UtteranceSplitPlan::for_morphology(source, &[0, 1, 1])
            .expect("after retrace and kept word")
            .execute(),
    );
    assert_eq!(outcome.invalidated_tiers().len(), 2);
    for (index, loss) in outcome.invalidated_tiers().iter().enumerate() {
        assert_eq!(loss.index(), index);
        assert_eq!(loss.reason(), TierInvalidationReason::BoundaryDependent);
        assert!(std::ptr::eq(
            loss.tier(),
            &source.dependent_tiers[index].tier
        ));
    }
    assert!(
        outcome
            .children()
            .iter()
            .all(|child| child.dependent_tiers.is_empty())
    );
    assert!(
        outcome.children()[0]
            .to_chat_string()
            .contains("[//] kitty")
    );
}

fn timeline(turns: &[(&str, u64, u64)]) -> DiarizationTimeline {
    DiarizationTimeline::new(
        turns
            .iter()
            .map(|(track, start, end)| DiarizationTurn {
                track: talkbank_model::SpeakerCode::new(*track),
                span: TimeSpanMs::new(*start, *end).expect("control interval"),
            })
            .collect(),
    )
}

#[test]
fn speaker_runs_can_return_to_an_earlier_track_without_reordering_words() {
    let chat = fixture(WOR);
    let source = chat.utterances().nth(1).expect("three measured words");
    let turns = timeline(&[("CHI", 700, 800), ("MOT", 800, 1000), ("CHI", 1000, 1200)]);
    let plan = WordSpeakerSplitPlan::admit(source, &turns).expect("measured speaker changes");
    assert_eq!(plan.ownership().len(), 3);
    let outcome = plan.execute().expect("producer retains every speaker run");
    assert_eq!(outcome.ownership().len(), 3);
    let children = speaker_split(&outcome);
    let records: Vec<_> = children
        .children()
        .iter()
        .map(|child| {
            let bullet = child
                .main
                .content
                .bullet
                .as_ref()
                .expect("measured child hull");
            (
                child.main.speaker.as_str(),
                bullet.timing.start_ms,
                bullet.timing.end_ms,
            )
        })
        .collect();
    assert_eq!(
        records,
        vec![("CHI", 700, 800), ("MOT", 800, 1000), ("CHI", 1000, 1200)]
    );
    assert!(children.children()[0].to_chat_string().contains("how ."));
    assert!(children.children()[1].to_chat_string().contains("are ."));
    assert!(children.children()[2].to_chat_string().contains("you ?"));
    assert!(children.invalidated_tiers().is_empty());
    assert_eq!(source.main.speaker.as_str(), "MOT");
    let parts = outcome.into_parts();
    let WordSpeakerPartition::Split(split) = parts.partition else {
        panic!("consumed speaker split")
    };
    assert_eq!(parts.ownership.len(), 3);
    assert_eq!(split.children()[1].main.speaker.as_str(), "MOT");
}

#[test]
fn speaker_word_ownership_unions_duplicate_turns_instead_of_double_counting() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("two words");
    let turns = timeline(&[("CHI", 100, 600), ("MOT", 100, 250), ("MOT", 100, 250)]);
    let plan = WordSpeakerSplitPlan::admit(source, &turns).expect("unique union-duration winners");
    assert_eq!(
        plan.ownership().first().shares(),
        &[
            (talkbank_model::SpeakerCode::new("CHI"), 200),
            (talkbank_model::SpeakerCode::new("MOT"), 150),
        ]
    );
    let outcome = plan.execute().expect("one speaker run");
    assert!(matches!(outcome.partition(),
        WordSpeakerPartition::Relabeled { speaker } if speaker.as_str() == "CHI"));
    assert_eq!(outcome.ownership().len(), 2);
    let parts = outcome.into_parts();
    assert!(matches!(parts.partition,
        WordSpeakerPartition::Relabeled { speaker } if speaker.as_str() == "CHI"));
    assert_eq!(parts.ownership.first().winner().as_str(), "CHI");
    assert_eq!(parts.ownership.last().winner().as_str(), "CHI");
    assert_eq!(parts.ownership.following().len(), 1);
    let encoded = serde_json::to_value(&parts.ownership).expect("ownership array wire");
    let expected = serde_json::to_value((&parts.ownership).into_iter().collect::<Vec<_>>())
        .expect("independent array view");
    assert_eq!(encoded, expected);
    assert!(
        encoded.is_array(),
        "implementation fields are not the public wire"
    );
    assert_eq!(
        parts.ownership.is_empty(),
        encoded.as_array().expect("ownership array").is_empty()
    );
    assert_eq!(parts.ownership.into_iter().count(), 2);
}

#[test]
fn singleton_speaker_measurement_preserves_the_source_and_array_wire() {
    let chat = fixture(WOR);
    let source = chat.utterances().nth(2).expect("one timed Cantonese word");
    let turns = timeline(&[("MOT", 565499, 566219), ("CHI", 565499, 565600)]);
    let plan = WordSpeakerSplitPlan::admit(source, &turns).expect("unique held-time winner");
    let outcome = plan
        .execute()
        .expect("singleton relabeling needs no structural split");
    assert!(matches!(outcome.partition(),
        WordSpeakerPartition::Relabeled { speaker } if speaker.as_str() == "MOT"));
    assert_eq!(source.main.speaker.as_str(), "CHI");
    let parts = outcome.into_parts();
    assert_eq!(
        parts.ownership.first().shares(),
        &[
            (talkbank_model::SpeakerCode::new("MOT"), 720),
            (talkbank_model::SpeakerCode::new("CHI"), 101),
        ]
    );
    assert_eq!(parts.ownership.last().winner().as_str(), "MOT");
    let encoded = serde_json::to_value(&parts.ownership).expect("singleton array wire");
    let array = encoded
        .as_array()
        .expect("array rather than implementation fields");
    assert_eq!(array.len(), 1);
    assert_eq!(parts.ownership.len(), array.len());
    assert_eq!(parts.ownership.is_empty(), array.is_empty());
    assert_eq!(
        parts
            .ownership
            .into_iter()
            .map(|held| held.total_ms())
            .collect::<Vec<_>>(),
        [821]
    );
}

#[test]
fn an_uncovered_word_cannot_inherit_the_nearest_or_previous_speaker() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("two words");
    for (turns, expected_slot) in [
        (timeline(&[("MOT", 700, 1200)]), 0),
        (timeline(&[("CHI", 100, 300), ("MOT", 700, 1200)]), 1),
    ] {
        assert!(matches!(
            WordSpeakerSplitPlan::admit(source, &turns),
            Err(WordSpeakerSplitRefusal::UncoveredWord { slot }) if slot == expected_slot
        ));
    }
}

#[test]
fn tied_word_ownership_is_not_resolved_by_track_name_or_turn_order() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("two words");
    for (turns, expected_slot) in [
        (timeline(&[("CHI", 100, 600), ("MOT", 100, 600)]), 0),
        (timeline(&[("MOT", 100, 600), ("CHI", 100, 600)]), 0),
        (timeline(&[("CHI", 100, 600), ("MOT", 300, 600)]), 1),
    ] {
        assert!(matches!(
            WordSpeakerSplitPlan::admit(source, &turns),
            Err(WordSpeakerSplitRefusal::TiedWord { slot, .. }) if slot == expected_slot
        ));
    }
}

#[test]
fn a_parent_bullet_cannot_replace_missing_or_incomplete_word_evidence() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("timed source");
    let turns = timeline(&[("CHI", 100, 600)]);
    let mut missing = source.clone();
    missing.dependent_tiers.clear();
    assert!(matches!(
        WordSpeakerSplitPlan::admit(&missing, &turns),
        Err(WordSpeakerSplitRefusal::MissingWordTiming)
    ));
    let mut incomplete = source.clone();
    let DependentTier::Wor(wor) = &mut incomplete.dependent_tiers[0].tier else {
        panic!("reference word tier")
    };
    let WorItem::Word(word) = &mut wor.items[0] else {
        panic!("reference word")
    };
    word.inline_bullet = None;
    assert!(matches!(
        WordSpeakerSplitPlan::admit(&incomplete, &turns),
        Err(WordSpeakerSplitRefusal::IncompleteWordTiming { .. })
    ));
}

/// Invalid API inputs must refuse before acoustic inference, not mint a plan.
#[test]
fn word_speaker_source_refuses_empty_drifted_and_lexically_stale_timing() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("two measured source words");
    let mut drifted = source.clone();
    let DependentTier::Wor(wor) = &mut drifted.dependent_tiers[0].tier else {
        panic!("reference word tier")
    };
    wor.items.pop();
    assert!(matches!(
        WordSpeakerSource::admit(&drifted),
        Err(WordSpeakerSplitRefusal::WordCount { main: 2, wor: 1 })
    ));

    let mut stale = source.clone();
    let DependentTier::Wor(wor) = &mut stale.dependent_tiers[0].tier else {
        panic!("reference word tier")
    };
    wor.items[0] = wor.items[1].clone();
    assert!(matches!(
        WordSpeakerSource::admit(&stale),
        Err(WordSpeakerSplitRefusal::LexicalMismatch { slots }) if slots == vec![0]
    ));

    let mut empty = source.clone();
    empty.main.content.content = Vec::new().into();
    let DependentTier::Wor(wor) = &mut empty.dependent_tiers[0].tier else {
        panic!("reference word tier")
    };
    wor.items.clear();
    assert!(matches!(
        WordSpeakerSource::admit(&empty),
        Err(WordSpeakerSplitRefusal::EmptyWordTiming)
    ));
    empty.dependent_tiers.clear();
    assert!(matches!(
        WordSpeakerSource::admit(&empty),
        Err(WordSpeakerSplitRefusal::EmptyWordTiming)
    ));
}

#[test]
fn complete_word_timing_is_admitted_before_a_timeline_is_available() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("timed source");
    let admitted = WordSpeakerSource::admit(source).expect("eligible source before inference");
    assert_eq!(admitted.timing().slots().len(), 2);
    assert_eq!(admitted.timing().hull().start().get(), 100);
    assert_eq!(admitted.timing().hull().end().get(), 600);
    let turns = timeline(&[("CHI", 100, 300), ("MOT", 300, 600)]);
    let outcome = admitted
        .bind_timeline(&turns)
        .expect("same admitted source")
        .execute()
        .expect("two measured children");
    assert_eq!(speaker_split(&outcome).children().len(), 2);
    assert_eq!(outcome.ownership().len(), 2);
}
