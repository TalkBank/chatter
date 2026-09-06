//! Postcode source locations and normalization through public parser boundaries.

use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome, SemanticEq, Span};

#[test]
fn postcode_admission_matches_the_spec_at_file_and_fragment_boundaries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E363.md")
        .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();

    // These are actual source-spec examples, not a parallel CHAT corpus.
    let source = spec.examples()[0].chat.as_str();
    let main = source.lines().find(|line| line.starts_with('*')).unwrap();
    let main_offset = source.find(main).unwrap();
    let postcode_start = source.find("[+").unwrap();
    let postcode_end = postcode_start + source[postcode_start..].find(']').unwrap() + 1;
    let expected = Span::from_usize(postcode_start, postcode_end);

    for external_offset in [0, 200] {
        let errors = ErrorCollector::new();
        assert!(matches!(
            parser.parse_chat_file(source, external_offset, &errors),
            ParseOutcome::Parsed(_)
        ));
        let diagnoses: Vec<_> = errors
            .into_vec()
            .into_iter()
            .filter(|error| error.code == ErrorCode::InvalidPostcode)
            .collect();
        assert_eq!(diagnoses.len(), 1);
        assert_eq!(
            diagnoses[0].location.span,
            Span::from_usize(
                postcode_start + external_offset,
                postcode_end + external_offset
            )
        );
    }

    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(tier) = parser.parse_main_tier(main, main_offset, &errors) else {
        panic!("recovery must preserve the main tier");
    };
    assert!(tier.content.postcodes.is_empty());
    let diagnoses: Vec<_> = errors
        .into_vec()
        .into_iter()
        .filter(|error| error.code == ErrorCode::InvalidPostcode)
        .collect();
    assert_eq!(diagnoses.len(), 1);
    assert_eq!(diagnoses[0].location.span, expected);

    let errors = ErrorCollector::new();
    assert!(matches!(
        parser.parse_utterance(main, main_offset, &errors),
        ParseOutcome::Parsed(_)
    ));
    let diagnoses: Vec<_> = errors
        .into_vec()
        .into_iter()
        .filter(|error| error.code == ErrorCode::InvalidPostcode)
        .collect();
    assert_eq!(diagnoses.len(), 1);
    assert_eq!(diagnoses[0].location.span, expected);

    let legal = spec
        .examples()
        .iter()
        .find(|example| example.chat.as_str().contains("[+  bch  ]"))
        .unwrap();
    let main = legal
        .chat
        .as_str()
        .lines()
        .find(|line| line.starts_with('*'))
        .unwrap();
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(actual) = parser.parse_main_tier(main, 0, &errors) else {
        panic!("legal padded postcode must parse");
    };
    assert!(errors.into_vec().is_empty());
    let canonical = talkbank_parser::TreeSitterParser::new()
        .unwrap()
        .parse_main_tier(main)
        .unwrap();
    assert!(canonical.semantic_eq(&actual));
    assert_eq!(actual.content.postcodes[0].text.as_str(), " bch");
}
