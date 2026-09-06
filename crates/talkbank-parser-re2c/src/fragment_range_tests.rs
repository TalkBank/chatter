//! Public offset boundary policy, independent of parser implementation.
#![allow(clippy::unwrap_used)]
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, Span};

#[test]
fn fragment_ranges_support_the_full_unsigned_coordinate_space() {
    fn check(parser: &impl ChatParser) {
        let input = "café";
        for offset in [0, i32::MAX as usize + 1, u32::MAX as usize - input.len()] {
            let errors = ErrorCollector::new();
            let word = parser
                .parse_word(input, offset, &errors)
                .into_option()
                .unwrap();
            assert_eq!(
                word.span,
                Span::from_usize(offset, offset + input.len()),
                "{}",
                parser.parser_name()
            );
            assert!(errors.is_empty());
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&crate::Re2cParser::new());
}

#[test]
fn fragment_ranges_reject_overflow_before_parsing() {
    fn check(parser: &impl ChatParser) {
        for offset in [u32::MAX as usize, usize::MAX] {
            let errors = ErrorCollector::new();
            assert!(parser.parse_word("hello", offset, &errors).is_rejected());
            assert!(
                parser
                    .parse_header("@Comment:\tx", offset, &errors)
                    .is_rejected()
            );
            assert!(parser.parse_com_tier("x\n", offset, &errors).is_rejected());
            assert_eq!(
                errors
                    .into_vec()
                    .iter()
                    .filter(|e| e.code == ErrorCode::ParseFailed)
                    .count(),
                3
            );
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&crate::Re2cParser::new());
}

#[test]
fn fragment_ranges_keep_synthetic_material_outside_document_coordinates() {
    fn check(parser: &impl ChatParser) {
        for (input, kind) in [("n|café", "mor"), ("hɛlo", "pho")] {
            let errors = ErrorCollector::new();
            let offset = u32::MAX as usize - input.len();
            let parsed = if kind == "mor" {
                parser.parse_mor_word(input, offset, &errors).is_parsed()
            } else {
                parser.parse_pho_word(input, offset, &errors).is_parsed()
            };
            assert!(parsed, "{}: {:?}", parser.parser_name(), errors.into_vec());
        }
        let input = "1|2|SUBJ";
        let errors = ErrorCollector::new();
        assert!(
            parser
                .parse_gra_relation(input, u32::MAX as usize - input.len(), &errors)
                .is_parsed(),
            "{}: {:?}",
            parser.parser_name(),
            errors.into_vec()
        );
        let errors = ErrorCollector::new();
        assert!(
            parser
                .parse_com_tier("x\n", u32::MAX as usize - 1, &errors)
                .is_rejected()
        );
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&crate::Re2cParser::new());
}

#[test]
fn fragment_ranges_rebase_diagnostics_without_moving_their_snippet() {
    fn check(parser: &impl ChatParser) {
        let input = "@Languages:\teng\n@Comment:\tcafé";
        let offset = u32::MAX as usize - input.len();
        let errors = ErrorCollector::new();
        assert!(parser.parse_header(input, offset, &errors).is_rejected());
        let errors = errors.into_vec();
        let error = errors
            .iter()
            .find(|e| e.code == ErrorCode::TierValidationError)
            .unwrap();
        assert_eq!(
            error.location.span,
            Span::from_usize(offset, offset + input.len())
        );
        let context = error.context.as_ref().unwrap();
        assert_eq!(context.source_text, input);
        assert!(context.span.end as usize <= input.len());
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&crate::Re2cParser::new());
}
