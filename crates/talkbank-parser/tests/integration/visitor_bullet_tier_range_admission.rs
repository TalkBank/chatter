//! Real selected bullet-tier root range, ownership and counted-work evidence.

use super::visitor_word_range_admission::check_retained;
use std::collections::{BTreeMap, BTreeSet};
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{
    ActDependentTierNode, AddDependentTierNode, AsRawNode, CodDependentTierNode,
    ComDependentTierNode, CompiledLanguage, ExpDependentTierNode, GpxDependentTierNode,
    IntDependentTierNode, NodeSlot, ReadableSlot, SitDependentTierNode, SpaDependentTierNode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyState {
    Present,
    Error,
}

#[test]
fn reference_bullet_tier_ranges_retain_fields_and_count_selected_work() {
    // SAFETY: all bridge exports come from the same statically linked canonical
    // grammar and reviewed engine as the production admission owner.
    let grammar = unsafe {
        CompiledLanguage::from_bridge(
            tree_sitter_talkbank::LANGUAGE.into(),
            tree_sitter_talkbank::LANGUAGE_METADATA_BRIDGE,
            talkbank_parser::generated_traversal::RequiredPresenceBridge {
                create: tree_sitter_talkbank::REQUIRED_GRAPH_NEW,
                destroy: tree_sitter_talkbank::REQUIRED_GRAPH_DROP,
                check: tree_sitter_talkbank::REQUIRED_RULE,
                prefix: tree_sitter_talkbank::REQUIRED_PREFIX,
            },
        )
    }
    .admit_generated()
    .unwrap();
    let parser = TreeSitterParser::new().unwrap();
    let mut counts = [0; 9];
    let mut absent = BTreeSet::new();
    let mut whitespace_present = BTreeSet::new();
    let mut recovered = 0;
    let mut layouts = BTreeMap::<_, BTreeSet<_>>::new();
    for source in [
        include_str!("../../../../corpus/reference/tiers/coding.cha"),
        include_str!("../../../../corpus/reference/tiers/other.cha"),
        include_str!("../../../../corpus/reference/tiers/descriptive.cha"),
        include_str!("../../../../corpus/reference/tiers/comment-media-segments.cha"),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E756_3.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E756_6.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E315_2.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E330_5.cha"
        ),
    ] {
        let parsed = parser.parse_source_incremental(source, None).unwrap();
        let foreign = parser.parse_source_incremental(source, None).unwrap();
        for node in parsed.root().unwrap().descendants() {
            let node = node.unwrap();
            macro_rules! body {
                ($kind:ident, $index:literal) => {
                    if let Some(tier) = node.typed::<$kind>() {
                        counts[$index] += 1;
                        assert!(foreign.bind(tier.raw_node()).is_err());
                        let selected = tier.extract_admitted(&grammar).unwrap();
                        let raw = selected.children();
                        let raw_size = std::mem::size_of_val(raw);
                        let extras = raw.child_0.leading_extras().len()
                            + raw.child_1.leading_extras().len()
                            + raw.child_2.leading_extras().len()
                            + raw.child_3.leading_extras().len()
                            + raw.trailing_extras.len()
                            + raw.unexpected.len();
                        let expected = 3 + usize::from(raw.child_2.slot().is_some()) + extras;
                        let expected_body = match raw.child_2.slot() {
                            Some(NodeSlot::Present(body)) => {
                                Some((body.raw_node().byte_range(), BodyState::Present))
                            }
                            Some(NodeSlot::Error(body)) => {
                                recovered += 1;
                                Some((body.byte_range(), BodyState::Error))
                            }
                            Some(NodeSlot::Missing(missing)) => match missing.0 {},
                            Some(NodeSlot::Unexpected(never) | NodeSlot::Absent(never)) => {
                                match *never {}
                            }
                            None => {
                                absent.insert(stringify!($kind));
                                None
                            }
                        };
                        let admitted = selected.admit_ranges().unwrap();
                        let fields = admitted.children();
                        assert_eq!(admitted.checked_range_count(), expected);
                        layouts.entry(stringify!($kind)).or_default().insert((
                            expected,
                            raw_size,
                            std::mem::size_of_val(fields),
                            std::mem::size_of_val(&admitted),
                        ));
                        for _ in 0..100 {
                            let ReadableSlot::Present(prefix) = &fields.child_0.slot else {
                                panic!("authored tier prefix")
                            };
                            let ReadableSlot::Present(separator) = &fields.child_1.slot else {
                                panic!("authored tier separator")
                            };
                            let ReadableSlot::Present(newline) = &fields.child_3.slot else {
                                panic!("authored tier newline")
                            };
                            check_retained(*prefix, source);
                            check_retained(*separator, source);
                            check_retained(*newline, source);
                            match (&fields.child_2.slot, &expected_body) {
                                (
                                    Some(ReadableSlot::Present(body)),
                                    Some((range, BodyState::Present)),
                                ) => {
                                    check_retained(*body, source);
                                    assert_eq!(body.raw_node().byte_range(), *range);
                                    if body.text().trim().is_empty() {
                                        whitespace_present.insert(stringify!($kind));
                                    }
                                }
                                (Some(ReadableSlot::Missing(missing)), _) => match missing.0 {},
                                (
                                    Some(ReadableSlot::Error(body)),
                                    Some((range, BodyState::Error)),
                                ) => {
                                    assert_eq!(body.source(), source);
                                    assert_eq!(body.raw_node().byte_range(), *range);
                                    assert_eq!(body.text(), source.get(range.clone()).unwrap());
                                }
                                (None, None) => {}
                                _ => {
                                    panic!("range admission must retain body presence and recovery")
                                }
                            }
                        }
                        assert_eq!(admitted.checked_range_count(), expected);
                    }
                };
            }
            body!(ActDependentTierNode, 0);
            body!(AddDependentTierNode, 1);
            body!(CodDependentTierNode, 2);
            body!(ComDependentTierNode, 3);
            body!(ExpDependentTierNode, 4);
            body!(GpxDependentTierNode, 5);
            body!(IntDependentTierNode, 6);
            body!(SitDependentTierNode, 7);
            body!(SpaDependentTierNode, 8);
        }
    }
    // Print the full observed shape inventory before any aggregate assertion.
    // Whitespace-only E756 content is not the grammar's optional absence.
    eprintln!(
        "BULLET TIER RANGE COST counts={counts:?} absent={absent:?} whitespace_present={whitespace_present:?} recovered={recovered} (checks,raw,ranged,owner)={layouts:?}"
    );
    assert!(
        counts.into_iter().all(|count| count > 0),
        "all nine shapes: {counts:?}"
    );
    assert_eq!(absent, BTreeSet::from(["ComDependentTierNode"]));
    assert_eq!(whitespace_present, BTreeSet::from(["AddDependentTierNode"]));
    assert!(recovered > 0, "authored recovered body remains represented");
    // Inline native layouts exclude heap/peak memory and establish no speed claim.
}
