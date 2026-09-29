//! Language-switch rewrites grounded in authored specs and reference CHAT.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{SemanticEq, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{
    chat_corpus::ChatCorpus, repo_paths::workspace_root, test_error::strict_parse,
};
use talkbank_transform::fix_s::rewrite_whole_utterance_language_switches;

#[test]
fn reference_validation_context_builders_preserve_clone_isolation_and_language_order() {
    use std::sync::Arc;
    use talkbank_model::validation::ValidationContext;
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut declared = 0;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        let languages: Vec<_> = file.languages.iter().cloned().collect();
        let speakers = file.participants.keys().cloned().collect();
        let empty = ValidationContext::new();
        let mut configured = empty
            .clone()
            .with_participant_ids(speakers)
            .with_declared_languages(languages.clone());
        if let Some(primary) = languages.first() {
            configured = configured.with_default_language(primary.clone());
        }
        assert!(empty.shared.participant_ids.is_empty());
        assert!(empty.shared.declared_languages.is_empty());
        assert!(empty.shared.default_language.is_none());
        assert_eq!(
            configured.shared.participant_ids.len(),
            file.participants.len()
        );
        assert!(
            file.participants
                .keys()
                .all(|speaker| configured.shared.participant_ids.contains(speaker))
        );
        assert_eq!(configured.shared.declared_languages, languages);
        assert_eq!(
            configured.shared.default_language.as_ref(),
            languages.first()
        );

        // This is an explicit caller mode override, not a claim about the
        // reference transcript's @Options header or its validity in CA mode.
        let mode_override = configured.clone().with_ca_mode(true);
        assert!(!configured.shared.ca_mode && !empty.shared.ca_mode);
        assert!(mode_override.shared.ca_mode);
        assert_eq!(mode_override.shared.declared_languages, languages);
        assert!(!Arc::ptr_eq(&configured.shared, &mode_override.shared));
        let tier = configured
            .clone()
            .with_tier_language(languages.first().cloned());
        assert!(Arc::ptr_eq(&configured.shared, &tier.shared));
        assert!(configured.tier_language.is_none());
        assert_eq!(tier.tier_language.as_ref(), languages.first());
        for language in &languages {
            let position = languages
                .iter()
                .position(|candidate| candidate == language)
                .expect("declared language position");
            let alternate = match position {
                0 => languages.get(1),
                1 => languages.first(),
                _ => None,
            };
            assert_eq!(configured.get_other_language(language).as_ref(), alternate);
            assert_eq!(configured.is_tertiary_language(language), position >= 2);
            assert!(empty.get_other_language(language).is_none());
            assert!(!empty.is_tertiary_language(language));
            declared += 1;
        }
    }
    assert!(declared > 0, "ordered declarations must be witnessed");
}

/// Candidate queries and their wire representation serve downstream language
/// selection. An all-candidates query is not the permissive E220 policy, and
/// an unresolved set cannot authorize a language-specific operation.
#[test]
fn canonical_language_candidates_preserve_order_quantification_and_wire_identity() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::validation::LanguageResolution;
    use talkbank_transform::extract::extract_words;

    let parser = TreeSitterParser::new().expect("parser");
    for (spec, index, candidates, display, wire, all_in_selection) in [
        (
            "E220_3",
            1,
            &["eng", "zho"][..],
            "eng+zho",
            serde_json::json!({"Multiple": ["eng", "zho"]}),
            false,
        ),
        (
            "E220_4",
            1,
            &["eng", "fra"][..],
            "eng+fra",
            serde_json::json!({"Multiple": ["eng", "fra"]}),
            true,
        ),
        (
            "E220_5",
            1,
            &["eng", "zho"][..],
            "eng&zho",
            serde_json::json!({"Ambiguous": ["eng", "zho"]}),
            false,
        ),
        (
            "E220_6",
            1,
            &["eng", "fra"][..],
            "eng&fra",
            serde_json::json!({"Ambiguous": ["eng", "fra"]}),
            true,
        ),
        (
            "E504_3",
            0,
            &["eng"][..],
            "eng",
            serde_json::json!({"Single": "eng"}),
            true,
        ),
        (
            "E504_2",
            0,
            &[][..],
            "<unresolved>",
            serde_json::json!("Unresolved"),
            true,
        ),
    ] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{spec}.cha",
        )))
        .expect("canonical language-policy specimen");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax parses");
        let extracted = extract_words(&file, PositionalDomain::Mor);
        assert_eq!(extracted.len(), 1, "{spec}");
        let outcome =
            extracted[0].words[index].resolve_language(file.languages.first(), &file.languages);
        assert!(
            outcome.diagnostics.is_empty(),
            "{spec}: {:?}",
            outcome.diagnostics
        );
        let resolution = outcome.resolution;
        assert_eq!(
            resolution
                .languages()
                .iter()
                .map(|code| code.as_str())
                .collect::<Vec<_>>(),
            candidates,
            "candidate order: {spec}"
        );
        assert_eq!(resolution.as_display_string(), display, "{spec}");
        assert_eq!(
            resolution.is_valid_in_all(|code| ["eng", "fra"].contains(&code.as_str())),
            all_in_selection,
            "strict downstream selection, not E220 permission: {spec}"
        );
        // Universal quantification is vacuously true for an empty set. The
        // explicit unresolved state survives the wire and remains observable.
        assert_eq!(
            matches!(resolution, LanguageResolution::Unresolved),
            candidates.is_empty()
        );
        let serialized = serde_json::to_value(&resolution).expect("resolution serializes");
        assert_eq!(serialized, wire, "{spec}");
        let restored: LanguageResolution =
            serde_json::from_value(serialized).expect("wire roundtrip");
        assert_eq!(restored, resolution, "{spec}");
        assert_eq!(
            file.to_chat_string(),
            source,
            "queries must preserve {spec}"
        );
    }
}

#[test]
fn reference_english_number_spelling_matches_authored_controls() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::{ErrorCode, ErrorCollector};
    use talkbank_transform::{extract::extract_words, num_words::expand_number};

    let parser = TreeSitterParser::new().expect("parser");
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/word-features/english-number-spelling.cha"),
    )
    .expect("authored English number controls");
    let parsed = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let errors = ErrorCollector::new();
    let admitted = parsed
        .validate_into(&errors, TranscriptName::Anonymous)
        .expect("written controls are valid CHAT");
    assert!(errors.is_empty());
    let controls = extract_words(admitted.document(), PositionalDomain::Mor);

    let input_source = std::fs::read_to_string(workspace_root().join(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E220_english_number_forms_1.cha",
    )).expect("canonical numeric-form specimen");
    let input =
        strict_parse(parser.parse_chat_file(&input_source)).expect("numeric forms retain syntax");
    let refusal = input
        .clone()
        .validate_into(&ErrorCollector::new(), TranscriptName::Anonymous)
        .expect_err("English suffixes do not license digits");
    let extracted = extract_words(&input, PositionalDomain::Mor);
    assert_eq!(extracted.len(), 1);
    assert_eq!(controls.len(), 44);
    assert_eq!(extracted[0].words.len(), controls.len());
    assert_eq!(
        refusal
            .diagnostics()
            .iter()
            .filter(|error| error.code == ErrorCode::IllegalDigits)
            .count(),
        controls.len()
    );
    for (numeral, control) in extracted[0].words.iter().zip(&controls) {
        for word in &control.words {
            assert_eq!(
                expand_number(word.text.as_str(), "eng"),
                word.text.as_str(),
                "already written reference words must not be reshaped",
            );
        }
        let expected = control
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            expand_number(numeral.text.as_str(), "eng"),
            expected,
            "authored generation control for {}",
            numeral.text
        );
    }
    assert_eq!(admitted.document().to_chat_string(), source);
    assert_eq!(
        input.to_chat_string(),
        input_source,
        "lookup must not repair its evidence"
    );
}

#[test]
fn number_language_controls() {
    use talkbank_model::ErrorCollector;
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::validation::LanguageResolution;
    use talkbank_transform::{extract::extract_words, num_words::expand_number};

    let source = std::fs::read_to_string(workspace_root().join(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E220_bare_numerals_2.cha",
    ))
    .expect("canonical legal tone and homonym controls");
    let parser = TreeSitterParser::new().expect("parser");
    let parsed = strict_parse(parser.parse_chat_file(&source)).expect("control syntax");
    let errors = ErrorCollector::new();
    let admitted = parsed
        .validate_into(&errors, TranscriptName::Anonymous)
        .expect("lexical digits are legal under the declared language");
    assert!(errors.is_empty());
    let file = admitted.document();
    let extracted = extract_words(file, PositionalDomain::Mor);
    assert_eq!(extracted.len(), 1);
    assert_eq!(extracted[0].words.len(), 2);
    for (word, expected) in extracted[0].words.iter().zip(["ma1", "本1"]) {
        assert_eq!(word.text.as_str(), expected);
        let language = word.resolve_language(file.languages.first(), &file.languages);
        assert!(language.diagnostics.is_empty());
        let LanguageResolution::Single(language) = language.resolution else {
            panic!("control supplies one unambiguous language");
        };
        assert_eq!(language.as_str(), "zho");
        assert_eq!(
            expand_number(word.text.as_str(), language.as_str()),
            expected
        );
    }
    assert_eq!(
        file.to_chat_string(),
        source,
        "lookup must preserve its evidence"
    );
}

#[path = "currency_number_corpus.rs"]
mod currency_contracts;

#[path = "spanish_number_corpus.rs"]
mod spanish_number_contracts;

/// Exact lexical entries are distinct from permission to compose larger numbers.
#[test]
fn reference_table_cardinals_preserve_authored_lexical_controls() {
    use talkbank_model::ErrorCollector;
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::validation::LanguageResolution;
    use talkbank_transform::{extract::extract_words, num_words::expand_number};

    let parser = TreeSitterParser::new().expect("parser");
    for (language, filename) in [
        ("fra", "french-number-spelling.cha"),
        ("deu", "german-number-spelling.cha"),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("corpus/reference/word-features")
                .join(filename),
        )
        .expect("authored cardinal controls");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
        let errors = ErrorCollector::new();
        let admitted = file
            .validate_into(&errors, TranscriptName::Anonymous)
            .expect("written cardinals validate");
        assert!(errors.is_empty());
        let controls = extract_words(admitted.document(), PositionalDomain::Mor);
        let numerals = ["0", "21", "100", "200", "1000", "10000"];
        assert_eq!(controls.len(), numerals.len());
        for (numeral, control) in numerals.into_iter().zip(controls) {
            assert!(!control.words.is_empty());
            let expected = control
                .words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(
                expand_number(numeral, language),
                expected,
                "{language} {numeral}"
            );
            assert_eq!(
                expand_number(numeral, &language.to_ascii_uppercase()),
                expected
            );
            for word in &control.words {
                let resolved = word.resolve_language(
                    admitted.document().languages.first(),
                    &admitted.document().languages,
                );
                assert!(resolved.diagnostics.is_empty());
                let LanguageResolution::Single(actual) = resolved.resolution else {
                    panic!("control must carry one language");
                };
                assert_eq!(actual.as_str(), language);
                assert_eq!(
                    expand_number(word.text.as_str(), language),
                    word.text.as_str()
                );
            }
        }
        assert_eq!(admitted.document().to_chat_string(), source);
        // A complete table phrase (10000) is not a scale unit for 100000.
        // Unsupported generation must preserve the entire caller-supplied token.
        for input in [
            "100000",
            "$100000",
            "100000€",
            "21-100000",
            "21\u{2014}100000",
            "100000-year-old",
        ] {
            assert_eq!(expand_number(input, language), input);
        }
    }
}

#[test]
fn reference_mandarin_number_spelling_matches_authored_controls() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::validation::LanguageResolution;
    use talkbank_model::{ErrorCode, ErrorCollector};
    use talkbank_transform::{extract::extract_words, num_words::expand_number};
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/word-features/mandarin-number-spelling.cha"),
    )
    .expect("authored number controls");
    let parser = TreeSitterParser::new().expect("parser");
    let parsed = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let errors = ErrorCollector::new();
    let admitted = parsed
        .validate_into(&errors, TranscriptName::Anonymous)
        .expect("written controls are valid CHAT");
    assert!(errors.to_vec().is_empty());
    let file = admitted.document();
    let controls = extract_words(file, PositionalDomain::Mor);
    let input_source = std::fs::read_to_string(workspace_root().join(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E220_bare_numerals_1.cha",
    )).expect("canonical bare numeral spec");
    let input =
        strict_parse(parser.parse_chat_file(&input_source)).expect("numerals retain valid syntax");
    let refusal = input
        .clone()
        .validate_into(&ErrorCollector::new(), TranscriptName::Anonymous)
        .expect_err("tone-digit exemption cannot admit bare numerals");
    assert_eq!(
        refusal
            .diagnostics()
            .iter()
            .filter(|error| error.code == ErrorCode::IllegalDigits)
            .count(),
        10
    );
    let extracted = extract_words(&input, PositionalDomain::Mor);
    assert_eq!(extracted.len(), 1);
    assert_eq!(extracted[0].words.len(), controls.len());
    assert_eq!(controls.len(), 10);
    for (numeral, control) in extracted[0].words.iter().zip(&controls) {
        assert_eq!(control.words.len(), 1);
        let language = numeral.resolve_language(input.languages.first(), &input.languages);
        assert!(language.diagnostics.is_empty());
        let LanguageResolution::Single(language) = language.resolution else {
            panic!("reference supplies one unambiguous numeral language");
        };
        assert_eq!(
            expand_number(numeral.text.as_str(), language.as_str()),
            control.words[0].text.as_str(),
            "authored spelling of {}",
            numeral.text
        );
    }
    assert_eq!(
        file.to_chat_string(),
        source,
        "generation lookup must not edit the transcript"
    );
    assert_eq!(
        input.to_chat_string(),
        input_source,
        "invalid input is evidence, not automatically repaired"
    );
}

#[test]
fn reference_cantonese_number_spelling_uses_traditional_script() {
    use talkbank_model::ErrorCollector;
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::validation::LanguageResolution;
    use talkbank_transform::{extract::extract_words, num_words::expand_number};

    let parser = TreeSitterParser::new().expect("parser");
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/word-features/cantonese-number-spelling.cha"),
    )
    .expect("authored traditional-script controls");
    let parsed = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let errors = ErrorCollector::new();
    let admitted = parsed
        .validate_into(&errors, TranscriptName::Anonymous)
        .expect("written controls validate");
    assert!(errors.is_empty());
    let file = admitted.document();
    let controls = extract_words(file, PositionalDomain::Mor);
    let numeric_source = std::fs::read_to_string(workspace_root().join(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E220_bare_numerals_1.cha",
    )).expect("canonical numeral spec");
    let input = strict_parse(parser.parse_chat_file(&numeric_source)).expect("numeral syntax");
    let numerals = extract_words(&input, PositionalDomain::Mor);
    assert_eq!(numerals.len(), 1);
    assert_eq!(controls.len(), 10);
    assert_eq!(numerals[0].words.len(), controls.len());
    for (numeral, control) in numerals[0].words.iter().zip(&controls) {
        assert_eq!(control.words.len(), 1);
        // The destination transcript supplies the generation language. The
        // invalid numeric source remains evidence, not a validated input.
        let language = control.words[0].resolve_language(file.languages.first(), &file.languages);
        assert!(language.diagnostics.is_empty());
        let LanguageResolution::Single(language) = language.resolution else {
            panic!("reference must supply one generation language");
        };
        assert_eq!(language.as_str(), "yue");
        assert_eq!(
            expand_number(numeral.text.as_str(), language.as_str()),
            control.words[0].text.as_str(),
            "{}",
            numeral.text
        );
    }
    assert_eq!(file.to_chat_string(), source);
    assert_eq!(input.to_chat_string(), numeric_source);
}

#[test]
fn numeric_overflow_specs_preserve_unsupported_lexical_input() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::validation::LanguageResolution;
    use talkbank_model::{ErrorCode, ErrorCollector};
    use talkbank_transform::{extract::extract_words, num_words::expand_number};

    let parser = TreeSitterParser::new().expect("parser");
    for (example, expected_language, expected_words) in
        [(1, "eng", 20), (2, "zho", 1), (3, "yue", 1), (4, "spa", 1)]
    {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E220_numeric_overflow_{example}.cha",
        ))).expect("authored overflow spec");
        let file =
            strict_parse(parser.parse_chat_file(&source)).expect("overflow is not syntax loss");
        let failure = file
            .clone()
            .validate_into(&ErrorCollector::new(), TranscriptName::Anonymous)
            .expect_err("overflow does not license digits");
        assert!(!failure.has_incomplete_parse());
        assert_eq!(failure.diagnostics().len(), expected_words);
        assert!(
            failure
                .diagnostics()
                .iter()
                .all(|error| error.code == ErrorCode::IllegalDigits)
        );
        let turns = extract_words(&file, PositionalDomain::Mor);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].words.len(), expected_words);
        for word in &turns[0].words {
            let resolved = word.resolve_language(file.languages.first(), &file.languages);
            assert!(resolved.diagnostics.is_empty());
            let LanguageResolution::Single(language) = resolved.resolution else {
                panic!("spec supplies one generation language");
            };
            assert_eq!(language.as_str(), expected_language);
            assert_eq!(
                expand_number(word.text.as_str(), language.as_str()),
                word.text.as_str(),
                "unsupported generation must preserve the complete lexical value"
            );
        }
        assert_eq!(file.to_chat_string(), source);
    }
}

#[test]
fn scoped_language_specs_preserve_extracted_governors() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::validation::{GoverningMarkKind, LanguageResolution};
    use talkbank_transform::extract::extract_words;

    let parser = TreeSitterParser::new().expect("parser");
    for (example, enclosing) in [(7, "zho"), (8, "eng")] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E220_{example}.cha",
        )))
        .expect("canonical scoped-language spec");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax parses");
        for domain in [
            PositionalDomain::Mor,
            PositionalDomain::Pho,
            PositionalDomain::Sin,
        ] {
            let extracted = extract_words(&file, domain);
            assert_eq!(extracted.len(), 1);
            let mut expected = vec![
                ("hello", GoverningMarkKind::Utterance, "eng"),
                ("ma1", GoverningMarkKind::Span, enclosing),
            ];
            if domain == PositionalDomain::Mor {
                expected.push((",", GoverningMarkKind::Span, enclosing));
            }
            expected.push(("ma2", GoverningMarkKind::Own, "zho"));
            assert_eq!(extracted[0].words.len(), expected.len(), "{domain:?}");
            for (word, (text, kind, code)) in extracted[0].words.iter().zip(expected) {
                assert_eq!(word.text.as_str(), text);
                assert_eq!(word.language_kind(), kind, "{text}");
                let outcome = word.resolve_language(file.languages.first(), &file.languages);
                assert!(
                    outcome.diagnostics.is_empty(),
                    "{text}: {:?}",
                    outcome.diagnostics
                );
                let LanguageResolution::Single(language) = outcome.resolution else {
                    panic!("explicit or inherited single language for {text}");
                };
                assert_eq!(language.as_str(), code, "E220_{example} {domain:?} {text}");
            }
        }
        assert_eq!(file.to_chat_string(), source, "extraction is read-only");
    }
}

/// Domain selection is a wire/semantic policy, not an index invariant. These
/// authored outputs do not call the selection helper to predict its answer.
#[test]
fn reference_extraction_selects_replacements_and_spoken_retraces() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_transform::extract::extract_words;

    let parser = TreeSitterParser::new().expect("parser");
    for (relative, mor, spoken) in [
        (
            "annotation/errors-and-replacements.cha",
            vec![
                "whenn is it",
                "rocking+horse",
                "when+ever will you go",
                "this is child speaking",
                "this is child speaking",
                "this is a+er b speaking",
                "this is child speaking",
                "this child speaking",
                "this is speaking",
            ],
            vec![
                "whenn is it",
                "rocking+house",
                "when+ever will you go",
                "this is child speaking",
                "this is child speaking",
                "this is child speaking",
                "this is child speaking",
                "this is child speaking",
                "this is child speaking",
            ],
        ),
        (
            "content/separators.cha",
            vec![
                "well , I think so",
                "it was lowering concrete girders off a lorry „ wasn't it",
                "it was lowering concrete girders off a lorry ‡ wasn't it",
            ],
            vec![
                "well I think so",
                "it was lowering concrete girders off a lorry wasn't it",
                "it was lowering concrete girders off a lorry wasn't it",
            ],
        ),
        (
            "annotation/retrace.cha",
            vec![
                "I want that",
                "the the cat ran",
                "I want give me that",
                "an",
                "the the magazine is here",
                "kitty is nice",
                "later in the day",
                "female",
                "dog",
                "dog",
                "the dog",
                "and now",
                "and now",
            ],
            vec![
                "I I want that",
                "the dog the cat ran",
                "I want the give me that",
                "ana an",
                "the book the magazine is here",
                "tika@u kitty is nice",
                "lɛɾɪ@u later in the day",
                "male male",
                "dog dog",
                "dog dog",
                "the dog the dog",
                "then and now",
                "then and now",
            ],
        ),
    ] {
        let source =
            std::fs::read_to_string(workspace_root().join("corpus/reference").join(relative))
                .expect("canonical extraction reference");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
        for domain in [
            PositionalDomain::Mor,
            PositionalDomain::Pho,
            PositionalDomain::Sin,
        ] {
            let extracted = extract_words(&file, domain);
            let actual: Vec<_> = extracted
                .iter()
                .map(|utterance| {
                    utterance
                        .words
                        .iter()
                        .map(|word| word.raw_text.as_str())
                        .collect::<Vec<_>>()
                })
                .collect();
            let expected: Vec<_> = if domain == PositionalDomain::Mor {
                &mor
            } else {
                &spoken
            }
            .iter()
            .map(|line| line.split_whitespace().collect::<Vec<_>>())
            .collect();
            assert_eq!(actual, expected, "{relative} {domain:?}");
        }
        assert_eq!(
            file.to_chat_string(),
            source,
            "extraction must not rewrite the source"
        );
    }
}

#[test]
fn replacement_category_specs_keep_extraction_distinct_from_validity() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::{ErrorCode, ErrorCollector};
    use talkbank_transform::extract::extract_words;
    enum Admission {
        Valid,
        Invalid(ErrorCode),
    }

    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, admission, mor, spoken) in [
        (
            "E387_1",
            Admission::Invalid(ErrorCode::ReplacementOnFragment),
            "dog",
            "",
        ),
        ("E387_2", Admission::Valid, "dog", "&+ba dog"),
        (
            "E388_1",
            Admission::Invalid(ErrorCode::ReplacementOnNonword),
            "um",
            "",
        ),
        ("E388_2", Admission::Valid, "dog", "&~um dog"),
        (
            "E389_1",
            Admission::Invalid(ErrorCode::ReplacementOnFiller),
            "and",
            "",
        ),
        ("E389_2", Admission::Valid, "dog", "&-um dog"),
        (
            "E390_1",
            Admission::Invalid(ErrorCode::ReplacementContainsOmission),
            "",
            "went",
        ),
        ("E390_2", Admission::Valid, "home", "home"),
        (
            "E391_1",
            Admission::Invalid(ErrorCode::ReplacementContainsUntranscribed),
            "",
            "went",
        ),
        ("E391_2", Admission::Valid, "went home", "went xxx home"),
    ] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{fixture}.cha",
        )))
        .expect("canonical replacement category spec");
        let mut file = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax parses");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        match admission {
            Admission::Valid => assert!(diagnostics.is_empty(), "{fixture}: {diagnostics:?}"),
            Admission::Invalid(code) => assert!(
                diagnostics.iter().any(|error| error.code == code),
                "extraction must not imply validity: {fixture}: {diagnostics:?}"
            ),
        }
        for domain in [
            PositionalDomain::Mor,
            PositionalDomain::Pho,
            PositionalDomain::Sin,
        ] {
            let extracted = extract_words(&file, domain);
            assert_eq!(extracted.len(), 1);
            let actual: Vec<_> = extracted[0]
                .words
                .iter()
                .map(|word| word.raw_text.as_str())
                .collect();
            let expected: Vec<_> = if domain == PositionalDomain::Mor {
                mor
            } else {
                spoken
            }
            .split_whitespace()
            .collect();
            assert_eq!(actual, expected, "{fixture} {domain:?}");
        }
        assert_eq!(file.to_chat_string(), source, "extraction is not repair");
    }
}

#[test]
fn semicolon_specs_preserve_typed_invalidity_and_extraction() {
    use talkbank_model::alignment::helpers::PositionalDomain;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::{ErrorCode, ErrorCollector, Span};
    use talkbank_transform::extract::extract_words;
    let parser = TreeSitterParser::new().expect("parser");
    for example in 1..=4 {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E769_{example}.cha",
        )))
        .expect("canonical separator spec");
        let mut file =
            strict_parse(parser.parse_chat_file(&source)).expect("legacy separator parses");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        if example == 1 {
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
        } else {
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
            assert_eq!(diagnostics[0].code, ErrorCode::SemicolonOnMainTier);
            let start = source.find(';').expect("authored semicolon");
            assert_eq!(
                diagnostics[0].location.span,
                Span::from_usize(start, start + 1)
            );
        }
        if example == 2 {
            for domain in [
                PositionalDomain::Mor,
                PositionalDomain::Pho,
                PositionalDomain::Sin,
            ] {
                let extracted = extract_words(&file, domain);
                let raw: Vec<_> = extracted[0]
                    .words
                    .iter()
                    .map(|word| word.raw_text.as_str())
                    .collect();
                assert_eq!(raw, ["one", "two"], "non-tag punctuation is not a word");
            }
        }
        assert_eq!(
            file.to_chat_string(),
            source,
            "diagnosis and extraction must preserve source"
        );
    }
}

#[test]
fn authored_language_assignments_drive_counts_and_switching_queries() {
    use talkbank_model::ErrorCollector;
    use talkbank_model::model::{LanguageCode, Line, WordLanguageInfos, WordLanguages};
    use talkbank_model::validation::{Validate, ValidationContext};

    let parser = TreeSitterParser::new().expect("parser");
    for (relative, expected) in [
        (
            "corpus/reference/content/language-switching.cha",
            vec![
                (false, vec![("zho", 3)], 0),
                (true, vec![("eng", 4), ("zho", 1)], 0),
                (true, vec![("eng", 4), ("zho", 1)], 0),
                (true, vec![("eng", 5), ("fra", 1), ("zho", 1)], 0),
                (
                    true,
                    vec![("eng", 4), ("fra", 1), ("spa", 1), ("zho", 1)],
                    0,
                ),
                (true, vec![("eng", 1), ("zho", 1)], 0),
                // The quick-uptake linker carries no word-language count;
                // the explicit precode assigns all three authored words to zho.
                (false, vec![("zho", 3)], 0),
            ],
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E504_5.cha",
            vec![(false, vec![("spa", 1)], 1)],
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E249_2.cha",
            vec![
                (true, vec![("eng", 1), ("spa", 1)], 0),
                (true, vec![("eng", 1), ("spa", 1)], 0),
            ],
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E249_3.cha",
            vec![(false, vec![], 2), (false, vec![("spa", 1)], 1)],
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E249_4.cha",
            vec![(false, vec![("eng", 1)], 1), (false, vec![("spa", 1)], 1)],
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E249_5.cha",
            vec![
                (true, vec![("eng", 1), ("spa", 1)], 0),
                (false, vec![("fra", 1)], 1),
            ],
        ),
    ] {
        let source = std::fs::read_to_string(workspace_root().join(relative))
            .expect("canonical language evidence");
        let mut file = strict_parse(parser.parse_chat_file(&source)).expect("syntax parses");
        let original_chat = file.to_chat_string();
        let declared_languages = file.languages.clone();
        for language in &declared_languages {
            let errors = ErrorCollector::new();
            language.validate(&ValidationContext::default(), &errors);
            assert!(errors.is_empty(), "authored language-code control");
            // JSON import retains the supplied spelling; it does not silently
            // normalize a caller's uppercase code into an admitted language.
            let wire = serde_json::to_value(language).expect("language wire");
            let uppercase = wire.as_str().expect("language string").to_ascii_uppercase();
            let imported: LanguageCode = serde_json::from_value(serde_json::json!(uppercase))
                .expect("lexical import precedes language validation");
            let errors = ErrorCollector::new();
            imported.validate(&ValidationContext::default(), &errors);
            let findings = errors.into_vec();
            assert_eq!(findings.len(), 1);
            assert_eq!(
                findings[0].code,
                talkbank_model::ErrorCode::InvalidLanguageCode
            );
            assert_eq!(imported.as_str(), uppercase);
        }
        let mut actual = Vec::new();
        for line in &mut file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            assert!(utterance.language_metadata.is_uncomputed());
            assert!(utterance.language_metadata.as_computed().is_none());
            assert!(utterance.language_metadata.as_computed_mut().is_none());
            assert!(
                utterance
                    .language_metadata
                    .clone()
                    .into_computed()
                    .is_none()
            );
            utterance.compute_language_metadata(file.languages.first(), &file.languages);
            let transferred = utterance
                .language_metadata
                .clone()
                .into_computed()
                .expect("computed state transfers its derived payload");
            assert_eq!(
                utterance
                    .language_metadata
                    .as_computed_mut()
                    .expect("computed state exposes mutable metadata"),
                &transferred,
                "metadata adapters preserve all assignments, including unresolved words"
            );
            let metadata = utterance
                .language_metadata
                .as_computed()
                .expect("explicit metadata computation completed");
            let transferred_words = WordLanguageInfos::from(metadata.word_languages.to_vec());
            assert_eq!(transferred_words, metadata.word_languages);
            assert_eq!(
                transferred_words.is_empty(),
                metadata.word_languages.is_empty()
            );
            let mut counts: Vec<_> = metadata
                .count_by_language()
                .into_iter()
                .map(|(code, count)| (code.as_str().to_owned(), count))
                .collect();
            counts.sort();
            let unresolved = metadata
                .word_languages
                .iter()
                .filter(|info| matches!(info.languages, WordLanguages::Unresolved))
                .count();
            actual.push((metadata.is_code_switching(), counts, unresolved));
        }
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(switching, counts, unresolved)| {
                (
                    switching,
                    counts
                        .into_iter()
                        .map(|(code, count)| (code.to_owned(), count))
                        .collect::<Vec<_>>(),
                    unresolved,
                )
            })
            .collect();
        assert_eq!(actual, expected, "{relative}");
        assert_eq!(
            file.languages, declared_languages,
            "metadata must not rewrite declarations"
        );
        assert_eq!(
            file.to_chat_string(),
            original_chat,
            "metadata must not rewrite CHAT"
        );
        // Computing metadata for E504 does not supply its missing declaration
        // or turn unresolved words into assignments from the ID header.
    }
}

#[test]
fn whole_utterance_rewrite_matches_authored_spec_controls() {
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let parser = TreeSitterParser::new().expect("parser");
    for (input, output) in [
        ("E255_1.cha", "E255_3.cha"),
        ("E255_2.cha", "E255_4.cha"),
        ("E255_5.cha", "E255_6.cha"),
    ] {
        let source = std::fs::read_to_string(root.join(input)).expect("authored violation");
        let target = std::fs::read_to_string(root.join(output)).expect("authored legal control");
        let mut actual = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax parses");
        let expected = strict_parse(parser.parse_chat_file(&target)).expect("legal control parses");
        let stats = rewrite_whole_utterance_language_switches(&mut actual);
        assert_eq!(stats.rewritten_utterances, 1);
        assert_eq!(stats.appended_language_codes, 0);
        assert_eq!(actual.to_chat_string(), expected.to_chat_string());
        let wire = actual.to_chat_string();
        let reparsed = strict_parse(parser.parse_chat_file(&wire)).expect("rewritten wire parses");
        assert!(reparsed.semantic_eq(&expected));
        let errors = talkbank_model::ErrorCollector::new();
        reparsed
            .validate_into(&errors, talkbank_model::model::TranscriptName::Anonymous)
            .expect("rewritten spec control is admitted");
        assert!(rewrite_whole_utterance_language_switches(&mut actual).is_empty());
    }
}

#[test]
fn governing_span_spec_cannot_enter_unspanned_rewrite() {
    let source = std::fs::read_to_string(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E255_7.cha"),
    )
    .expect("canonical governing-span control");
    let parser = TreeSitterParser::new().expect("parser");
    let mut file = strict_parse(parser.parse_chat_file(&source)).expect("spec parses");
    let original = file.clone();
    for utterance in file.utterances() {
        assert!(
            utterance
                .main
                .whole_utterance_language_switch_target(file.languages.first(), &file.languages,)
                .is_none(),
            "span-governed words cannot produce an unspanned capability"
        );
    }
    assert!(rewrite_whole_utterance_language_switches(&mut file).is_empty());
    assert!(original.semantic_eq(&file));
    assert_eq!(original.to_chat_string(), file.to_chat_string());
}

#[test]
fn reference_language_switch_rewrite_reaches_a_stable_wire_form() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("canonical reference corpus");
    for fixture in corpus.fixtures() {
        let mut file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        rewrite_whole_utterance_language_switches(&mut file);
        let wire = file.to_chat_string();
        let mut reparsed =
            strict_parse(parser.parse_chat_file(&wire)).expect("rewritten reference parses");
        let stats = rewrite_whole_utterance_language_switches(&mut reparsed);
        assert!(
            stats.is_empty(),
            "rewrite must reach fixed point: {} {stats:?}",
            fixture.path().display()
        );
        assert_eq!(reparsed.to_chat_string(), wire);
    }
}

#[test]
fn missing_language_header_spec_is_not_fabricated_by_switch_rewrite() {
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let parser = TreeSitterParser::new().expect("parser");
    for fixture in ["E504_4.cha", "E504_5.cha"] {
        let input =
            std::fs::read_to_string(root.join(fixture)).expect("canonical header control/mutation");
        let mut file = strict_parse(parser.parse_chat_file(&input)).expect("spec syntax parses");
        let original = file.clone();
        assert!(rewrite_whole_utterance_language_switches(&mut file).is_empty());
        assert!(
            original.semantic_eq(&file),
            "header handling preserves input: {fixture}"
        );
        assert_eq!(original.to_chat_string(), file.to_chat_string());
    }
}
