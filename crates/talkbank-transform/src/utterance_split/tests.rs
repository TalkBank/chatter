//! Split admission and execution over reference CHAT.
use super::content_groups::as_retrace;
use super::*;
use crate::rediarize::DiarizationTimeline;
use talkbank_model::model::Bullet;
use talkbank_model::model::dependent_tier::wor::WorItem;
use talkbank_model::{ParseValidateOptions, WriteChat};

const WOR: &str = include_str!("../../../../corpus/reference/tiers/wor.cha");
const GROUPS: &str = include_str!("../../../../corpus/reference/annotation/groups-regular.cha");
const REPLACEMENTS: &str =
    include_str!("../../../../corpus/reference/annotation/errors-and-replacements.cha");
const RETRACES: &str = include_str!("../../../../corpus/reference/annotation/retrace.cha");
const RETRACE_RUNS: &str =
    include_str!("../../../../corpus/reference/languages/ita-conversation.cha");

fn fixture(source: &str) -> talkbank_model::model::ChatFile {
    crate::parse_and_validate(source, ParseValidateOptions::default().with_validation())
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
    assert_eq!(bullets[0], &Bullet::new(100, 300));
    assert_eq!(bullets[1], &Bullet::new(300, 600));
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

/// The group count sizes the per-child collections in `rebuild_split`; it
/// must come from the number of distinct runs, never a label's magnitude.
#[test]
fn the_group_count_is_the_run_count_not_the_largest_label() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("source");
    let plan = UtteranceSplitPlan::for_word_timing(source, &[usize::MAX, 42])
        .expect("source-bounded labels");
    let groups = plan.content_groups.as_ref().expect("two runs split");
    assert_eq!(groups.group_count(), 2);
    assert_eq!(split(plan.execute()).children().len(), 2);
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

/// A run of retraces binds forward as a unit: with the boundary placed
/// just before the run, every retrace in it, and the fragment between the
/// last one and its material, travels with the material's child.
#[test]
fn a_run_of_retraces_travels_with_the_material_after_it() {
    let chat = fixture(RETRACE_RUNS);
    let source = chat
        .utterances()
        .nth(2)
        .expect("utterance with a run of retraces");
    let content = &source.main.content.content;
    let run_start = (0..content.len().saturating_sub(1))
        .find(|&index| {
            as_retrace(&content[index]).is_some() && as_retrace(&content[index + 1]).is_some()
        })
        .expect("two adjacent retraces");
    let assignments: Vec<usize> = build_word_to_content_map(content)
        .into_iter()
        .map(|content_index| usize::from(content_index >= run_start))
        .collect();
    let outcome = split(
        UtteranceSplitPlan::for_morphology(source, &assignments)
            .expect("boundary before the run")
            .execute(),
    );
    let text = outcome
        .children()
        .iter()
        .map(WriteChat::to_chat_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("retrace_run_travels_forward", text);
}

fn timeline(turns: &[(&str, u64, u64)]) -> DiarizationTimeline {
    DiarizationTimeline::new(
        turns
            .iter()
            .map(|(track, start, end)| crate::rediarize::DiarizationTurn {
                track: talkbank_model::SpeakerCode::new(*track),
                span: crate::rediarize::TimeSpanMs::new(*start, *end).expect("control interval"),
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
}

#[test]
fn speaker_word_ownership_unions_duplicate_turns_instead_of_double_counting() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("two words");
    let turns = timeline(&[("CHI", 100, 600), ("MOT", 100, 250), ("MOT", 100, 250)]);
    let plan = WordSpeakerSplitPlan::admit(source, &turns).expect("unique union-duration winners");
    assert_eq!(
        plan.ownership()[0].shares(),
        &[
            (talkbank_model::SpeakerCode::new("CHI"), 200),
            (talkbank_model::SpeakerCode::new("MOT"), 150),
        ]
    );
    let outcome = plan.execute().expect("one speaker run");
    assert!(matches!(outcome.partition(),
        WordSpeakerPartition::Relabeled { speaker } if speaker.as_str() == "CHI"));
    assert_eq!(outcome.ownership().len(), 2);
}

#[test]
fn an_uncovered_word_cannot_inherit_the_nearest_or_previous_speaker() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("two words");
    let turns = timeline(&[("CHI", 100, 300), ("MOT", 700, 1200)]);
    assert!(matches!(
        WordSpeakerSplitPlan::admit(source, &turns),
        Err(WordSpeakerSplitRefusal::UncoveredWord { slot: 1 })
    ));
}

#[test]
fn tied_word_ownership_is_not_resolved_by_track_name_or_turn_order() {
    let chat = fixture(WOR);
    let source = chat.utterances().next().expect("two words");
    for turns in [
        timeline(&[("CHI", 100, 600), ("MOT", 100, 600)]),
        timeline(&[("MOT", 100, 600), ("CHI", 100, 600)]),
    ] {
        assert!(matches!(
            WordSpeakerSplitPlan::admit(source, &turns),
            Err(WordSpeakerSplitRefusal::TiedWord { slot: 0, .. })
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

#[test]
fn word_timing_ineligibility_is_reported_without_acoustic_evidence() {
    let chat = fixture(WOR);
    let mut source = chat.utterances().next().expect("timed source").clone();
    source.dependent_tiers.clear();
    assert!(matches!(
        WordSpeakerSource::admit(&source),
        Err(WordSpeakerSplitRefusal::MissingWordTiming)
    ));
    // A boundary-construction control, not an additional CHAT corpus case.
    source.main.content.content = Vec::new().into();
    assert!(matches!(
        WordSpeakerSource::admit(&source),
        Err(WordSpeakerSplitRefusal::EmptyWordTiming)
    ));
}
