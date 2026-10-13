//! Count the selected tier/item graph's work and transient native layouts.
//! Real reference carriers retain kind/source ownership, not whole-tree validity.

use super::visitor_word_range_admission::check_retained;
use std::collections::BTreeSet;
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{
    AsRawNode, CompiledLanguage, MorContentNode, MorContentsChild0Choice, MorContentsNode,
    MorDependentTierNode, MorPostCliticNode, NodeSlot, ReadableSlot,
};

#[test]
fn reference_mor_tier_ranges_retain_fields_and_count_selected_work() {
    // SAFETY: both static exports use the linked grammar's parser header,
    // matching production compiled-kind admission.
    let grammar = unsafe {
        CompiledLanguage::from_bridge(
            tree_sitter_talkbank::LANGUAGE.into(),
            tree_sitter_talkbank::LANGUAGE_METADATA_BRIDGE,
        )
    }
    .admit_generated()
    .unwrap();
    let parser = TreeSitterParser::new().unwrap();
    let mut counts = [0; 5];
    let mut tier_costs = BTreeSet::new();
    let mut body_costs = BTreeSet::new();
    let mut item_costs = BTreeSet::new();
    let mut clitic_costs = BTreeSet::new();
    for source in [
        include_str!("../../../../corpus/reference/tiers/mor-gra.cha"),
        include_str!("../../../../corpus/reference/edge-cases/clitics-and-compounds.cha"),
        include_str!("../../../../corpus/reference/tiers/mor-terminator-only.cha"),
        include_str!("../../../../corpus/reference/tiers/mor-quoted-terminator.cha"),
    ] {
        let parsed = parser.parse_source_incremental(source, None).unwrap();
        let foreign = parser.parse_source_incremental(source, None).unwrap();
        let mut source_tiers = 0;
        for node in parsed.root().unwrap().descendants() {
            let node = node.unwrap();
            if let Some(tier) = node.typed::<MorDependentTierNode>() {
                source_tiers += 1;
                counts[0] += 1;
                assert!(foreign.bind(tier.raw_node()).is_err());
                let selected = tier.extract_admitted(&grammar).unwrap();
                let raw = selected.children();
                assert!(raw.unexpected.is_empty() && raw.trailing_extras.is_empty());
                let raw_size = std::mem::size_of_val(raw);
                let admitted = selected.admit_ranges().unwrap();
                let fields = admitted.children();
                assert_eq!(admitted.checked_range_count(), 4);
                tier_costs.insert((
                    raw_size,
                    std::mem::size_of_val(fields),
                    std::mem::size_of_val(&admitted),
                ));
                for _ in 0..100 {
                    let ReadableSlot::Present(prefix) = &fields.child_0.slot else {
                        panic!("reference prefix")
                    };
                    let ReadableSlot::Present(separator) = &fields.child_1.slot else {
                        panic!("reference separator")
                    };
                    let ReadableSlot::Present(body) = &fields.child_2.slot else {
                        panic!("reference body")
                    };
                    let ReadableSlot::Present(newline) = &fields.child_3.slot else {
                        panic!("reference newline")
                    };
                    check_retained(*prefix, source);
                    check_retained(*separator, source);
                    check_retained(*body, source);
                    check_retained(*newline, source);
                }
                assert_eq!(admitted.checked_range_count(), 4);
            }
            if let Some(body) = node.typed::<MorContentsNode>() {
                counts[1] += 1;
                assert!(foreign.bind(body.raw_node()).is_err());
                let selected = body.extract_admitted(&grammar).unwrap();
                let raw = selected.children();
                assert!(raw.unexpected.is_empty() && raw.trailing_extras.is_empty());
                assert!(
                    raw.child_1.slot().is_none(),
                    "these references have no trailing body whitespace"
                );
                let raw_size = std::mem::size_of_val(raw);
                let (checks, raw_element) = match raw.child_0.slot() {
                    NodeSlot::Present(MorContentsChild0Choice::MorContent(items)) => {
                        assert!(items.child_2.slot().is_some());
                        (
                            2 * (1 + items.child_1.slot().len()) + 1,
                            items.child_1.slot().first().map(std::mem::size_of_val),
                        )
                    }
                    NodeSlot::Present(MorContentsChild0Choice::BreakForCoding(_)) => (1, None),
                    _ => panic!("reference body choice"),
                };
                let admitted = selected.admit_ranges().unwrap();
                let fields = admitted.children();
                let ReadableSlot::Present(choice) = &fields.child_0.slot else {
                    panic!("reference body")
                };
                let ranged_element = match choice {
                    MorContentsChild0Choice::MorContent(items) => {
                        items.child_1.slot.first().map(std::mem::size_of_val)
                    }
                    MorContentsChild0Choice::BreakForCoding(_) => {
                        counts[4] += 1;
                        None
                    }
                };
                body_costs.insert((
                    checks,
                    raw_size,
                    std::mem::size_of_val(fields),
                    std::mem::size_of_val(&admitted),
                    raw_element,
                    ranged_element,
                ));
                assert_eq!(admitted.checked_range_count(), checks);
                for _ in 0..100 {
                    if let MorContentsChild0Choice::MorContent(items) = choice {
                        let ReadableSlot::Present(item) = &items.child_0.slot else {
                            panic!("reference first item")
                        };
                        check_retained(*item, source);
                        for pair in &items.child_1.slot {
                            let ReadableSlot::Present(pair) = &pair.slot else {
                                panic!("reference repeated pair")
                            };
                            let ReadableSlot::Present(space) = &pair.child_0.slot else {
                                panic!("reference item space")
                            };
                            let ReadableSlot::Present(item) = &pair.child_1.slot else {
                                panic!("reference repeated item")
                            };
                            check_retained(*space, source);
                            check_retained(*item, source);
                        }
                    }
                }
                assert_eq!(admitted.checked_range_count(), checks);
            }
            if let Some(item) = node.typed::<MorContentNode>() {
                counts[2] += 1;
                assert!(foreign.bind(item.raw_node()).is_err());
                let selected = item.extract_admitted(&grammar).unwrap();
                let raw = selected.children();
                assert!(raw.unexpected.is_empty() && raw.trailing_extras.is_empty());
                let checks = 1 + raw.post_clitics.slot().len();
                let raw_size = std::mem::size_of_val(raw);
                let raw_element = raw.post_clitics.slot().first().map(std::mem::size_of_val);
                let admitted = selected.admit_ranges().unwrap();
                let fields = admitted.children();
                item_costs.insert((
                    checks,
                    raw_size,
                    std::mem::size_of_val(fields),
                    std::mem::size_of_val(&admitted),
                    raw_element,
                    fields.post_clitics.slot.first().map(std::mem::size_of_val),
                ));
                for _ in 0..100 {
                    let ReadableSlot::Present(main) = &fields.main.slot else {
                        panic!("reference main")
                    };
                    check_retained(*main, source);
                    for clitic in &fields.post_clitics.slot {
                        let ReadableSlot::Present(clitic) = &clitic.slot else {
                            panic!("reference clitic")
                        };
                        check_retained(*clitic, source);
                    }
                }
                assert_eq!(admitted.checked_range_count(), checks);
            }
            if let Some(clitic) = node.typed::<MorPostCliticNode>() {
                counts[3] += 1;
                assert!(foreign.bind(clitic.raw_node()).is_err());
                let selected = clitic.extract_admitted(&grammar).unwrap();
                let raw = selected.children();
                assert!(raw.unexpected.is_empty() && raw.trailing_extras.is_empty());
                let raw_size = std::mem::size_of_val(raw);
                let admitted = selected.admit_ranges().unwrap();
                let fields = admitted.children();
                clitic_costs.insert((
                    raw_size,
                    std::mem::size_of_val(fields),
                    std::mem::size_of_val(&admitted),
                ));
                for _ in 0..100 {
                    let ReadableSlot::Present(tilde) = &fields.child_0.slot else {
                        panic!("reference tilde")
                    };
                    let ReadableSlot::Present(word) = &fields.child_1.slot else {
                        panic!("reference clitic word")
                    };
                    check_retained(*tilde, source);
                    check_retained(*word, source);
                }
                assert_eq!(admitted.checked_range_count(), 2);
            }
        }
        assert!(
            source_tiers > 0,
            "every selected reference contributes a tier"
        );
    }
    assert!(
        counts.iter().all(|count| *count > 0),
        "all owner paths: {counts:?}"
    );
    eprintln!(
        "MOR TIER RANGE COST tier(raw,ranged,owner)={tier_costs:?} body(checks,raw,ranged,owner,raw_element,ranged_element)={body_costs:?} item(checks,raw,ranged,owner,raw_element,ranged_element)={item_costs:?} clitic(raw,ranged,owner)={clitic_costs:?} counts(tier,body,item,clitic,bare)={counts:?}"
    );
}
