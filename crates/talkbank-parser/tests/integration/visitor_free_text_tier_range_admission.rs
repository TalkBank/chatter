//! Native selected-tier range, ownership and counted-work evidence.
//! These are real reference/spec CSTs, not a claim that recovered CHAT is valid.

use super::visitor_word_range_admission::check_retained;
use std::collections::{BTreeMap, BTreeSet};
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{
    AltDependentTierNode, AsRawNode, CohDependentTierNode, CompiledLanguage, DefDependentTierNode,
    EngDependentTierNode, ErrDependentTierNode, FacDependentTierNode, FloDependentTierNode,
    GlsDependentTierNode, ModsylDependentTierNode, NodeSlot, OrtDependentTierNode,
    ParDependentTierNode, PhoalnDependentTierNode, PhosylDependentTierNode, ReadableSlot,
    TimDependentTierNode, UnsupportedDependentTierNode, XDependentTierNode,
    XphointDependentTierNode,
};

#[test]
fn reference_free_text_tier_ranges_retain_fields_and_count_selected_work() {
    // SAFETY: both exports come from the same linked canonical grammar.
    let grammar = unsafe {
        CompiledLanguage::from_bridge(
            tree_sitter_talkbank::LANGUAGE.into(),
            tree_sitter_talkbank::LANGUAGE_METADATA_BRIDGE,
        )
    }
    .admit_generated()
    .unwrap();
    let parser = TreeSitterParser::new().unwrap();
    let mut counts = [0; 17];
    let mut absent = BTreeSet::new();
    let mut recovered = 0;
    let mut anonymous = [0; 2];
    let mut layouts = BTreeMap::<_, BTreeSet<_>>::new();
    for source in [
        include_str!("../../../../corpus/reference/tiers/coding.cha"),
        include_str!("../../../../corpus/reference/tiers/other.cha"),
        include_str!("../../../../corpus/reference/tiers/phon-syllabification.cha"),
        include_str!("../../../../corpus/reference/tiers/phon-intervals.cha"),
        include_str!("../../../../corpus/reference/tiers/user-defined.cha"),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E756_1.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E756_2.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E756_4.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E756_7.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E756_8.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E756_9.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E756_10.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E330_6.cha"
        ),
    ] {
        let parsed = parser.parse_source_incremental(source, None).unwrap();
        let foreign = parser.parse_source_incremental(source, None).unwrap();
        for node in parsed.root().unwrap().descendants() {
            let node = node.unwrap();
            macro_rules! optional_body {
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
                                Some((body.raw_node().byte_range(), false))
                            }
                            Some(NodeSlot::Error(body)) => {
                                recovered += 1;
                                Some((body.byte_range(), true))
                            }
                            Some(NodeSlot::Missing(never)) => match never.0 {},
                            Some(NodeSlot::Unexpected(never) | NodeSlot::Absent(never)) => {
                                match *never {}
                            }
                            None => {
                                absent.insert(stringify!($kind));
                                None
                            }
                        };
                        let admitted = selected.admit_ranges().unwrap();
                        assert_eq!(admitted.checked_range_count(), expected);
                        let fields = admitted.children();
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
                                (Some(ReadableSlot::Present(body)), Some((range, false))) => {
                                    check_retained(*body, source);
                                    assert_eq!(body.raw_node().byte_range(), *range);
                                }
                                (Some(ReadableSlot::Error(body)), Some((range, true))) => {
                                    assert_eq!(body.source(), source);
                                    assert_eq!(body.raw_node().byte_range(), *range);
                                    assert_eq!(body.text(), source.get(range.clone()).unwrap());
                                }
                                (None, None) => {}
                                _ => panic!("range admission must preserve body presence/recovery"),
                            }
                        }
                        assert_eq!(admitted.checked_range_count(), expected);
                    }
                };
            }
            optional_body!(OrtDependentTierNode, 0);
            optional_body!(EngDependentTierNode, 1);
            optional_body!(GlsDependentTierNode, 2);
            optional_body!(AltDependentTierNode, 3);
            optional_body!(CohDependentTierNode, 4);
            optional_body!(DefDependentTierNode, 5);
            optional_body!(ErrDependentTierNode, 6);
            optional_body!(FacDependentTierNode, 7);
            optional_body!(FloDependentTierNode, 8);
            optional_body!(ParDependentTierNode, 9);
            optional_body!(TimDependentTierNode, 10);
            optional_body!(ModsylDependentTierNode, 11);
            optional_body!(PhosylDependentTierNode, 12);
            optional_body!(PhoalnDependentTierNode, 13);
            optional_body!(XphointDependentTierNode, 14);
            optional_body!(XDependentTierNode, 15);

            if let Some(tier) = node.typed::<UnsupportedDependentTierNode>() {
                counts[16] += 1;
                assert!(foreign.bind(tier.raw_node()).is_err());
                let selected = tier.extract_admitted(&grammar).unwrap();
                let raw = selected.children();
                let raw_size = std::mem::size_of_val(raw);
                let range = raw.child_2.range.clone();
                let body = source.get(range.clone()).unwrap();
                anonymous[usize::from(!body.is_empty())] += 1;
                let expected = 5
                    + raw.child_0.leading_extras().len()
                    + raw.child_1.leading_extras().len()
                    + raw.child_3.leading_extras().len()
                    + raw.trailing_extras.len()
                    + raw.unexpected.len();
                let admitted = selected.admit_ranges().unwrap();
                let fields = admitted.children();
                assert_eq!(admitted.checked_range_count(), expected);
                layouts
                    .entry("UnsupportedDependentTierNode")
                    .or_default()
                    .insert((
                        expected,
                        raw_size,
                        std::mem::size_of_val(fields),
                        std::mem::size_of_val(&admitted),
                    ));
                for _ in 0..100 {
                    let ReadableSlot::Present(prefix) = &fields.child_0.slot else {
                        panic!("authored unsupported prefix")
                    };
                    let ReadableSlot::Present(separator) = &fields.child_1.slot else {
                        panic!("authored unsupported separator")
                    };
                    let ReadableSlot::Present(newline) = &fields.child_3.slot else {
                        panic!("authored unsupported newline")
                    };
                    check_retained(*prefix, source);
                    check_retained(*separator, source);
                    check_retained(*newline, source);
                    assert_eq!(fields.child_2.range(), range);
                    assert_eq!(fields.child_2.text(), body);
                    assert_eq!(fields.child_2.parent().source(), source);
                    assert_eq!(fields.child_2.parent().raw_node(), tier.raw_node());
                }
                assert_eq!(admitted.checked_range_count(), expected);
            }
        }
    }
    assert!(
        counts.into_iter().all(|count| count > 0),
        "all selected tier shapes: {counts:?}"
    );
    assert_eq!(
        absent,
        BTreeSet::from([
            "EngDependentTierNode",
            "TimDependentTierNode",
            "XDependentTierNode"
        ])
    );
    assert!(recovered > 0, "authored recovery body remains represented");
    assert!(
        anonymous.into_iter().all(|count| count > 0),
        "empty/present anonymous bodies: {anonymous:?}"
    );
    // Native inline layouts exclude heap allocations and peak memory; no speed claim.
    eprintln!(
        "FREE TEXT TIER RANGE COST counts={counts:?} absent={absent:?} recovered={recovered} anonymous={anonymous:?} (checks,raw,ranged,owner)={layouts:?}"
    );
}
