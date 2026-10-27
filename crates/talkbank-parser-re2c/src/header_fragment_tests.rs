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
                error.location.span().expect("located diagnostic"),
                Span::new(offset, offset + input.len())
            );
            let context = error.context.as_ref().unwrap();
            assert_eq!(context.source_text, input);
            assert_eq!(context.span, Span::new(0, input.len()));
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&crate::Re2cParser::new());
}

#[test]
fn media_shared_construction_preserves_fields_and_comma_provenance() {
    use talkbank_model::{
        SemanticEq, Span,
        model::{Header, WriteChat},
    };
    let parser = crate::Re2cParser::new();
    for fixture in [
        include_str!("../../../corpus/reference/core/headers-media.cha"),
        include_str!("../../../corpus/reference/core/headers-media-notrans.cha"),
        include_str!("../../../corpus/reference/core/headers-media-url.cha"),
        include_str!("../../../corpus/reference/core/headers-media-http-url.cha"),
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E767_1.cha"),
        include_str!("../../talkbank-parser-tests/tests/error_corpus/validation_errors/E767_2.cha"),
    ] {
        let authored = fixture
            .lines()
            .find(|line| line.starts_with("@Media:"))
            .unwrap();
        let normalized = authored.replace("  ,", ",").replace(" ,", ",");
        for newline in ["\n", "\r\n", "\r"] {
            let source = fixture.replace('\n', newline);
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(&source, 200, &errors)
                .into_option()
                .unwrap();
            let whole = file
                .headers()
                .find(|header| matches!(header, Header::Media(_)))
                .unwrap();
            let fragment = parser
                .parse_header(&format!("{authored}{newline}"), 200, &errors)
                .into_option()
                .unwrap();
            assert!(errors.is_empty(), "{:?}", errors.to_vec());
            assert!(whole.semantic_eq(&fragment));
            assert_eq!(fragment.to_chat_string(), normalized);
            let decoded: Header =
                serde_json::from_str(&serde_json::to_string(&fragment).unwrap()).unwrap();
            assert!(decoded.semantic_eq(&fragment));
            let Header::Media(media) = fragment else {
                panic!("media fragment")
            };
            let Header::Media(whole_media) = whole else {
                panic!("media file")
            };
            if let Some(comma) = authored.find(" ,") {
                let gap_start = authored[..=comma].trim_end().len();
                let gap_end = comma + 1;
                assert_eq!(
                    media.whitespace_before_comma.unwrap().span(),
                    Some(Span::new(200 + gap_start, 200 + gap_end))
                );
                let line_start = source.find(authored).unwrap();
                assert_eq!(
                    whole_media.whitespace_before_comma.unwrap().span(),
                    Some(Span::new(
                        200 + line_start + gap_start,
                        200 + line_start + gap_end
                    ))
                );
            } else {
                assert!(media.whitespace_before_comma.is_none());
                assert!(whole_media.whitespace_before_comma.is_none());
            }
        }
    }
}
