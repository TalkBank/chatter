//! Selected word fields retain both kind and readable-range proofs. This is a
//! finite carrier-cost contract, not whole-tree admission or a speed claim.

use std::collections::BTreeSet;
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{
    AsRawNode, CompiledLanguage, NodeSlot, ReadableSlot, SourceBound, SourceBoundKind,
    StandaloneWordChild0Choice, StandaloneWordChild2Choice, StandaloneWordNode,
};

pub(super) fn check_retained<'tree, T: SourceBoundKind<'tree>>(
    field: SourceBound<'tree, '_, T>,
    source: &str,
) {
    assert_eq!(field.source(), source);
    assert_eq!(
        field.text(),
        source.get(field.raw_node().byte_range()).unwrap()
    );
}

#[test]
fn reference_word_root_ranges_preserve_fields_and_count_selected_work() {
    // SAFETY: all bridge exports have static lifetime and use the linked
    // grammar's parser header and reviewed engine, as the production owner does.
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
    let mut words = 0;
    let mut positions = [0; 5];
    let mut repeated_forms = 0;
    let mut costs = BTreeSet::new();
    for source in [
        include_str!("../../../../corpus/reference/content/words-markers.cha"),
        include_str!("../../../../corpus/reference/content/words-special-forms.cha"),
        include_str!("../../../../corpus/reference/word/pos-hint-vocabulary.cha"),
        include_str!("../../../../corpus/reference/edge-cases/multi-language.cha"),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E203_1.cha"
        ),
    ] {
        let parsed = parser.parse_source_incremental(source, None).unwrap();
        let foreign = parser.parse_source_incremental(source, None).unwrap();
        let mut fixture_words = 0;
        for node in parsed.root().unwrap().descendants() {
            let Some(word) = node.unwrap().typed::<StandaloneWordNode>() else {
                continue;
            };
            fixture_words += 1;
            assert!(foreign.bind(word.raw_node()).is_err());
            let selected = word.extract_admitted(&grammar).unwrap();
            let raw = selected.children();
            assert!(raw.unexpected.is_empty());
            assert!(raw.trailing_extras.is_empty());
            assert!(raw.child_0.leading_extras().is_empty());
            assert!(raw.child_1.leading_extras().is_empty());
            assert!(raw.child_2.leading_extras().is_empty());
            assert!(raw.child_3.leading_extras().is_empty());
            assert!(raw.child_4.leading_extras().is_empty());
            let present = [
                matches!(raw.child_0.slot(), Some(NodeSlot::Present(_))),
                matches!(raw.child_1.slot(), NodeSlot::Present(_)),
                matches!(raw.child_2.slot(), Some(NodeSlot::Present(_))),
                matches!(raw.child_3.slot(), Some(NodeSlot::Present(_))),
                matches!(raw.child_4.slot(), Some(NodeSlot::Present(_))),
            ];
            assert!(present[1], "these specimens retain a word body");
            for (index, present) in present.iter().enumerate() {
                positions[index] += usize::from(*present);
            }
            let expected_checks = present.into_iter().filter(|present| *present).count();
            let raw_bytes = std::mem::size_of_val(raw);
            let admitted = selected.admit_ranges().unwrap();
            assert_eq!(admitted.checked_range_count(), expected_checks);
            let fields = admitted.children();
            costs.insert((
                expected_checks,
                raw_bytes,
                std::mem::size_of_val(fields),
                std::mem::size_of_val(&admitted),
            ));
            if matches!(
                fields.child_2.slot,
                Some(ReadableSlot::Present(
                    StandaloneWordChild2Choice::RepeatedFormMarker(_)
                ))
            ) {
                repeated_forms += 1;
            }
            for _ in 0..100 {
                if let Some(ReadableSlot::Present(choice)) = &fields.child_0.slot {
                    match choice {
                        StandaloneWordChild0Choice::WordPrefix(prefix) => {
                            check_retained(*prefix, source)
                        }
                        StandaloneWordChild0Choice::Zero(zero) => check_retained(*zero, source),
                    }
                }
                let ReadableSlot::Present(body) = &fields.child_1.slot else {
                    panic!("reference body")
                };
                check_retained(*body, source);
                if let Some(ReadableSlot::Present(choice)) = &fields.child_2.slot {
                    match choice {
                        StandaloneWordChild2Choice::FormMarker(marker) => {
                            check_retained(*marker, source)
                        }
                        StandaloneWordChild2Choice::RepeatedFormMarker(marker) => {
                            check_retained(*marker, source)
                        }
                    }
                }
                if let Some(ReadableSlot::Present(suffix)) = &fields.child_3.slot {
                    check_retained(*suffix, source);
                }
                if let Some(ReadableSlot::Present(tag)) = &fields.child_4.slot {
                    check_retained(*tag, source);
                }
            }
            assert_eq!(admitted.checked_range_count(), expected_checks);
        }
        assert!(
            fixture_words > 0,
            "each authored source must contribute words"
        );
        words += fixture_words;
    }
    assert!(
        positions.iter().all(|count| *count > 0),
        "all five fields: {positions:?}"
    );
    assert_eq!(repeated_forms, 1, "the authored E203 repeated-form carrier");
    eprintln!(
        "WORD ROOT RANGE COST (checks,raw_inline,ranged_inline,owner)={costs:?} words={words} fields={positions:?}"
    );
}
