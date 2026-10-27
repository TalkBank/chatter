//! Registry semantics and source geometry, independently through each backend.

use talkbank_model::model::{CADelimiterType, CAElementType, WordContent};
use talkbank_model::{ChatParser, ErrorCollector, Location, ParseOutcome, Span};

fn check_registry_symbols(parser: &impl ChatParser) {
    for origin in [0, 200] {
        for &kind in CAElementType::ALL {
            let symbol = kind.to_symbol();
            let source = format!("a{symbol}b");
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(word) = parser.parse_word(&source, origin, &errors) else {
                panic!("element {kind:?} must parse");
            };
            assert!(
                errors.to_vec().is_empty(),
                "{kind:?}: {:?}",
                errors.to_vec()
            );
            let elements: Vec<_> = word
                .content()
                .iter()
                .filter_map(|item| match item {
                    WordContent::CAElement(element) => Some(element),
                    _ => None,
                })
                .collect();
            assert_eq!(elements.len(), 1, "{kind:?}");
            assert_eq!(elements[0].element_type, kind);
            assert_eq!(
                elements[0].location,
                Location::Located(Span::new(origin + 1, origin + 1 + symbol.len()))
            );
        }
        for &kind in CADelimiterType::ALL {
            let symbol = kind.to_symbol();
            let source = format!("a{symbol}b{symbol}c");
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(word) = parser.parse_word(&source, origin, &errors) else {
                panic!("delimiter {kind:?} must parse");
            };
            assert!(
                errors.to_vec().is_empty(),
                "{kind:?}: {:?}",
                errors.to_vec()
            );
            let delimiters: Vec<_> = word
                .content()
                .iter()
                .filter_map(|item| match item {
                    WordContent::CADelimiter(delimiter) => Some(delimiter),
                    _ => None,
                })
                .collect();
            assert_eq!(delimiters.len(), 2, "{kind:?}");
            for (delimiter, start) in delimiters.iter().zip([1, 2 + symbol.len()]) {
                assert_eq!(delimiter.delimiter_type, kind);
                assert_eq!(
                    delimiter.location,
                    Location::Located(Span::new(origin + start, origin + start + symbol.len()))
                );
            }
        }
    }
}

#[test]
fn registry_word_symbols_retain_semantics_and_locations() {
    check_registry_symbols(&talkbank_parser::TreeSitterParser::new().unwrap());
    check_registry_symbols(&talkbank_parser_re2c::Re2cParser::new());
}
