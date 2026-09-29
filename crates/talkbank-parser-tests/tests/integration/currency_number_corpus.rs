use super::*;

/// Explicit generation uses typed lexical inputs and independently authored
/// written controls. Validation refuses the numeric source without editing it.
#[test]
fn currency_spec_expands_to_authored_english_reference_words() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::{ErrorCode, ErrorCollector};
    use talkbank_transform::{extract::extract_words, num_words::expand_number};

    let parser = TreeSitterParser::new().expect("parser");
    let source = std::fs::read_to_string(workspace_root().join(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E220_currency_forms_1.cha",
    )).expect("canonical currency spec");
    let input =
        strict_parse(parser.parse_chat_file(&source)).expect("currency forms retain syntax");
    let refusal = input
        .clone()
        .validate_into(&ErrorCollector::new(), TranscriptName::Anonymous)
        .expect_err("currency symbols do not license English digits");
    let written_source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/word-features/currency-number-spelling.cha"),
    )
    .expect("authored currency controls");
    let written =
        strict_parse(parser.parse_chat_file(&written_source)).expect("written controls parse");
    let errors = ErrorCollector::new();
    let admitted = written
        .validate_into(&errors, TranscriptName::Anonymous)
        .expect("written controls validate");
    assert!(errors.is_empty());
    let numerals = extract_words(&input, PositionalDomain::Mor);
    let controls = extract_words(admitted.document(), PositionalDomain::Mor);
    assert_eq!(numerals.len(), 1);
    assert_eq!(controls.len(), 8);
    assert_eq!(numerals[0].words.len(), controls.len());
    assert_eq!(
        refusal
            .diagnostics()
            .iter()
            .filter(|error| error.code == ErrorCode::IllegalDigits)
            .count(),
        8
    );
    for (numeral, control) in numerals[0].words.iter().zip(controls) {
        let expected = control
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            expand_number(numeral.text.as_str(), "eng"),
            expected,
            "{}",
            numeral.text
        );
    }
    assert_eq!(
        input.to_chat_string(),
        source,
        "expansion cannot edit source evidence"
    );
    assert_eq!(admitted.document().to_chat_string(), written_source);
}
