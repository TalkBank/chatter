//! Language-switch rewrites grounded in authored specs and reference CHAT.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{SemanticEq, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{
    chat_corpus::ChatCorpus, repo_paths::workspace_root, test_error::strict_parse,
};
use talkbank_transform::fix_s::rewrite_whole_utterance_language_switches;

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
    assert_eq!(controls.len(), 32);
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
    use talkbank_model::model::{Line, WordLanguages};

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
        let mut actual = Vec::new();
        for line in &mut file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            assert!(utterance.language_metadata.is_uncomputed());
            utterance.compute_language_metadata(file.languages.first(), &file.languages);
            let metadata = utterance
                .language_metadata
                .as_computed()
                .expect("explicit metadata computation completed");
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
