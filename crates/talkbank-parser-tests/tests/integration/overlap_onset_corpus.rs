//! Observed marker positions and estimated onsets are different facts.
//! Expected values describe authored reference CHAT, never fabricated ASTs.

use talkbank_model::ErrorCollector;
use talkbank_model::alignment::helpers::{OverlapRegionKind, extract_overlap_info};
use talkbank_model::model::TranscriptName;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;

enum Region {
    Absent,
    Present {
        kind: OverlapRegionKind,
        begin: Option<usize>,
        end: Option<usize>,
    },
}

struct Expected {
    words: usize,
    region: Region,
    estimated_onset_ms: Option<u64>,
}

#[test]
fn reference_indexed_overlaps_keep_kind_index_and_word_positions() {
    use talkbank_model::alignment::helpers::{ContentItem, walk_content};
    use talkbank_model::model::{OverlapPoint, OverlapPointKind, WordContent, WriteChat};

    let source = std::fs::read_to_string(workspace_root().join("corpus/reference/ca/overlaps.cha"))
        .expect("canonical indexed overlaps");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    use OverlapRegionKind::{Bottom, Top};
    let expected = [
        (2, vec![(Top, None, 0, 1), (Top, Some(2), 1, 2)]),
        (1, vec![(Bottom, None, 0, 1)]),
        (1, vec![(Bottom, Some(2), 0, 1)]),
        (4, vec![(Top, None, 0, 2)]),
        (4, vec![(Bottom, None, 2, 4)]),
    ];
    assert_eq!(file.utterances().count(), expected.len());
    let mut witnessed = std::collections::BTreeSet::new();
    let mut index_forms = [0; 2];
    let mut placements = [0; 2];
    let mut check_point = |point: &OverlapPoint| {
        let (glyph, opening) = match point.kind {
            OverlapPointKind::TopOverlapBegin => ("⌈", true),
            OverlapPointKind::TopOverlapEnd => ("⌉", false),
            OverlapPointKind::BottomOverlapBegin => ("⌊", true),
            OverlapPointKind::BottomOverlapEnd => ("⌋", false),
        };
        assert_eq!(point.is_opening(), opening);
        assert_eq!(point.is_closing(), !opening);
        let index = point.index().map(|index| index.get());
        assert!(matches!(index, None | Some(2)), "authored index vocabulary");
        index_forms[usize::from(index.is_some())] += 1;
        let expected = format!(
            "{glyph}{}",
            index.map(|value| value.to_string()).unwrap_or_default()
        );
        assert_eq!(point.to_string(), expected);
        assert_eq!(point.to_chat_string(), expected);
        let span = point.span.expect("parsed marker retains its source span");
        assert_eq!(source.get(span.to_range()), Some(expected.as_str()));
        witnessed.insert(glyph);
    };
    for (utterance, (words, regions)) in file.utterances().zip(expected) {
        walk_content(
            &utterance.main.content.content,
            None,
            &mut |item| match item {
                ContentItem::OverlapPoint(point) => {
                    check_point(point);
                    placements[0] += 1;
                }
                ContentItem::Word(word) => {
                    for content in word.content() {
                        if let WordContent::OverlapPoint(point) = content {
                            check_point(point);
                            placements[1] += 1;
                        }
                    }
                }
                _ => {} // This authored control has standalone and intra-word markers.
            },
        );
        let info = extract_overlap_info(&utterance.main.content.content);
        assert_eq!(info.total_words, words);
        assert_eq!(info.regions.len(), regions.len());
        for (actual, (kind, index, begin, end)) in info.regions.iter().zip(regions) {
            assert_eq!(actual.kind, kind);
            assert_eq!(
                actual.index,
                index.map(talkbank_model::model::OverlapIndex::new)
            );
            assert_eq!(actual.begin_at_word, Some(begin));
            assert_eq!(actual.end_at_word, Some(end));
            assert!(actual.is_well_paired());
        }
    }
    assert_eq!(
        witnessed,
        std::collections::BTreeSet::from(["⌈", "⌉", "⌊", "⌋"])
    );
    assert!(index_forms.iter().all(|count| *count > 0));
    assert_eq!(
        placements,
        [10, 2],
        "standalone and intra-word authored markers"
    );
}

#[test]
fn reference_overlap_onsets_preserve_unpaired_and_wordless_observations() {
    let source =
        std::fs::read_to_string(workspace_root().join("corpus/reference/ca/overlap-onsets.cha"))
            .expect("authored overlap reference");
    let parser = TreeSitterParser::new().expect("parser");
    let mut file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let errors = ErrorCollector::new();
    file.validate_with_alignment(&errors, TranscriptName::Anonymous);
    assert!(
        errors.is_empty(),
        "reference validates: {:?}",
        errors.to_vec()
    );
    let expected = [
        Expected {
            words: 2,
            region: Region::Absent,
            estimated_onset_ms: None,
        },
        Expected {
            words: 5,
            region: Region::Present {
                kind: OverlapRegionKind::Top,
                begin: Some(3),
                end: Some(5),
            },
            estimated_onset_ms: Some(1600),
        },
        Expected {
            words: 1,
            region: Region::Present {
                kind: OverlapRegionKind::Bottom,
                begin: Some(0),
                end: Some(1),
            },
            estimated_onset_ms: None,
        },
        Expected {
            words: 1,
            region: Region::Present {
                kind: OverlapRegionKind::Top,
                begin: Some(0),
                end: None,
            },
            estimated_onset_ms: Some(2000),
        },
        Expected {
            words: 1,
            region: Region::Present {
                kind: OverlapRegionKind::Bottom,
                begin: Some(0),
                end: None,
            },
            estimated_onset_ms: None,
        },
        Expected {
            words: 1,
            region: Region::Present {
                kind: OverlapRegionKind::Top,
                begin: None,
                end: Some(1),
            },
            estimated_onset_ms: None,
        },
        Expected {
            words: 1,
            region: Region::Present {
                kind: OverlapRegionKind::Bottom,
                begin: None,
                end: Some(1),
            },
            estimated_onset_ms: None,
        },
        Expected {
            words: 0,
            region: Region::Present {
                kind: OverlapRegionKind::Top,
                begin: Some(0),
                end: Some(0),
            },
            estimated_onset_ms: None,
        },
        Expected {
            words: 1,
            region: Region::Present {
                kind: OverlapRegionKind::Top,
                begin: Some(1),
                end: None,
            },
            estimated_onset_ms: Some(6000),
        },
        Expected {
            words: 2,
            region: Region::Present {
                kind: OverlapRegionKind::Top,
                begin: Some(0),
                end: Some(1),
            },
            estimated_onset_ms: Some(6000),
        },
    ];
    assert_eq!(file.utterances().count(), expected.len());
    for (utterance, expected) in file.utterances().zip(expected) {
        let main = &utterance.main;
        let info = extract_overlap_info(&main.content.content);
        assert_eq!(info.total_words, expected.words);
        assert_eq!(info.total_words, main.wor_projection().slot_count().get());
        let bullet = main.content.bullet.as_ref().expect("reference timing");
        assert_eq!(
            info.estimate_onset_ms(bullet.timing.start_ms, bullet.timing.end_ms),
            expected.estimated_onset_ms
        );
        match expected.region {
            Region::Absent => {
                assert!(!info.has_any_markers());
                assert!(!info.has_top_overlap() && !info.has_bottom_overlap());
                assert_eq!(info.top_regions().count(), 0);
                assert_eq!(info.bottom_regions().count(), 0);
                assert_eq!(info.top_onset_fraction(), None);
            }
            Region::Present { kind, begin, end } => {
                assert!(info.has_any_markers());
                let [region] = info.regions.as_slice() else {
                    panic!("one region expected");
                };
                assert_eq!(region.kind, kind);
                assert_eq!(region.begin_at_word, begin);
                assert_eq!(region.end_at_word, end);
                assert_eq!(region.is_well_paired(), begin.is_some() && end.is_some());
                assert_eq!(
                    info.has_top_overlap(),
                    kind == OverlapRegionKind::Top && begin.is_some()
                );
                assert_eq!(
                    info.has_bottom_overlap(),
                    kind == OverlapRegionKind::Bottom && begin.is_some()
                );
                assert_eq!(
                    info.top_regions().count(),
                    usize::from(kind == OverlapRegionKind::Top)
                );
                assert_eq!(
                    info.bottom_regions().count(),
                    usize::from(kind == OverlapRegionKind::Bottom)
                );
            }
        }
    }
}
