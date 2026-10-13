//! Private representation and constructed-boundary controls.
use super::content_groups::as_retrace;
use super::*;
use talkbank_model::{ParseValidateOptions, WriteChat};

const WOR: &str = include_str!("../../../../corpus/reference/tiers/wor.cha");
const RETRACE_RUNS: &str =
    include_str!("../../../../corpus/reference/languages/ita-conversation.cha");

fn fixture(source: &str) -> talkbank_model::model::ChatFile {
    crate::parse_and_validate(source, ParseValidateOptions::default().with_validation())
        .expect("admitted reference CHAT")
}

fn split(outcome: SplitOutcome<'_>) -> SplitChildren<'_> {
    match outcome {
        SplitOutcome::Split(split) => split,
        SplitOutcome::Unchanged => panic!("expected a split"),
    }
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
