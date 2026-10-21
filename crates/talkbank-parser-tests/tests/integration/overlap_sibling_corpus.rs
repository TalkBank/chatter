//! Authored indexed siblings travel through checked source admission and analysis.

#![allow(clippy::expect_used)]

use talkbank_model::alignment::helpers::analyze_file_overlaps;
use talkbank_model::model::TranscriptName;
use talkbank_model::{ErrorCollector, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::parse_source_with_parser;

const SOURCE: &str = include_str!("../../../../corpus/reference/ca/overlap-siblings.cha");

#[test]
fn authored_overlap_siblings_distribute_and_reuse_indices_by_turn() {
    let source = SOURCE.to_owned();
    let parser = TreeSitterParser::new().expect("grammar");
    let errors = ErrorCollector::new();
    let admitted = parse_source_with_parser(&parser, &source)
        .admit(
            TranscriptName::for_path(std::path::Path::new("overlap-siblings.cha")),
            &errors,
        )
        .expect("authored indexed siblings are valid");
    assert!(errors.is_empty(), "{:?}", errors.to_vec());
    let owned = admitted.into_owned();
    drop(source);
    let valid = owned.into_valid_file();
    assert!(valid.diagnostics().is_empty());
    assert_eq!(valid.to_chat_string(), SOURCE);

    let document = valid.document();
    let analysis = analyze_file_overlaps(&document.lines);
    assert!(analysis.orphaned_tops.is_empty());
    assert!(analysis.orphaned_bottoms.is_empty());
    assert_eq!(analysis.group_count(), 3);
    assert_eq!(analysis.total_bottoms(), 5);
    assert_eq!(analysis.timed_groups().count(), 3);
    // Independent source-order expectations protect the grouping policy from
    // accidental acceptance of a snapshot made from collapsed siblings.
    let bottom_origins: Vec<Vec<_>> = analysis
        .groups
        .iter()
        .map(|group| group.bottoms.iter().map(|b| b.utterance_index).collect())
        .collect();
    assert_eq!(bottom_origins, [vec![1], vec![1, 2, 3], vec![5]]);
    assert_eq!(analysis.groups[0].top.utterance_index, 0);
    assert_eq!(analysis.groups[1].top.utterance_index, 0);
    assert_eq!(analysis.groups[2].top.utterance_index, 4);
    assert!(analysis.groups[1].bottoms[1].bullet.is_none());
    for group in &analysis.groups {
        for anchor in std::iter::once(&group.top).chain(group.bottoms.iter()) {
            let origin = document
                .utterances()
                .nth(anchor.utterance_index)
                .expect("producer-retained origin");
            assert_eq!(anchor.utterance_location(), origin.main.location);
            assert_eq!(anchor.speaker, origin.main.speaker);
        }
    }
    let groups: Vec<_> = analysis
        .groups
        .iter()
        .map(|group| {
            let anchor = |a: &talkbank_model::alignment::helpers::OverlapAnchor| {
                serde_json::json!({
                    "utterance": a.utterance_index,
                    "speaker": a.speaker.as_str(),
                    "kind": format!("{:?}", a.region.kind),
                    "index": a.region.index.map(|index| index.get()),
                    "begin_at_word": a.region.begin_at_word,
                    "end_at_word": a.region.end_at_word,
                    "bullet": a.bullet,
                })
            };
            serde_json::json!({
                "top": anchor(&group.top),
                "bottoms": group.bottoms.iter().map(anchor).collect::<Vec<_>>(),
            })
        })
        .collect();
    insta::assert_json_snapshot!("authored_overlap_sibling_groups", groups);
}
