//! Authored metadata and tier policies survive source-bound utterance splitting.

use talkbank_model::model::dependent_tier::wor::WorItem;
use talkbank_model::model::{Bullet, DependentTier, TranscriptName};
use talkbank_model::validation::ValidChatFile;
use talkbank_model::{ErrorCollector, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::parse_source_with_parser;
use talkbank_transform::utterance_split::{
    SplitChildren, SplitOutcome, SplitRefusal, TierInvalidationReason, UtteranceSplitPlan,
};

const SOURCE: &str = include_str!("../../../../corpus/reference/content/split-metadata.cha");

fn fixture() -> ValidChatFile {
    let source = SOURCE.to_owned();
    let parser = TreeSitterParser::new().expect("grammar");
    let errors = ErrorCollector::new();
    let admitted = parse_source_with_parser(&parser, &source)
        .admit(
            TranscriptName::for_path(std::path::Path::new("split-metadata.cha")),
            &errors,
        )
        .expect("authored split metadata is valid");
    assert!(errors.is_empty(), "{:?}", errors.to_vec());
    let owned = admitted.into_owned();
    drop(source);
    assert_eq!(owned.source(), SOURCE);
    let valid = owned.into_valid_file();
    assert!(valid.diagnostics().is_empty());
    assert_eq!(valid.to_chat_string(), SOURCE);
    valid
}

fn split(outcome: SplitOutcome<'_>) -> SplitChildren<'_> {
    match outcome {
        SplitOutcome::Split(children) => children,
        SplitOutcome::Unchanged => panic!("the authored two-word partition splits"),
    }
}

#[test]
fn authored_split_metadata_keeps_scope_and_source_bound_tier_policies() {
    let valid = fixture();
    let source = valid.document().utterances().nth(1).expect("metadata turn");
    let outcome = split(
        UtteranceSplitPlan::for_word_timing(source, &[usize::MAX, 42])
            .expect("two contiguous, source-bounded labels")
            .execute(),
    );
    let children = outcome.children();
    assert_eq!(children.len(), 2);
    assert!(matches!(
        UtteranceSplitPlan::for_morphology(source, &[0, 1]),
        Err(SplitRefusal::SlotCount {
            expected: 3,
            actual: 2
        })
    ));
    let morphology = split(
        UtteranceSplitPlan::for_morphology(source, &[0, 0, 1])
            .expect("morphology assigns the comma's own slot")
            .execute(),
    );
    assert_eq!(
        morphology.children()[0].to_chat_string(),
        children[0].to_chat_string()
    );
    assert_eq!(
        morphology.children()[1].to_chat_string(),
        children[1].to_chat_string()
    );
    assert_eq!(
        children[0].main.content.linkers,
        source.main.content.linkers
    );
    assert!(children[1].main.content.linkers.is_empty());
    assert!(children[0].main.content.postcodes.is_empty());
    assert_eq!(
        children[1].main.content.postcodes,
        source.main.content.postcodes
    );
    assert_eq!(
        children[1].main.content.terminator,
        source.main.content.terminator
    );
    for child in children {
        assert_eq!(
            child.main.content.language_code,
            source.main.content.language_code
        );
        assert_eq!(child.main.span, source.main.span);
        assert_eq!(child.main.speaker_span, source.main.speaker_span);
    }
    let tier_kinds: Vec<Vec<_>> = children
        .iter()
        .map(|child| {
            child
                .dependent_tiers
                .iter()
                .map(|entry| entry.tier.kind())
                .collect()
        })
        .collect();
    assert_eq!(tier_kinds, [vec!["com", "xtra", "act", "wor"], vec!["wor"]]);
    assert_eq!(children[0].main.content.bullet, Some(Bullet::new(100, 300)));
    assert_eq!(children[1].main.content.bullet, Some(Bullet::new(300, 600)));
    let leading_wor = children[0].wor_tier().expect("first child's timing");
    assert!(matches!(
        leading_wor.items.first(),
        Some(WorItem::Separator { .. })
    ));
    let losses = outcome.invalidated_tiers();
    assert_eq!(losses.len(), 4);
    for (index, loss) in losses.iter().enumerate() {
        assert_eq!(loss.index(), index);
        assert_eq!(loss.reason(), TierInvalidationReason::BoundaryDependent);
        assert!(std::ptr::eq(
            loss.tier(),
            &source.dependent_tiers[index].tier
        ));
    }
    assert_eq!(losses[3].tier().kind(), "xcoref");
    let text: Vec<_> = children.iter().map(WriteChat::to_chat_string).collect();
    insta::assert_snapshot!("authored_split_metadata", text.join("\n"));
}

#[test]
fn authored_split_stale_word_timing_has_an_explicit_invalidation_receipt() {
    let valid = fixture();
    // A source edit makes retained timing stale. The edited model is not
    // represented as a ValidChatFile or silently accepted for output.
    let source = valid.document().utterances().nth(1).expect("metadata turn");
    let mut edited = source.clone();
    let DependentTier::Wor(wor) = &mut edited.dependent_tiers[4].tier else {
        panic!("the authored fifth tier is word timing");
    };
    assert!(matches!(wor.items.pop(), Some(WorItem::Word(_))));
    let outcome = split(
        UtteranceSplitPlan::for_word_timing(&edited, &[0, 1])
            .expect("the source still has two word slots")
            .execute(),
    );
    assert_eq!(outcome.children().len(), 2);
    assert!(
        outcome
            .children()
            .iter()
            .all(|child| { child.wor_tier().is_none() && child.main.content.bullet.is_none() })
    );
    let losses = outcome.invalidated_tiers();
    assert_eq!(losses.len(), 5);
    let word_loss = losses.last().expect("stale word-timing receipt");
    assert_eq!(word_loss.index(), 4);
    assert_eq!(
        word_loss.reason(),
        TierInvalidationReason::UncorroboratedWordTiming
    );
    assert!(std::ptr::eq(
        word_loss.tier(),
        &edited.dependent_tiers[4].tier
    ));
    assert_eq!(edited.main.content.bullet, source.main.content.bullet);
    let parent_bullet = edited
        .main
        .content
        .bullet
        .as_ref()
        .expect("parent interval");
    assert_eq!(parent_bullet.timing.start_ms, 100);
    assert_eq!(parent_bullet.timing.end_ms, 600);
    let (children, invalidated) = outcome.into_parts();
    assert_eq!(children.len(), 2);
    assert_eq!(invalidated.len(), 5);
    assert!(std::ptr::eq(
        invalidated[4].tier(),
        &edited.dependent_tiers[4].tier
    ));
}
