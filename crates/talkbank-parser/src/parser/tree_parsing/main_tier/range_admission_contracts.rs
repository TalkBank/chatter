//! Native carrier contracts, separate from payload validity and public parsing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::TreeSitterParser;
use crate::generated_traversal::{
    AsRawNode, BaseAnnotationsNode, BulletNode, FinalCodesNode, GroupWithAnnotationsNode,
    LinkersNode, MainPhoGroupNode, MainSinGroupNode, NodeSlot, QuotationNode, ReadableSlot,
    ShorteningNode, SlotView, SourceBindingError, SourceBound, SourceBoundKind, UtteranceEndNode,
    WordBodyChoice, WordBodyNode,
};
use crate::parser::typed_cst::{ExtractCanonical, extract_inner};
use std::collections::{BTreeMap, BTreeSet};

fn retained<'tree, T: SourceBoundKind<'tree>>(field: SourceBound<'tree, '_, T>, source: &str) {
    assert_eq!(field.source(), source);
    assert_eq!(
        field.text(),
        source.get(field.raw_node().byte_range()).unwrap()
    );
}

#[test]
fn prelude_and_ending_ranges_retain_payloads_without_rechecking_reads() {
    let parser = TreeSitterParser::new().unwrap();
    let mut observed = [0; 4];
    let mut repeated_codes = 0;
    let mut leading_zero = 0;
    for source in [
        include_str!("../../../../../../corpus/reference/content/linkers-multiple.cha"),
        include_str!("../../../../../../corpus/reference/content/body-ending-controls.cha"),
        include_str!("../../../../../../corpus/reference/content/media-bullets.cha"),
        include_str!(
            "../../../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E748_1.cha"
        ),
    ] {
        let parsed = parser.parse_source_incremental(source, None).unwrap();
        let foreign = parser.parse_source_incremental(source, None).unwrap();
        for node in parsed.root().unwrap().descendants() {
            let node = node.unwrap();
            if let Some(linkers) = node.typed::<LinkersNode>() {
                assert!(matches!(
                    foreign.bind(linkers.raw_node()),
                    Err(SourceBindingError::ForeignTree)
                ));
                let ranges = linkers.extract().admit_ranges().unwrap();
                assert_eq!(ranges.parent().raw_node(), linkers.raw_node());
                assert_eq!(ranges.parent().source(), source);
                assert!(ranges.checked_range_count() >= 2);
                observed[0] += 1;
            }
            if let Some(ending) = node.typed::<UtteranceEndNode>() {
                assert!(matches!(
                    foreign.bind(ending.raw_node()),
                    Err(SourceBindingError::ForeignTree)
                ));
                let ranges = ending.extract_canonical().unwrap().admit_ranges().unwrap();
                assert_eq!(ranges.parent().raw_node(), ending.raw_node());
                assert_eq!(ranges.parent().source(), source);
                if let Some(SlotView::Present(codes)) = ranges
                    .children()
                    .child_1
                    .slot
                    .as_ref()
                    .map(ReadableSlot::view)
                {
                    retained(*codes, source);
                }
                if let Some(SlotView::Present(group)) = ranges
                    .children()
                    .child_2
                    .slot
                    .as_ref()
                    .map(ReadableSlot::view)
                    && let SlotView::Present(bullet) = group.child_1.slot.view()
                {
                    retained(*bullet, source);
                }
                observed[1] += 1;
            }
            if let Some(codes) = node.typed::<FinalCodesNode>() {
                assert!(matches!(
                    foreign.bind(codes.raw_node()),
                    Err(SourceBindingError::ForeignTree)
                ));
                let ranges = codes.extract_canonical().unwrap().admit_ranges().unwrap();
                let fields = ranges.children();
                let checks = ranges.checked_range_count();
                assert_eq!(ranges.parent().raw_node(), codes.raw_node());
                macro_rules! postcode {
                    ($slot:expr) => {
                        if let SlotView::Present(group) = $slot.view()
                            && let SlotView::Present(code) = group.child_1.slot.view()
                        {
                            retained(*code, source);
                        }
                    };
                }
                postcode!(&fields.child_0.slot);
                for next in &fields.child_1.slot {
                    postcode!(&next.slot);
                    repeated_codes += 1;
                }
                assert_eq!(ranges.checked_range_count(), checks);
                observed[2] += 1;
            }
            if let Some(bullet) = node.typed::<BulletNode>() {
                assert!(matches!(
                    foreign.bind(bullet.raw_node()),
                    Err(SourceBindingError::ForeignTree)
                ));
                let ranges = bullet.extract().admit_ranges().unwrap();
                // The five selected token fields have no extras in these
                // specimens, including the syntactically clean E748 token.
                assert_eq!(ranges.checked_range_count(), 5);
                assert_eq!(ranges.parent().raw_node(), bullet.raw_node());
                let SlotView::Present(start) = ranges.children().start_time.slot.view() else {
                    panic!("authored start")
                };
                let SlotView::Present(end) = ranges.children().end_time.slot.view() else {
                    panic!("authored end")
                };
                for _ in 0..100 {
                    retained(*start, source);
                    retained(*end, source);
                    assert_eq!(ranges.checked_range_count(), 5);
                }
                if start.text() == "012" {
                    leading_zero += 1;
                    let errors = crate::error::ErrorCollector::new();
                    assert!(
                        crate::parser::tree_parsing::media_bullet::parse_bullet_node(
                            bullet, &errors
                        )
                        .is_ok()
                    );
                    assert!(
                        !errors.to_vec().is_empty(),
                        "readability does not admit the timestamp spelling"
                    );
                }
                observed[3] += 1;
            }
        }
    }
    assert!(
        observed.iter().all(|count| *count > 0),
        "all producer families: {observed:?}"
    );
    assert!(repeated_codes > 0);
    assert_eq!(leading_zero, 1, "the authored E748 source remains verbatim");
}

#[test]
fn bracketed_ranges_retain_required_presence_and_delimiter_extents() {
    let parser = TreeSitterParser::new().unwrap();
    let mut observed = BTreeMap::new();
    for source in [
        include_str!("../../../../../../corpus/reference/annotation/groups-regular.cha"),
        include_str!("../../../../../../corpus/reference/annotation/groups-phonological.cha"),
        include_str!("../../../../../../corpus/reference/annotation/groups-sign.cha"),
        include_str!("../../../../../../corpus/reference/content/quotation-groups-smart.cha"),
        include_str!("../../../../../../corpus/reference/content/events-in-groups.cha"),
        include_str!(
            "../../../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E342_empty_scoped_content_1.cha"
        ),
    ] {
        let parsed = parser.parse_source_incremental(source, None).unwrap();
        let foreign = parser.parse_source_incremental(source, None).unwrap();
        for node in parsed.root().unwrap().descendants() {
            let node = node.unwrap();
            macro_rules! bracketed {
                ($wrapper:ident, $contents:ident $(, $annotations:ident)?) => {
                    if let Some(typed) = node.typed::<$wrapper>() {
                        assert!(matches!(foreign.bind(typed.raw_node()), Err(SourceBindingError::ForeignTree)));
                        let selected = extract_inner(typed).unwrap();
                        let raw = selected.children();
                        let open_missing = matches!(raw.child_0.slot(), NodeSlot::Missing(_));
                        let close_missing = matches!(raw.child_2.slot(), NodeSlot::Missing(_));
                        let open = raw.child_0.slot().known_or_placeholder().present_or_placeholder().unwrap().raw_node();
                        let close = raw.child_2.slot().known_or_placeholder().present_or_placeholder().unwrap().raw_node();
                        let expected = 3 + raw.unexpected.len() + raw.trailing_extras.len()
                            + raw.child_0.leading_extras().len()
                            + raw.$contents.leading_extras().len()
                            + raw.child_2.leading_extras().len()
                            $(+ 1 + raw.$annotations.leading_extras().len())?;
                        let admitted = selected.admit_ranges().unwrap();
                        assert_eq!(admitted.checked_range_count(), expected);
                        assert_eq!(admitted.parent().raw_node(), typed.raw_node());
                        assert_eq!(admitted.parent().source(), source);
                        let fields = admitted.children();
                        // These matches are irrefutable only while BOTH kind and
                        // required-presence proofs survive the range transition.
                        let SlotView::Present(contents) = fields.$contents.slot.view();
                        retained(*contents, source);
                        $(let SlotView::Present(annotations) = fields.$annotations.slot.view();
                        retained(*annotations, source);)?
                        assert_eq!(open_missing, matches!(fields.child_0.slot, ReadableSlot::Missing(_)));
                        assert_eq!(close_missing, matches!(fields.child_2.slot, ReadableSlot::Missing(_)));
                        let after_open = match fields.child_0.slot.view() {
                            SlotView::Present(value) => value.raw_node(),
                            SlotView::Missing(value) => value.raw_node(),
                        };
                        let after_close = match fields.child_2.slot.view() {
                            SlotView::Present(value) => value.raw_node(),
                            SlotView::Missing(value) => value.raw_node(),
                        };
                        assert_eq!(after_open, open);
                        assert_eq!(after_close, close);
                        for _ in 0..100 {
                            retained(*contents, source);
                            assert_eq!(admitted.checked_range_count(), expected);
                        }
                        *observed.entry(stringify!($wrapper)).or_insert(0) += 1;
                    }
                };
            }
            bracketed!(GroupWithAnnotationsNode, content_2, annotations);
            bracketed!(MainPhoGroupNode, child_1);
            bracketed!(MainSinGroupNode, child_1);
            bracketed!(QuotationNode, child_1);
        }
    }
    assert_eq!(observed.len(), 4, "all bracketed producers: {observed:?}");
}

#[test]
fn word_and_annotation_ranges_retain_nested_choices_and_native_recovery() {
    let parser = TreeSitterParser::new().unwrap();
    let mut bodies = BTreeSet::new();
    let mut shortenings = 0;
    let mut annotation_runs = 0;
    let mut recovery_ranges = 0;
    for source in [
        include_str!("../../../../../../corpus/reference/ca/word-boundary-combinations.cha"),
        include_str!("../../../../../../corpus/reference/content/words-prosody.cha"),
        include_str!("../../../../../../corpus/reference/content/shortenings-in-words.cha"),
        include_str!("../../../../../../corpus/reference/annotation/overlap-markers.cha"),
        include_str!("../../../../../../corpus/reference/annotation/errors-and-replacements.cha"),
        include_str!("../../../../../../corpus/reference/languages/code-switch-spans.cha"),
        include_str!(
            "../../../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E244_word_recovery_5.cha"
        ),
    ] {
        let parsed = parser.parse_source_incremental(source, None).unwrap();
        let foreign = parser.parse_source_incremental(source, None).unwrap();
        for node in parsed.root().unwrap().descendants() {
            let node = node.unwrap();
            if let Some(body) = node.typed::<WordBodyNode>() {
                assert!(matches!(
                    foreign.bind(body.raw_node()),
                    Err(SourceBindingError::ForeignTree)
                ));
                let ranged = body.extract_canonical().unwrap().admit_ranges().unwrap();
                let checks = ranged.checked_range_count();
                assert!(checks > 0);
                for _ in 0..100 {
                    if let SlotView::Present(choice) = ranged.children().content.slot.view() {
                        match choice {
                            WordBodyChoice::WordSegment(_) => {
                                bodies.insert("segment");
                            }
                            WordBodyChoice::OverlapPoint(_) => {
                                bodies.insert("marker");
                            }
                        }
                    }
                    assert_eq!(ranged.parent().raw_node(), body.raw_node());
                    assert_eq!(ranged.parent().source(), source);
                    assert_eq!(ranged.checked_range_count(), checks);
                }
            }
            if let Some(shortening) = node.typed::<ShorteningNode>() {
                assert!(matches!(
                    foreign.bind(shortening.raw_node()),
                    Err(SourceBindingError::ForeignTree)
                ));
                let selected = shortening.extract_canonical().unwrap();
                let before = selected.children().clone();
                let ranged = selected.admit_ranges().unwrap();
                let fields = ranged.children();
                assert_eq!(fields.unexpected.len(), before.unexpected.len());
                for (raw, recovery) in before.unexpected.iter().zip(&fields.unexpected) {
                    assert_eq!(recovery.raw_node(), *raw);
                    assert_eq!(recovery.source(), source);
                    assert_eq!(recovery.text(), source.get(raw.byte_range()).unwrap());
                    recovery_ranges += 1;
                }
                macro_rules! position {
                    ($field:ident) => {
                        match (before.$field.slot().view(), fields.$field.slot.view()) {
                            (SlotView::Present(raw), SlotView::Present(bound)) => {
                                assert_eq!(raw.raw_node(), bound.raw_node());
                                retained(*bound, source);
                            }
                            (SlotView::Missing(raw), SlotView::Missing(bound)) => {
                                assert_eq!(raw, bound.raw_node());
                                retained(bound, source);
                                recovery_ranges += 1;
                            }
                            (SlotView::Error(raw), SlotView::Error(bound)) => {
                                assert_eq!(raw, bound.raw_node());
                                assert_eq!(bound.source(), source);
                                assert_eq!(bound.text(), source.get(raw.byte_range()).unwrap());
                                recovery_ranges += 1;
                            }
                            (SlotView::Absent(_), SlotView::Absent(_)) => {}
                            other => panic!("shortening position changed state: {other:?}"),
                        }
                    };
                }
                position!(child_0);
                position!(child_1);
                position!(child_2);
                shortenings += 1;
            }
            if let Some(annotations) = node.typed::<BaseAnnotationsNode>() {
                assert!(matches!(
                    foreign.bind(annotations.raw_node()),
                    Err(SourceBindingError::ForeignTree)
                ));
                let ranged = annotations.extract().admit_ranges().unwrap();
                assert_eq!(ranged.parent().raw_node(), annotations.raw_node());
                assert_eq!(ranged.parent().text(), annotations.text());
                assert!(ranged.checked_range_count() > 0);
                annotation_runs += 1;
            }
        }
    }
    assert_eq!(bodies.len(), 2, "both grammar alternatives: {bodies:?}");
    assert!(shortenings > 0);
    assert!(annotation_runs > 0);
    assert!(
        recovery_ranges > 0,
        "authored interior-shortening recovery is retained"
    );
}
