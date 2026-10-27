//! Authored marker semantics and source geometry through both public backends.

use talkbank_model::model::{
    OverlapPointKind, UtteranceContent, WordContent, WordStressMarkerType,
};
use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome, Span};

fn check_authored_stress_and_overlap(parser: &impl ChatParser) {
    let reference = include_str!(
        "../../../../corpus/reference/word-features/segment-metadata-combinations.cha"
    );
    let word_source = reference
        .split_whitespace()
        .find(|text| *text == "ˈriˌver")
        .unwrap();
    let overlap_source = include_str!("../../../../corpus/reference/ca/overlaps.cha");
    for origin in [0, 200] {
        let errors = ErrorCollector::new();
        let ParseOutcome::Parsed(word) = parser.parse_word(word_source, origin, &errors) else {
            panic!("authored stress word must parse");
        };
        assert!(errors.to_vec().is_empty(), "{:?}", errors.to_vec());
        let stress: Vec<_> = word
            .content()
            .iter()
            .filter_map(|content| match content {
                WordContent::StressMarker(marker) => {
                    Some((marker.marker_type, marker.location.span().unwrap()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            stress,
            [
                (WordStressMarkerType::Primary, Span::new(origin, origin + 2)),
                (
                    WordStressMarkerType::Secondary,
                    Span::new(origin + 4, origin + 6)
                ),
            ]
        );

        let errors = ErrorCollector::new();
        let ParseOutcome::Parsed(file) = parser.parse_chat_file(overlap_source, origin, &errors)
        else {
            panic!("authored overlaps must parse");
        };
        assert!(errors.to_vec().is_empty(), "{:?}", errors.to_vec());
        let mut observed = Vec::new();
        for utterance in file.utterances() {
            for content in &utterance.main.content.content {
                match content {
                    UtteranceContent::OverlapPoint(point) => observed.push(point),
                    UtteranceContent::Word(word) => {
                        observed.extend(word.content().iter().filter_map(|item| match item {
                            WordContent::OverlapPoint(point) => Some(point),
                            _ => None,
                        }));
                    }
                    _ => {}
                }
            }
        }
        let expected: Vec<_> = overlap_source
            .char_indices()
            .filter_map(|(start, symbol)| {
                let kind = match symbol {
                    '⌈' => OverlapPointKind::TopOverlapBegin,
                    '⌉' => OverlapPointKind::TopOverlapEnd,
                    '⌊' => OverlapPointKind::BottomOverlapBegin,
                    '⌋' => OverlapPointKind::BottomOverlapEnd,
                    _ => return None,
                };
                let end = start + symbol.len_utf8();
                let index = overlap_source[end..]
                    .chars()
                    .next()
                    .and_then(|ch| ch.to_digit(10));
                Some((
                    kind,
                    index,
                    Span::new(origin + start, origin + end + usize::from(index.is_some())),
                ))
            })
            .collect();
        assert_eq!(observed.len(), 12);
        assert_eq!(
            observed
                .iter()
                .map(|point| (
                    point.kind,
                    point.index.map(|index| index.get()),
                    point.location.span().unwrap()
                ))
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn authored_stress_and_overlap_kinds_retain_indices_and_locations() {
    check_authored_stress_and_overlap(&talkbank_parser::TreeSitterParser::new().unwrap());
    check_authored_stress_and_overlap(&talkbank_parser_re2c::Re2cParser::new());
}
