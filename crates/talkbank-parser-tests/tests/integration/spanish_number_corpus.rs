use super::*;

#[test]
fn reference_spanish_cardinals_preserve_hundred_boundary() {
    use talkbank_model::ErrorCollector;
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_transform::{extract::extract_words, num_words::expand_number};
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/word-features/spanish-number-spelling.cha"),
    )
    .expect("authored Spanish reference");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let errors = ErrorCollector::new();
    let admitted = file
        .validate_into(&errors, TranscriptName::Anonymous)
        .expect("written cardinals validate");
    assert!(errors.is_empty());
    let controls = extract_words(admitted.document(), PositionalDomain::Mor);
    let numerals = [
        "100", "101", "199", "200", "201", "1000", "1101", "1100", "2000", "2001", "100000", "0",
        "21", "999", "10000", "11000", "111000", "999999",
    ];
    assert_eq!(controls.len(), numerals.len());
    for (numeral, control) in numerals.into_iter().zip(controls) {
        let expected = control
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(expand_number(numeral, "spa"), expected, "{numeral}");
        for word in &control.words {
            assert_eq!(expand_number(word.text.as_str(), "spa"), word.text.as_str());
        }
    }
    assert_eq!(admitted.document().to_chat_string(), source);
}

/// Refused grammar must preserve the caller's spelling, not partially expand it.
#[test]
fn spanish_cardinal_unsupported_grammar_preserves_input() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::validation::LanguageResolution;
    use talkbank_model::{ErrorCode, ErrorCollector};
    use talkbank_transform::{extract::extract_words, num_words::expand_number};

    let source = std::fs::read_to_string(workspace_root().join(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E220_spanish_generation_limits_1.cha",
    )).expect("authored Spanish generation refusals");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("numeric syntax preserved");
    let refusal = file
        .clone()
        .validate_into(&ErrorCollector::new(), TranscriptName::Anonymous)
        .expect_err("generation refusal does not license digits");
    assert!(!refusal.has_incomplete_parse());
    assert_eq!(refusal.diagnostics().len(), 8);
    assert!(
        refusal
            .diagnostics()
            .iter()
            .all(|error| error.code == ErrorCode::IllegalDigits)
    );
    let turns = extract_words(&file, PositionalDomain::Mor);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].words.len(), 8);
    for word in &turns[0].words {
        let resolved = word.resolve_language(file.languages.first(), &file.languages);
        assert!(resolved.diagnostics.is_empty());
        let LanguageResolution::Single(language) = resolved.resolution else {
            panic!("spec supplies one generation language");
        };
        assert_eq!(language.as_str(), "spa");
        assert_eq!(
            expand_number(word.text.as_str(), language.as_str()),
            word.text.as_str()
        );
    }
    assert_eq!(file.to_chat_string(), source);
    // These belong to the string API boundary, not plain CHAT numerals:
    // dollar syntax is timing and leading zero marks an omitted word.
    for input in ["$1000000", "021000"] {
        assert_eq!(expand_number(input, "spa"), input);
    }
}
