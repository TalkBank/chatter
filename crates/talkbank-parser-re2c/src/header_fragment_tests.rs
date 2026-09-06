//! Public parser contract across independent backends and source boundaries.
#![allow(clippy::unwrap_used)]

use talkbank_model::{ChatParser, ErrorCollector};

#[test]
fn header_fragment_requires_the_complete_input() {
    fn rejected(parser: &impl ChatParser, input: &str) {
        let errors = ErrorCollector::new();
        assert!(
            parser.parse_header(input, 200, &errors).is_rejected(),
            "{} accepted only part of {input:?}",
            parser.parser_name()
        );
        assert!(!errors.is_empty());
    }
    let canonical = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = crate::Re2cParser::new();
    for input in [
        "@Languages:\teng\n@Comment:\tsecond header",
        "@Comment:\tfirst header\n@Date:\t01-JAN-2020",
        "@Comment:\tfirst header\n*CHI:\thello .",
        "@Comment:\tfirst header\nunsupported source line",
    ] {
        rejected(&canonical, input);
        rejected(&re2c, input);
    }
}

#[test]
fn header_fragment_preserves_folding_and_line_endings() {
    fn accepted(parser: &impl ChatParser, input: &str) {
        let errors = ErrorCollector::new();
        assert!(
            parser.parse_header(input, 200, &errors).is_parsed(),
            "{input:?}"
        );
        assert!(
            errors.is_empty(),
            "{}: {:?}",
            parser.parser_name(),
            errors.into_vec()
        );
    }
    let canonical = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = crate::Re2cParser::new();
    for input in [
        "@Languages:\teng\n",
        "@Comment:\tcafé\r\n",
        "@Comment:\tfirst line\n\tcontinued content",
    ] {
        accepted(&canonical, input);
        accepted(&re2c, input);
    }
}

#[test]
fn header_fragment_diagnostic_keeps_source_and_document_coordinates() {
    use talkbank_model::{ErrorCode, Span};
    fn check(parser: &impl ChatParser) {
        let input = "@Languages:\teng\n@Comment:\tcafé";
        for offset in [0, 200] {
            let errors = ErrorCollector::new();
            assert!(parser.parse_header(input, offset, &errors).is_rejected());
            let errors = errors.into_vec();
            let error = errors
                .iter()
                .find(|error| error.code == ErrorCode::TierValidationError)
                .unwrap();
            assert_eq!(
                error.location.span,
                Span::from_usize(offset, offset + input.len())
            );
            let context = error.context.as_ref().unwrap();
            assert_eq!(context.source_text, input);
            assert_eq!(context.span, Span::from_usize(0, input.len()));
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&crate::Re2cParser::new());
}
