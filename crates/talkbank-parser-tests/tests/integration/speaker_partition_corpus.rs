//! Structural speaker-partition policy over admitted reference models.
//!
//! Groups without authored timing receive an explicitly generated external
//! timing input. The canonical main-tier projection generates its words and
//! separators; these controls neither invent CHAT nor claim authored timing.
use talkbank_model::WriteChat;
use talkbank_model::model::{Bullet, DependentTier, Utterance, UtteranceContent};
use talkbank_transform::rediarize::{DiarizationTimeline, DiarizationTurn, TimeSpanMs};
use talkbank_transform::utterance_split::{
    SplitOutcome, SplitRefusal, UtteranceSplitPlan, WordSpeakerPartition, WordSpeakerSource,
    WordSpeakerSplitRefusal,
};

use super::{GROUPS, SEPARATORS, WOR_REPLACEMENTS, fixture, speaker_split};

fn with_generated_test_timing(source: &Utterance) -> Utterance {
    let mut wor = source.main.generate_wor_tier();
    for (slot, word) in wor
        .items
        .iter_mut()
        .filter_map(|item| match item {
            super::WorItem::Word(word) => Some(word),
            super::WorItem::Separator { .. } => None,
        })
        .enumerate()
    {
        let start = 100 * (1 + u64::try_from(slot).expect("finite reference word position"));
        word.inline_bullet = Some(Bullet::new(
            talkbank_model::Location::Unlocated,
            start,
            start + 100,
        ));
    }
    source.clone().add_dependent_tier(DependentTier::Wor(wor))
}

fn word_turns(
    source: &WordSpeakerSource<'_>,
    track: impl Fn(usize) -> &'static str,
) -> DiarizationTimeline {
    DiarizationTimeline::new(
        source
            .timing()
            .slots()
            .iter()
            .enumerate()
            .map(|(slot, word)| DiarizationTurn {
                track: talkbank_model::SpeakerCode::new(track(slot)),
                span: TimeSpanMs::new(word.timing().start().get(), word.timing().end().get())
                    .expect("admitted positive word interval"),
            })
            .collect(),
    )
}

/// A real alternative policy: the boundary is before, not inside, the group.
#[test]
fn speaker_boundary_before_annotated_group_preserves_the_whole_group() {
    let chat = fixture(GROUPS);
    let source = with_generated_test_timing(chat.utterances().next().expect("annotated group"));
    let admitted = WordSpeakerSource::admit(&source).expect("complete corroborated timing");
    assert_eq!(admitted.timing().slots().len(), 8);
    let turns = word_turns(&admitted, |slot| if slot < 2 { "CHI" } else { "MOT" });
    let outcome = admitted
        .bind_timeline(&turns)
        .expect("speaker change precedes the whole group")
        .execute()
        .expect("source-affiliated children");
    let split = speaker_split(&outcome);
    let children = split.children();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].main.speaker.as_str(), "CHI");
    assert_eq!(children[1].main.speaker.as_str(), "MOT");
    assert!(
        children[1]
            .to_chat_string()
            .contains("<what in the world> [= bar]")
    );
    let retained: Vec<_> = children
        .iter()
        .flat_map(|child| child.main.content.content.iter())
        .collect();
    assert_eq!(
        retained,
        source.main.content.content.iter().collect::<Vec<_>>()
    );
    let child_slot_counts: Vec<_> = children
        .iter()
        .map(|child| {
            WordSpeakerSource::admit(child)
                .expect("child word tier corroborates its actual grouped main tier")
                .timing()
                .slots()
                .len()
        })
        .collect();
    assert_eq!(child_slot_counts, [2, 6]);
}

/// Timing and unique winners cannot override structural indivisibility.
#[test]
fn speaker_boundary_inside_annotated_group_refuses_after_timing_admission() {
    let chat = fixture(GROUPS);
    let source = with_generated_test_timing(chat.utterances().next().expect("annotated group"));
    let admitted = WordSpeakerSource::admit(&source).expect("complete corroborated timing");
    let turns = word_turns(&admitted, |slot| if slot < 3 { "CHI" } else { "MOT" });
    assert!(matches!(
        admitted.bind_timeline(&turns),
        Err(WordSpeakerSplitRefusal::Partition(
            SplitRefusal::IndivisibleContent { content_index: 2 }
        ))
    ));
}

/// Nested annotations have the same whole-unit versus internal-boundary policy.
#[test]
fn nested_annotated_groups_remain_single_speaker_units() {
    let chat = fixture(GROUPS);
    for (position, expected_words) in [(2, 2), (3, 3)] {
        let original = chat
            .utterances()
            .nth(position)
            .expect("both nested annotation controls must exist");
        let source = with_generated_test_timing(original);
        let admitted = WordSpeakerSource::admit(&source).expect("nested group timing");
        assert_eq!(admitted.timing().slots().len(), expected_words);
        let turns = word_turns(&admitted, |_| "MOT");
        let outcome = admitted
            .bind_timeline(&turns)
            .expect("whole nested group has one winner")
            .execute()
            .expect("no structural rebuilding for one speaker");
        assert!(matches!(outcome.partition(),
            WordSpeakerPartition::Relabeled { speaker } if speaker.as_str() == "MOT"));
        assert_eq!(outcome.ownership().len(), expected_words);

        let admitted = WordSpeakerSource::admit(&source).expect("same nested group timing");
        let turns = word_turns(&admitted, |slot| if slot == 0 { "CHI" } else { "MOT" });
        assert!(matches!(
            admitted.bind_timeline(&turns),
            Err(WordSpeakerSplitRefusal::Partition(
                SplitRefusal::IndivisibleContent { content_index: 0 }
            ))
        ));
    }
}

/// Speaker runs follow recorded spoken-original timing, not replacement targets.
#[test]
fn speaker_runs_measure_replacement_originals_not_target_words() {
    let chat = fixture(WOR_REPLACEMENTS);
    let mut cases = 0;
    for source in chat.utterances() {
        cases += 1;
        let admitted = WordSpeakerSource::admit(source).expect("authored replacement timing");
        assert_eq!(admitted.timing().slots().len(), 4);
        let turns = word_turns(&admitted, |slot| if slot % 2 == 0 { "CHI" } else { "MOT" });
        let outcome = admitted
            .bind_timeline(&turns)
            .expect("each spoken original has one unique winner")
            .execute()
            .expect("replacement targets stay with their original");
        let children = speaker_split(&outcome).children();
        assert_eq!(children.len(), 4);
        assert_eq!(
            children[2].main.content.content[0],
            source.main.content.content[2]
        );
        for (slot, child) in children.iter().enumerate() {
            assert_eq!(
                child.main.speaker.as_str(),
                if slot % 2 == 0 { "CHI" } else { "MOT" }
            );
            assert_eq!(
                WordSpeakerSource::admit(child)
                    .expect("one original word corroborates its child word tier")
                    .timing()
                    .slots()
                    .len(),
                1
            );
        }
    }
    assert_eq!(cases, 2, "single- and multiple-target authored controls");
}

/// A controlled invalid-layout input still cannot mint a leading-separator child.
#[test]
fn timed_leading_tag_markers_cannot_become_speaker_split_children() {
    let chat = fixture(SEPARATORS);
    let mut cases = 0;
    for original in chat.utterances().skip(1).take(2) {
        cases += 1;
        let marker = original
            .main
            .content
            .content
            .iter()
            .position(|item| matches!(item, UtteranceContent::Separator(_)))
            .expect("authored tag-question or vocative separator");
        let mut source = original.clone();
        source.main.content.content = source.main.content.content[marker..].to_vec().into();
        let source = with_generated_test_timing(&source);
        let admitted =
            WordSpeakerSource::admit(&source).expect("complete timing does not admit layout");
        assert_eq!(admitted.timing().slots().len(), 2);
        let turns = word_turns(&admitted, |slot| if slot == 0 { "CHI" } else { "MOT" });
        assert!(matches!(
            admitted.bind_timeline(&turns),
            Err(WordSpeakerSplitRefusal::Partition(
                SplitRefusal::LeadingSeparator { content_index: 0 }
            ))
        ));
    }
    assert_eq!(cases, 2, "both authored tag-marker alternatives");
}

/// Sidecar separator placement is a distinct wire policy, not lexical identity.
/// The extra endpoint markers are typed API boundary inputs, not authored CHAT.
#[test]
fn timing_partition_retains_leading_interior_and_trailing_sidecar_separators() {
    let chat = fixture(SEPARATORS);
    let mut cases = 0;
    for original in chat.utterances().skip(1).take(2) {
        cases += 1;
        let mut source = with_generated_test_timing(original);
        let mut wor = source.wor_tier().expect("generated sidecar").clone();
        let marker = wor
            .items
            .iter()
            .find(|item| matches!(item, super::WorItem::Separator { .. }))
            .expect("authored tag-marker alternative reaches generated sidecar")
            .clone();
        wor.items.insert(0, marker.clone());
        wor.items.push(marker.clone());
        source
            .dependent_tiers
            .retain(|tier| !matches!(tier.tier, DependentTier::Wor(_)));
        let source = source.add_dependent_tier(DependentTier::Wor(wor));
        let admitted = WordSpeakerSource::admit(&source)
            .expect("separators do not alter complete lexical slot admission");
        let last_root = admitted
            .timing()
            .slots()
            .iter()
            .last()
            .expect("nonempty admitted timing")
            .source_content()
            .index();
        let assignments: Vec<_> = admitted
            .timing()
            .slots()
            .iter()
            .map(|word| usize::from(word.source_content().index() == last_root))
            .collect();
        assert!(assignments.contains(&0) && assignments.contains(&1));
        let before = source.to_chat_string();
        let outcome = UtteranceSplitPlan::for_word_timing(&source, &assignments)
            .expect("boundary before the final whole root")
            .execute();
        let SplitOutcome::Split(split) = outcome else {
            panic!("two distinct source-root affiliations must split");
        };
        assert_eq!(split.children().len(), 2);
        let first = split.children()[0].wor_tier().expect("first child sidecar");
        let last = split.children()[1].wor_tier().expect("last child sidecar");
        assert_eq!(first.items.first(), Some(&marker));
        assert_eq!(last.items.last(), Some(&marker));
        let retained: Vec<_> = split
            .children()
            .iter()
            .flat_map(|child| {
                let admitted = WordSpeakerSource::admit(child)
                    .expect("each child retains its actual root words and complete timing");
                let hull = admitted.timing().hull();
                let bullet = child
                    .main
                    .content
                    .bullet
                    .as_ref()
                    .expect("child timing comes from the attached sidecar");
                assert_eq!(bullet.timing.start_ms, hull.start().get());
                assert_eq!(bullet.timing.end_ms, hull.end().get());
                child.wor_tier().expect("child sidecar").items.iter()
            })
            .collect();
        assert_eq!(
            retained,
            source
                .wor_tier()
                .expect("parent sidecar")
                .items
                .iter()
                .collect::<Vec<_>>()
        );
        assert_eq!(source.to_chat_string(), before);
    }
    assert_eq!(cases, 2, "both authored tag-marker alternatives");
}
