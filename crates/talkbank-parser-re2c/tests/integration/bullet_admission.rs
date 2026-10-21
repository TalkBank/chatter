//! Numeric admission must govern recovered model values, not only diagnostics.

use talkbank_model::model::WriteChat;
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
use talkbank_parser_re2c::Re2cParser;

const OVERFLOW: &str =
    include_str!("../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E360_3.cha");
const ZERO_START: &str =
    include_str!("../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E748_3.cha");

#[test]
fn overflow_is_diagnosed_without_inventing_model_timing() {
    let parser = Re2cParser::new();
    let main = OVERFLOW.lines().find(|line| line.starts_with('*')).unwrap();
    let offset = OVERFLOW.find(main).unwrap();
    for (source, origin) in [(OVERFLOW, 0), (main, offset)] {
        let errors = ErrorCollector::new();
        let output = if origin == 0 {
            parser
                .parse_chat_file(source, 0, &errors)
                .into_option()
                .unwrap()
                .to_chat_string()
        } else {
            parser
                .parse_main_tier(source, origin, &errors)
                .into_option()
                .unwrap()
                .to_chat_string()
        };
        assert!(
            errors
                .into_vec()
                .iter()
                .any(|error| error.code == ErrorCode::InvalidMediaBullet)
        );
        assert!(
            output.contains("the dog"),
            "surrounding content lost: {output:?}"
        );
        assert!(
            !output.contains('\u{15}'),
            "overflow produced timing: {output:?}"
        );
    }
}

#[test]
fn legal_zero_start_is_not_rejected_by_numeric_admission() {
    let errors = ErrorCollector::new();
    let model = Re2cParser::new()
        .parse_chat_file(ZERO_START, 0, &errors)
        .into_option()
        .unwrap();
    assert!(errors.is_empty());
    assert!(model.to_chat_string().contains("\u{15}0_200\u{15}"));
}

fn example(title: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    specs
        .iter()
        .find(|spec| spec.filename == "E360.md")
        .unwrap()
        .examples()
        .iter()
        .find(|example| example.title.as_deref() == Some(title))
        .unwrap()
        .chat
        .as_str()
        .to_owned()
}

fn assert_overflow(errors: ErrorCollector, origin: usize, input: &str) {
    let diagnostics = errors.into_vec();
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|error| error.code == ErrorCode::InvalidMediaBullet)
        .collect();
    assert!(
        !errors.is_empty(),
        "missing overflow diagnostic: {diagnostics:?}"
    );
    for error in errors {
        let span = error.location.span().expect("located diagnostic");
        assert!(span.start() >= origin);
        assert!(span.end() <= origin + input.len());
        let slice = &input[span.start() - origin..span.end() - origin];
        assert!(
            slice.contains("18446744073709551616"),
            "wrong region: {slice:?}"
        );
    }
}

#[test]
fn nested_header_and_wor_routes_preserve_content_but_not_overflow() {
    let parser = Re2cParser::new();
    for (title, prefix) in [
        (
            "Nested overflow retains surrounding content without invented timing",
            "*CHI:\t",
        ),
        ("Internal overflow retains words on both sides", "*CHI:\t"),
        (
            "Comment header overflow is not a zero timestamp",
            "@Comment:\t",
        ),
        (
            "WOR overflow retains the word without invented timing",
            "%wor:\t",
        ),
    ] {
        let source = example(title);
        let errors = ErrorCollector::new();
        let output = parser
            .parse_chat_file(&source, 0, &errors)
            .into_option()
            .unwrap()
            .to_chat_string();
        assert_overflow(errors, 0, &source);
        assert!(
            output.contains("before") && output.contains("after"),
            "{output:?}"
        );
        assert!(
            !output.contains("18446744073709551616") && !output.contains("\u{15}0_"),
            "{output:?}"
        );
        if prefix == "%wor:\t" {
            assert!(output.contains("\u{15}300_400\u{15}"));
        }
        assert_eq!(
            output.matches('\u{15}').count(),
            if prefix == "%wor:\t" { 2 } else { 0 }
        );

        let line = source
            .lines()
            .find(|line| line.starts_with(prefix))
            .unwrap();
        let input = if prefix == "%wor:\t" {
            line.strip_prefix(prefix).unwrap()
        } else {
            line
        };
        let origin = 173;
        let errors = ErrorCollector::new();
        let output = match prefix {
            "*CHI:\t" => parser
                .parse_main_tier(input, origin, &errors)
                .into_option()
                .unwrap()
                .to_chat_string(),
            "@Comment:\t" => parser
                .parse_header(input, origin, &errors)
                .into_option()
                .unwrap()
                .to_chat_string(),
            _ => parser
                .parse_wor_tier(input, origin, &errors)
                .into_option()
                .unwrap()
                .to_chat_string(),
        };
        assert_overflow(errors, origin, input);
        assert!(
            output.contains("before") && output.contains("after"),
            "{output:?}"
        );
        assert!(
            !output.contains("18446744073709551616") && !output.contains("\u{15}0_"),
            "{output:?}"
        );
        assert_eq!(
            output.matches('\u{15}').count(),
            if prefix == "%wor:\t" { 2 } else { 0 }
        );
    }
}

#[test]
fn rejected_tail_cannot_promote_the_preceding_inline_bullet() {
    for title in [
        "Rejected final timing does not promote an earlier inline bullet",
        "Rejected explicit terminal slot does not absorb an inline bullet",
    ] {
        let source = example(title);
        let input = source.lines().find(|line| line.starts_with('*')).unwrap();
        let errors = ErrorCollector::new();
        let main = Re2cParser::new()
            .parse_main_tier(input, 0, &errors)
            .into_option()
            .unwrap();
        assert_overflow(errors, 0, input);
        assert!(main.content.bullet.is_none());
        assert!(
            main.content
                .content
                .iter()
                .any(|item| matches!(item, talkbank_model::UtteranceContent::InternalBullet(_)))
        );
        assert_eq!(main.to_chat_string().matches('\u{15}').count(), 2);
    }
}

#[test]
fn largest_numeric_pair_remains_representable() {
    let source = example("The largest representable counts pass numeric admission");
    let errors = ErrorCollector::new();
    let output = Re2cParser::new()
        .parse_chat_file(&source, 0, &errors)
        .into_option()
        .unwrap()
        .to_chat_string();
    assert!(
        !errors
            .into_vec()
            .iter()
            .any(|error| error.code == ErrorCode::InvalidMediaBullet)
    );
    assert!(output.contains("\u{15}18446744073709551614_18446744073709551615\u{15}"));
}

#[test]
fn both_text_tier_coordinates_are_independently_admitted() {
    for source in [
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E360_6.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E360_7.cha"
        ),
    ] {
        let input = source
            .lines()
            .find_map(|line| line.strip_prefix("%com:\t"))
            .unwrap();
        let errors = ErrorCollector::new();
        let tier = Re2cParser::new()
            .parse_com_tier(input, 67, &errors)
            .into_option()
            .unwrap();
        assert_overflow(errors, 67, input);
        let output = tier.to_chat_string();
        assert!(output.contains("timing"));
        assert!(!output.contains('\u{15}'));
    }
}

#[test]
fn wor_numeric_rejection_is_reported_by_both_supported_apis() {
    fn check(parser: &impl ChatParser, source: &str) {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .unwrap();
        assert_overflow(errors, 0, source);
        let input = source
            .lines()
            .find_map(|line| line.strip_prefix("%wor:\t"))
            .unwrap();
        let errors = ErrorCollector::new();
        let tier = parser.parse_wor_tier(input, 61, &errors).into_option();
        assert_overflow(errors, 61, input);
        // Primary fragment admission may reject the tier while native recovery
        // retains it. Detection and truthful values are shared requirements,
        // not identical recovery outcomes. The native-specific test above
        // independently requires its recovered words and valid sibling timing.
        for output in
            std::iter::once(file.to_chat_string()).chain(tier.map(|tier| tier.to_chat_string()))
        {
            assert!(output.contains("before") && output.contains("after"));
            assert!(output.contains("\u{15}300_400\u{15}"));
            assert_eq!(output.matches('\u{15}').count(), 2);
        }
    }
    let source = example("WOR overflow retains the word without invented timing");
    check(&Re2cParser::new(), &source);
    check(&talkbank_parser::TreeSitterParser::new().unwrap(), &source);
}
