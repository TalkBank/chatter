//! Count selected MOR word/feature range work on real reference carriers.
//! Readability is not nonempty admission, validity or a speed guarantee.

use super::visitor_word_range_admission::check_retained;
use std::collections::BTreeSet;
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{
    AsRawNode, CompiledLanguage, MorFeatureNode, MorPostCliticNode, MorWordNode, NodeSlot,
    ReadableSlot,
};

#[test]
fn reference_mor_ranges_retain_word_and_feature_fields_with_counted_work() {
    // SAFETY: both exports are static and use the linked grammar's parser
    // header, matching the production compiled-kind admission boundary.
    let grammar = unsafe {
        CompiledLanguage::from_bridge(
            tree_sitter_talkbank::LANGUAGE.into(),
            tree_sitter_talkbank::LANGUAGE_METADATA_BRIDGE,
        )
    }
    .admit_generated()
    .unwrap();
    let parser = TreeSitterParser::new().unwrap();
    let mut words = 0;
    let mut clitic_words = 0;
    let mut features = 0;
    let mut empty_feature_lists = 0;
    let mut word_costs = BTreeSet::new();
    let mut feature_costs = BTreeSet::new();
    for source in [
        include_str!("../../../../corpus/reference/tiers/mor-gra.cha"),
        include_str!("../../../../corpus/reference/edge-cases/clitics-and-compounds.cha"),
    ] {
        let parsed = parser.parse_source_incremental(source, None).unwrap();
        let foreign = parser.parse_source_incremental(source, None).unwrap();
        let mut source_words = 0;
        let mut source_features = 0;
        for node in parsed.root().unwrap().descendants() {
            let node = node.unwrap();
            if let Some(word) = node.typed::<MorWordNode>() {
                source_words += 1;
                if let Some(parent) = word.raw_node().parent()
                    && parsed
                        .bind(parent)
                        .unwrap()
                        .typed::<MorPostCliticNode>()
                        .is_some()
                {
                    clitic_words += 1;
                }
                assert!(foreign.bind(word.raw_node()).is_err());
                let selected = word.extract_admitted(&grammar).unwrap();
                let raw = selected.children();
                assert!(raw.unexpected.is_empty());
                assert!(raw.trailing_extras.is_empty());
                assert!(raw.child_0.leading_extras().is_empty());
                assert!(raw.child_1.leading_extras().is_empty());
                assert!(raw.child_2.leading_extras().is_empty());
                assert!(raw.child_3.leading_extras().is_empty());
                let repeated = raw.child_3.slot();
                for element in repeated {
                    assert!(element.leading_extras().is_empty());
                    assert!(matches!(element.slot(), NodeSlot::Present(_)));
                }
                let feature_count = repeated.len();
                empty_feature_lists += usize::from(feature_count == 0);
                let raw_inline = std::mem::size_of_val(raw);
                let raw_feature_element = repeated.first().map(std::mem::size_of_val);
                let admitted = selected.admit_ranges().unwrap();
                assert_eq!(admitted.checked_range_count(), 3 + feature_count);
                let fields = admitted.children();
                assert_eq!(fields.child_3.slot.len(), feature_count);
                word_costs.insert((
                    3 + feature_count,
                    raw_inline,
                    std::mem::size_of_val(fields),
                    std::mem::size_of_val(&admitted),
                    raw_feature_element,
                    fields.child_3.slot.first().map(std::mem::size_of_val),
                ));
                for _ in 0..100 {
                    let ReadableSlot::Present(pos) = &fields.child_0.slot else {
                        panic!("reference POS")
                    };
                    let ReadableSlot::Present(pipe) = &fields.child_1.slot else {
                        panic!("reference pipe")
                    };
                    let ReadableSlot::Present(lemma) = &fields.child_2.slot else {
                        panic!("reference lemma")
                    };
                    check_retained(*pos, source);
                    check_retained(*pipe, source);
                    check_retained(*lemma, source);
                    for feature in &fields.child_3.slot {
                        let ReadableSlot::Present(feature) = &feature.slot else {
                            panic!("reference feature")
                        };
                        check_retained(*feature, source);
                    }
                }
                assert_eq!(admitted.checked_range_count(), 3 + feature_count);
            }
            if let Some(feature) = node.typed::<MorFeatureNode>() {
                source_features += 1;
                assert!(foreign.bind(feature.raw_node()).is_err());
                let selected = feature.extract();
                let raw = selected.children();
                assert!(raw.unexpected.is_empty());
                assert!(raw.trailing_extras.is_empty());
                assert!(raw.child_0.leading_extras().is_empty());
                assert!(raw.child_1.leading_extras().is_empty());
                let raw_inline = std::mem::size_of_val(raw);
                let admitted = selected.admit_ranges().unwrap();
                assert_eq!(admitted.checked_range_count(), 2);
                let fields = admitted.children();
                feature_costs.insert((
                    raw_inline,
                    std::mem::size_of_val(fields),
                    std::mem::size_of_val(&admitted),
                ));
                for _ in 0..100 {
                    let ReadableSlot::Present(hyphen) = &fields.child_0.slot else {
                        panic!("reference feature hyphen")
                    };
                    let ReadableSlot::Present(value) = &fields.child_1.slot else {
                        panic!("reference feature value")
                    };
                    check_retained(*hyphen, source);
                    check_retained(*value, source);
                }
                assert_eq!(admitted.checked_range_count(), 2);
            }
        }
        assert!(source_words > 0, "each reference contributes MOR words");
        assert!(source_features > 0, "each reference contributes features");
        words += source_words;
        features += source_features;
    }
    assert!(empty_feature_lists > 0 && empty_feature_lists < words);
    assert!(
        clitic_words > 0,
        "the second production word consumer has a witness"
    );
    eprintln!(
        "MOR RANGE COST word(checks,raw_inline,ranged_inline,owner,raw_feature_element,ranged_feature_element)={word_costs:?} feature(raw_inline,ranged_inline,owner)={feature_costs:?} words={words} clitic_words={clitic_words} features={features} empty_feature_lists={empty_feature_lists}"
    );
}
