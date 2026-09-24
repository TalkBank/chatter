//! Fragment API equivalence and coordinate contracts over admitted CHAT sources.
//! Models come from parsing; translation compares independent API entry points.

#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{Header, Line, SemanticEq, WriteChat};
use talkbank_model::{ErrorCollector, FragmentSemanticContext, ParseOutcome, SpanShift};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;

#[path = "token_corpus.rs"]
mod token_contracts;

#[test]
fn reference_terminator_tokens_roundtrip_without_claiming_source_provenance() {
    use talkbank_model::alignment::helpers::{ContentItem, walk_content};
    use talkbank_model::model::Terminator;

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut terminators = std::collections::BTreeSet::new();
    let mut separators = std::collections::BTreeSet::new();
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses");
        for utterance in file.utterances() {
            if let Some(expected) = utterance.main.content.terminator.as_ref() {
                let spelling = expected.to_chat_string();
                let actual = Terminator::try_from_chat_str(&spelling)
                    .expect("parsed terminator's canonical spelling is recognized");
                assert!(
                    actual.semantic_eq(expected),
                    "{} {spelling:?}",
                    fixture.path().display()
                );
                assert!(
                    actual.span().is_dummy(),
                    "a free string cannot certify a source span"
                );
                assert_eq!(actual.to_chat_string(), spelling);
                assert!(Terminator::is_chat_terminator(&spelling));
                terminators.insert(spelling);
            }
            walk_content(&utterance.main.content.content, None, &mut |item| {
                if let ContentItem::Separator(separator) = item {
                    let spelling = separator.to_chat_string();
                    assert!(
                        Terminator::try_from_chat_str(&spelling).is_none(),
                        "content separators are not terminators: {spelling:?}"
                    );
                    assert!(!Terminator::is_chat_terminator(&spelling));
                    separators.insert(spelling);
                }
            });
        }
    }
    assert!(
        !terminators.is_empty() && !separators.is_empty(),
        "both admission and refusal require parsed-reference witnesses"
    );
}

/// Check the coordinate wire contract without inventing an expected model or
/// diagnostic code. The fixture runner separately enforces the spec's claim.
fn assert_fragment_rebases<T: SpanShift + PartialEq + std::fmt::Debug>(
    path: &std::path::Path,
    input: &str,
    parse: impl Fn(usize, &ErrorCollector) -> ParseOutcome<T>,
) -> bool {
    let local_errors = ErrorCollector::new();
    let local = parse(0, &local_errors);
    let shifted_errors = ErrorCollector::new();
    let shifted = parse(17, &shifted_errors);
    match (local, shifted) {
        (ParseOutcome::Parsed(mut expected), ParseOutcome::Parsed(actual)) => {
            expected.shift_spans_after(0, 17);
            assert_eq!(actual, expected, "model: {} {input:?}", path.display());
        }
        (ParseOutcome::Rejected, ParseOutcome::Rejected) => {
            assert!(
                !local_errors.is_empty(),
                "rejection needs evidence: {}",
                path.display()
            );
        }
        _ => panic!("offset changed admission: {} {input:?}", path.display()),
    }
    let mut expected_errors = local_errors.to_vec();
    for error in &mut expected_errors {
        let span = error.location.span;
        assert!(
            input.get(span.start as usize..span.end as usize).is_some(),
            "diagnostic must address caller bytes: {} {input:?} {error:?}",
            path.display()
        );
        error.location.span.shift_spans_after(0, 17);
        for label in &mut error.labels {
            let span = label.span;
            assert!(
                input.get(span.start as usize..span.end as usize).is_some(),
                "label must address caller bytes: {} {input:?} {label:?}",
                path.display()
            );
            label.span.shift_spans_after(0, 17);
        }
        // Retained diagnostic context remains snippet-relative.
    }
    assert_eq!(
        shifted_errors.to_vec(),
        expected_errors,
        "diagnostics: {} {input:?}",
        path.display()
    );
    !expected_errors.is_empty()
}

#[test]
fn spec_main_tier_fragments_preserve_recovery_under_rebasing() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical spec fixtures");
    let mut recovered = 0;
    for fixture in corpus.fixtures() {
        let file_errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(fixture.source(), &file_errors);
        for utterance in file.utterances() {
            let span = utterance.main.span;
            let input = fixture
                .source()
                .get(span.start as usize..span.end as usize)
                .expect("retained main tier belongs to its spec source");
            // The fragment adapter may append a synthetic terminator. Exercise
            // both byte forms from the same canonical source, including recovery
            // diagnostics whose end must not expose that appended newline.
            for input in [input, input.trim_end_matches('\n')] {
                recovered += usize::from(assert_fragment_rebases(
                    fixture.path(),
                    input,
                    |offset, errors| parser.parse_main_tier_fragment(input, offset, errors),
                ));
            }
        }
    }
    assert!(recovered > 0, "spec corpus must exercise fragment recovery");
}

#[test]
fn reference_fragment_boundaries_refuse_extra_headers_and_wrong_tier_kinds() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut witnesses = [0; 3];
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        for pair in file.lines.windows(2) {
            let [
                Line::Header { span: first, .. },
                Line::Header { span: second, .. },
            ] = pair
            else {
                continue;
            };
            let input = fixture
                .source()
                .get(first.start as usize..second.end as usize)
                .expect("adjacent headers belong to the same parsed source");
            assert_fragment_rebases(fixture.path(), input, |offset, errors| {
                let outcome = parser.parse_header_fragment(input, offset, errors);
                assert!(
                    matches!(outcome, ParseOutcome::Rejected),
                    "one-header API accepted two headers: {} {input:?}",
                    fixture.path().display()
                );
                outcome
            });
            witnesses[0] += 1;
        }
        for line in &file.lines {
            match line {
                Line::Header { header, span, .. } => {
                    if matches!(header.as_ref(), Header::ID(_)) {
                        continue;
                    }
                    let input = fixture
                        .source()
                        .get(span.start as usize..span.end as usize)
                        .expect("header belongs to parsed source");
                    // The ID adapter selects a typed value, not CHAT validity:
                    // a valid non-ID header has no ID and no syntax diagnostic.
                    for offset in [0, 17] {
                        let errors = ErrorCollector::new();
                        let outcome = parser.parse_id_header_fragment(input, offset, &errors);
                        assert!(
                            matches!(outcome, ParseOutcome::Rejected),
                            "ID API accepted another header kind: {} {input:?}",
                            fixture.path().display()
                        );
                        assert!(
                            errors.is_empty(),
                            "valid header was diagnosed: {} {input:?}: {:?}",
                            fixture.path().display(),
                            errors.to_vec()
                        );
                    }
                    witnesses[1] += 1;
                }
                Line::Utterance(utterance) => {
                    let span = utterance.main.span;
                    let input = fixture
                        .source()
                        .get(span.start as usize..span.end as usize)
                        .expect("main tier belongs to parsed source");
                    assert_fragment_rebases(fixture.path(), input, |offset, errors| {
                        let outcome = parser.parse_header_fragment(input, offset, errors);
                        assert!(
                            matches!(outcome, ParseOutcome::Rejected),
                            "header API accepted speech: {} {input:?}",
                            fixture.path().display()
                        );
                        outcome
                    });
                    witnesses[2] += 1;
                }
            }
        }
    }
    assert!(
        witnesses.iter().all(|count| *count > 0),
        "every refusal needs reference witnesses: {witnesses:?}"
    );
}

#[test]
fn spec_header_and_dependent_fragments_preserve_recovery_under_rebasing() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical spec fixtures");
    let mut recovered = [0; 2];
    for fixture in corpus.fixtures() {
        let file_errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(fixture.source(), &file_errors);
        for line in &file.lines {
            match line {
                Line::Header { span, .. } => {
                    let input = fixture
                        .source()
                        .get(span.start as usize..span.end as usize)
                        .expect("retained header belongs to its spec source");
                    recovered[0] += usize::from(assert_fragment_rebases(
                        fixture.path(),
                        input,
                        |offset, errors| parser.parse_header_fragment(input, offset, errors),
                    ));
                }
                Line::Utterance(utterance) => {
                    for entry in &utterance.dependent_tiers {
                        let span = entry.span();
                        let input = fixture
                            .source()
                            .get(span.start as usize..span.end as usize)
                            .expect("retained dependent tier belongs to its spec source");
                        recovered[1] += usize::from(assert_fragment_rebases(
                            fixture.path(),
                            input,
                            |offset, errors| {
                                parser.parse_dependent_tier_fragment(input, offset, errors)
                            },
                        ));
                    }
                }
            }
        }
    }
    assert!(
        recovered.iter().all(|count| *count > 0),
        "each API needs spec recovery witnesses: {recovered:?}"
    );
}

#[test]
fn spec_documents_preserve_utterance_adapter_recovery_under_rebasing() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical spec fixtures");
    let mut recovered = 0;
    for fixture in corpus.fixtures() {
        // The legacy utterance adapter explicitly supports full documents as
        // well as fragments. Keep that real input boundary covered, including
        // documents from which no complete utterance can be admitted.
        for input in [fixture.source(), fixture.source().trim_end_matches('\n')] {
            recovered += usize::from(assert_fragment_rebases(
                fixture.path(),
                input,
                |offset, errors| parser.parse_utterance_fragment(input, offset, errors),
            ));
        }
    }
    assert!(
        recovered > 0,
        "spec documents must exercise utterance adapter recovery"
    );
}

#[test]
fn reference_participants_roundtrip_through_entry_fragment_api() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut compared = 0;
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        for line in &file.lines {
            let Line::Header { header, .. } = line else {
                continue;
            };
            let Header::Participants { entries } = header.as_ref() else {
                continue;
            };
            for entry in entries.iter() {
                let input = entry.to_chat_string();
                for offset in [0, 17] {
                    let errors = ErrorCollector::new();
                    let ParseOutcome::Parsed(actual) =
                        parser.parse_participant_entry_fragment(&input, offset, &errors)
                    else {
                        panic!("entry rejected: {} {input:?}", fixture.path().display());
                    };
                    assert!(
                        errors.is_empty(),
                        "{} {input:?}: {:?}",
                        fixture.path().display(),
                        errors.to_vec()
                    );
                    assert!(
                        actual.semantic_eq(entry),
                        "entry: {} {input:?}",
                        fixture.path().display()
                    );
                    compared += 1;
                }
            }
        }
    }
    assert!(
        compared > 0,
        "reference population must contain participants"
    );
}

#[test]
fn reference_words_preserve_spelling_and_coordinates_through_fragment_api() {
    use talkbank_model::alignment::helpers::{WordItem, walk_words};

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut compared = 0;
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        let ca_mode = file.lines.iter().any(|line| matches!(line,
            Line::Header { header, .. } if matches!(header.as_ref(),
                Header::Options { options } if options.iter().any(|option|
                    option.has_effect(talkbank_model::model::CaOptionEffect::ParentheticalIsCaOmission)))));
        for line in &file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => &replaced.word,
                    WordItem::Separator(_) => return,
                };
                let input = fixture
                    .source()
                    .get(word.span.start as usize..word.span.end as usize)
                    .expect("parsed word belongs to reference source");
                let errors = ErrorCollector::new();
                let parsed = parser.parse_word_fragment(input, word.span.start as usize, &errors);
                assert!(
                    errors.is_empty(),
                    "{} {input:?}: {:?}",
                    fixture.path().display(),
                    errors.to_vec()
                );
                let ParseOutcome::Parsed(mut actual) = parsed else {
                    panic!("word rejected: {} {input:?}", fixture.path().display());
                };
                assert_eq!(
                    actual.span,
                    word.span,
                    "coordinates: {}",
                    fixture.path().display()
                );
                assert_eq!(
                    actual.raw_text(),
                    word.raw_text(),
                    "spelling: {}",
                    fixture.path().display()
                );
                // The standalone API has no enclosing @Options context. Its
                // caller applies the shared contextual interpretation explicitly.
                if ca_mode {
                    talkbank_model::model::content::word::ca::normalize_ca_omission_word(
                        &mut actual,
                    );
                }
                assert!(
                    actual.semantic_eq(word),
                    "word semantics: {} {input:?}",
                    fixture.path().display()
                );
                compared += 1;
            });
        }
    }
    assert!(compared > 0, "reference population must contain words");
}

#[test]
fn reference_dependent_tiers_preserve_semantics_through_standalone_api() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut compared = 0;
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        for line in &file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            for tier in &utterance.dependent_tiers {
                // Exercise the public wire-format boundary, not a fabricated AST.
                let input = tier.to_chat_string();
                let actual = parser
                    .parse_tiers(&input)
                    .expect("serialized reference dependent tier parses cleanly");
                assert!(
                    actual.semantic_eq(&tier.tier),
                    "tier: {} {input:?}",
                    fixture.path().display()
                );
                compared += 1;
            }
        }
    }
    assert!(
        compared > 0,
        "reference population must contain dependent tiers"
    );
}

#[test]
fn file_fragments_preserve_models_and_snippet_relative_diagnostics() {
    let parser = TreeSitterParser::new().expect("parser");
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("admitted corpus");
        for fixture in corpus.fixtures() {
            let baseline_errors = ErrorCollector::new();
            let baseline = parser.parse_chat_file_streaming(fixture.source(), &baseline_errors);
            for offset in [0, 17] {
                let errors = ErrorCollector::new();
                let ParseOutcome::Parsed(actual) =
                    parser.parse_chat_file_fragment(fixture.source(), offset, &errors)
                else {
                    panic!(
                        "representable file fragment rejected: {}",
                        fixture.path().display()
                    );
                };
                let mut expected = baseline.clone();
                expected.shift_spans_after(0, offset as i32);
                assert_eq!(
                    actual,
                    expected,
                    "model: {} at {offset}",
                    fixture.path().display()
                );
                let mut expected_errors = baseline_errors.to_vec();
                for error in &mut expected_errors {
                    error.location.span.shift_spans_after(0, offset as i32);
                    for label in &mut error.labels {
                        label.span.shift_spans_after(0, offset as i32);
                    }
                    // Context points into its retained snippet, not the caller's
                    // enclosing document; do not shift those coordinates.
                }
                assert_eq!(
                    errors.to_vec(),
                    expected_errors,
                    "diagnostics: {} at {offset}",
                    fixture.path().display()
                );
            }
        }
    }
}

#[test]
fn reference_dependent_tier_fragments_preserve_source_models() {
    use talkbank_model::model::DependentTier;
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut compared = 0;
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        for line in &file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            for entry in &utterance.dependent_tiers {
                let span = entry.span();
                let input = fixture
                    .source()
                    .get(span.start as usize..span.end as usize)
                    .expect("tier source span");
                let errors = ErrorCollector::new();
                let ParseOutcome::Parsed(actual) =
                    parser.parse_dependent_tier_fragment(input, span.start as usize, &errors)
                else {
                    panic!("tier rejected: {} {input:?}", fixture.path().display());
                };
                assert!(
                    errors.is_empty(),
                    "{}: {:?}",
                    fixture.path().display(),
                    errors.to_vec()
                );
                assert!(
                    actual.semantic_eq(&entry.tier),
                    "tier semantics: {} {input:?}",
                    fixture.path().display()
                );
                assert_eq!(
                    actual.span(),
                    span,
                    "tier coordinates: {}",
                    fixture.path().display()
                );

                let body_span = entry
                    .content_span()
                    .expect("reference tier has content span");
                let body = fixture
                    .source()
                    .get(body_span.start as usize..body_span.end as usize)
                    .expect("tier content belongs to reference source");
                // Only tier variants with a public content-only entry point
                // participate here; the generic API above covers every variant.
                macro_rules! compare_content {
                    ($variant:ident, $method:ident) => {
                        if let DependentTier::$variant(expected) = &entry.tier {
                            let errors = ErrorCollector::new();
                            let ParseOutcome::Parsed(actual) =
                                parser.$method(body, body_span.start as usize, &errors)
                            else {
                                panic!("content rejected: {} {body:?}", fixture.path().display());
                            };
                            assert!(
                                errors.is_empty(),
                                "{}: {:?}",
                                fixture.path().display(),
                                errors.to_vec()
                            );
                            assert!(
                                actual.semantic_eq(expected),
                                "content semantics: {} {body:?}",
                                fixture.path().display()
                            );
                            compared += 1;
                        }
                    };
                }
                compare_content!(Mor, parse_mor_tier_fragment);
                compare_content!(Gra, parse_gra_tier_fragment);
                compare_content!(Pho, parse_pho_tier_fragment);
                compare_content!(Sin, parse_sin_tier_fragment);
                compare_content!(Act, parse_act_tier_fragment);
                compare_content!(Cod, parse_cod_tier_fragment);
                compare_content!(Com, parse_com_tier_fragment);
                compare_content!(Exp, parse_exp_tier_fragment);
                compare_content!(Add, parse_add_tier_fragment);
                compare_content!(Gpx, parse_gpx_tier_fragment);
                compare_content!(Int, parse_int_tier_fragment);
                compare_content!(Spa, parse_spa_tier_fragment);
                compare_content!(Sit, parse_sit_tier_fragment);
                compare_content!(Wor, parse_wor_tier_fragment);
            }
        }
    }
    assert!(
        compared > 0,
        "reference corpus must exercise content-only tier APIs"
    );
}

#[test]
fn invalid_timing_states_from_specs_rebase_to_document_coordinates() {
    use talkbank_model::model::DependentTier;
    let parser = TreeSitterParser::new().expect("parser");
    for source in [
        include_str!("../error_corpus/validation_errors/E603_1.cha"),
        include_str!("../error_corpus/validation_errors/E756_4.cha"),
    ] {
        // These fixtures are invalid at validation, not at parsing. The
        // recovery model must preserve their declared timing state and location.
        let file = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(source))
            .expect("timing spec is syntactically parseable");
        let mut compared = 0;
        for line in &file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            for entry in &utterance.dependent_tiers {
                let DependentTier::Tim(expected) = &entry.tier else {
                    continue;
                };
                let span = entry.span();
                let input = source
                    .get(span.start as usize..span.end as usize)
                    .expect("tim source span");
                let errors = ErrorCollector::new();
                let ParseOutcome::Parsed(DependentTier::Tim(actual)) =
                    parser.parse_dependent_tier_fragment(input, span.start as usize, &errors)
                else {
                    panic!("timing state must survive fragment parsing");
                };
                assert!(errors.is_empty(), "{:?}", errors.to_vec());
                assert_eq!(&actual, expected, "timing state and document coordinates");
                compared += 1;
            }
        }
        assert!(compared > 0, "timing spec must exercise a timing tier");
    }
}

#[test]
fn reference_dependent_items_roundtrip_through_fragment_apis() {
    use talkbank_model::model::{DependentTier, PhoItem};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut counts = [0; 3];
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        macro_rules! compare_item {
            ($expected:expr, $method:ident, $counter:expr) => {{
                let expected = $expected;
                let input = expected.to_chat_string();
                for offset in [0, 17] {
                    let errors = ErrorCollector::new();
                    let ParseOutcome::Parsed(actual) = parser.$method(&input, offset, &errors)
                    else {
                        panic!("item rejected: {} {input:?}", fixture.path().display());
                    };
                    assert!(
                        errors.is_empty(),
                        "{} {input:?}: {:?}",
                        fixture.path().display(),
                        errors.to_vec()
                    );
                    assert!(
                        actual.semantic_eq(expected),
                        "item semantics: {} {input:?}",
                        fixture.path().display()
                    );
                }
                counts[$counter] += 1;
            }};
        }
        for line in &file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            for entry in &utterance.dependent_tiers {
                match &entry.tier {
                    DependentTier::Mor(tier) => {
                        for item in tier.items() {
                            for word in std::iter::once(&item.main).chain(item.post_clitics.iter())
                            {
                                compare_item!(word, parse_mor_word_fragment, 0);
                            }
                        }
                    }
                    DependentTier::Gra(tier) => {
                        for relation in tier.relations() {
                            compare_item!(relation, parse_gra_relation_fragment, 1);
                        }
                    }
                    DependentTier::Pho(tier) => {
                        for item in tier.items.iter() {
                            match item {
                                PhoItem::Word(word) => {
                                    compare_item!(word, parse_pho_word_fragment, 2);
                                }
                                PhoItem::Group(words) => {
                                    for word in words.iter() {
                                        compare_item!(word, parse_pho_word_fragment, 2);
                                    }
                                }
                            }
                        }
                    }
                    // Other dependent-tier kinds have no item API in this test;
                    // their complete tiers are exercised above.
                    _ => {}
                }
            }
        }
    }
    assert!(
        counts.iter().all(|count| *count > 0),
        "each item API needs corpus witnesses: {counts:?}"
    );
}

#[test]
fn duplicate_end_spec_keeps_its_diagnostic_with_or_without_final_newline() {
    let parser = TreeSitterParser::new().expect("parser");
    let fixture = include_str!("../error_corpus/validation_errors/E501_5.cha");
    for source in [fixture, fixture.trim_end_matches('\n')] {
        for offset in [0, 17] {
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(file) =
                parser.parse_chat_file_fragment(source, offset, &errors)
            else {
                panic!("duplicate End still retains the document");
            };
            assert_eq!(file.utterances().count(), 1);
            let diagnostics = errors.to_vec();
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
            assert_eq!(
                diagnostics[0].code,
                talkbank_model::ErrorCode::DuplicateHeader
            );
            let span = diagnostics[0].location.span;
            let text = source
                .get((span.start as usize - offset)..(span.end as usize - offset))
                .expect("duplicate header diagnostic belongs to caller source");
            assert_eq!(text.trim_end_matches('\n'), "@End");
        }
    }
}

#[test]
fn tier_and_header_fragments_preserve_reference_models_with_file_context() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut compared = 0;
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        let flags = file
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Header { header, .. } => match header.as_ref() {
                    Header::Options { options } => Some(options.iter().cloned()),
                    _ => None,
                },
                Line::Utterance(_) => None,
            })
            .flatten()
            .collect();
        let context = FragmentSemanticContext::new().with_option_flags(flags);
        for line in &file.lines {
            let utterance = match line {
                Line::Header { header, span, .. } => {
                    let input = fixture
                        .source()
                        .get(span.start as usize..span.end as usize)
                        .expect("parsed header span belongs to fixture");
                    let errors = ErrorCollector::new();
                    let parsed = parser.parse_header_fragment(input, span.start as usize, &errors);
                    assert!(
                        errors.is_empty(),
                        "header {}: {:?}",
                        fixture.path().display(),
                        errors.to_vec()
                    );
                    let ParseOutcome::Parsed(actual) = parsed else {
                        panic!("header rejected: {}", fixture.path().display());
                    };
                    assert_eq!(
                        &actual,
                        header.as_ref(),
                        "header: {}",
                        fixture.path().display()
                    );
                    if let Header::ID(expected) = header.as_ref() {
                        let errors = ErrorCollector::new();
                        let ParseOutcome::Parsed(actual) =
                            parser.parse_id_header_fragment(input, span.start as usize, &errors)
                        else {
                            panic!("ID header rejected: {}", fixture.path().display());
                        };
                        assert!(
                            errors.is_empty(),
                            "ID {}: {:?}",
                            fixture.path().display(),
                            errors.to_vec()
                        );
                        assert!(
                            actual.semantic_eq(expected),
                            "ID header: {}",
                            fixture.path().display()
                        );
                    }
                    continue;
                }
                Line::Utterance(utterance) => utterance,
            };
            let main = &utterance.main;
            let input = fixture
                .source()
                .get(main.span.start as usize..main.span.end as usize)
                .expect("parsed main-tier span belongs to fixture");
            let errors = ErrorCollector::new();
            let parsed = parser.parse_main_tier_fragment_with_context(
                input,
                main.span.start as usize,
                &context,
                &errors,
            );
            assert!(
                errors.is_empty(),
                "{}: {:?}",
                fixture.path().display(),
                errors.to_vec()
            );
            let ParseOutcome::Parsed(actual) = parsed else {
                panic!("main tier rejected: {}", fixture.path().display());
            };
            assert_eq!(&actual, main, "main tier: {}", fixture.path().display());
            let serialized = utterance.to_chat_string();
            let errors = ErrorCollector::new();
            let parsed =
                parser.parse_utterance_fragment_with_context(&serialized, 0, &context, &errors);
            assert!(
                errors.is_empty(),
                "utterance {}: {:?}",
                fixture.path().display(),
                errors.to_vec()
            );
            let ParseOutcome::Parsed(actual) = parsed else {
                panic!("utterance rejected: {}", fixture.path().display());
            };
            assert!(
                actual.semantic_eq(utterance.as_ref()),
                "utterance: {}",
                fixture.path().display()
            );
            compared += 1;
        }
    }
    assert!(compared > 0, "reference population must contain main tiers");
}
