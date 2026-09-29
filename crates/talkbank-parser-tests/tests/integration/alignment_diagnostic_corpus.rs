//! Canonical grouped alignment policy and user-facing mismatch presentation.

use talkbank_model::alignment::helpers::{
    PositionalDomain, collect_tier_items, count_tier_positions_until, visit_pho_words,
};
use talkbank_model::model::{TranscriptName, WriteChat};
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;

/// Reassemble observed rows through both public accumulation routes. Domain
/// indices and emitted diagnostics come from the alignment producer, never
/// invented placeholder rows. Error-free is only a diagnostic query here.
fn assert_result_construction<A>(
    observed: &A,
    with_pair: fn(A, A::Pair) -> A,
    with_error: fn(A, talkbank_model::ParseError) -> A,
) where
    A: talkbank_model::alignment::TierAlignmentResult + PartialEq + std::fmt::Debug,
{
    use talkbank_model::alignment::IndexPair;
    let mut builder = A::default();
    let mut collector = A::default();
    for pair in observed.pairs() {
        let rebuilt = A::Pair::from_indices(pair.source(), pair.target());
        assert_eq!(rebuilt.source(), pair.source());
        assert_eq!(rebuilt.target(), pair.target());
        let complete = pair.source().is_some() && pair.target().is_some();
        assert_eq!(IndexPair::is_complete(&rebuilt), complete);
        assert_eq!(IndexPair::is_placeholder(&rebuilt), !complete);
        builder = with_pair(builder, rebuilt.clone());
        collector.push_pair(rebuilt);
    }
    for error in observed.errors() {
        builder = with_error(builder, error.clone());
        collector.push_error(error.clone());
    }
    assert_eq!(
        &builder, observed,
        "builder preserves rows and diagnostic order"
    );
    assert_eq!(
        &collector, observed,
        "collector preserves rows and diagnostic order"
    );
    assert_eq!(collector.is_error_free(), observed.errors().is_empty());
}

/// Authored insertion/deletion specs retain the matching prefix and the
/// unmatched side; JSON must preserve the same domain-indexed rows.
#[test]
fn morphology_count_specs_preserve_unmatched_positions() {
    use talkbank_model::alignment::{
        IndexPair, MainWordIndex, MorAlignment, MorItemIndex, TierAlignmentResult,
        align_main_to_mor,
    };

    let parser = TreeSitterParser::new().expect("parser");
    for (spec, expected_main, expected_mor, code) in [
        ("E705_2", 3, 2, ErrorCode::MorCountMismatchTooFew),
        ("E706_1", 2, 3, ErrorCode::MorCountMismatchTooMany),
    ] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{spec}.cha"
        )))
        .expect("canonical morphology count specimen");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax parses");
        let utterance = file.utterances().next().expect("spec utterance");
        let mor = utterance.mor_tier().expect("authored morphology tier");
        assert_eq!(utterance.mor_alignable_word_count().get(), expected_main);
        assert_eq!(mor.items().len(), expected_mor);
        let alignment = align_main_to_mor(&utterance.main, mor);
        assert_eq!(alignment.errors.len(), 1);
        assert_eq!(alignment.errors[0].code, code);
        assert_eq!(alignment.pairs.len(), 3);
        let mut builder = MorAlignment::default();
        let mut collector = MorAlignment::new();
        for (index, pair) in alignment.pairs.iter().enumerate() {
            assert_eq!(
                pair.source_index,
                (index < expected_main).then(|| MainWordIndex::new(index))
            );
            assert_eq!(
                pair.target_index,
                (index < expected_mor).then(|| MorItemIndex::new(index))
            );
            let complete = index < expected_main && index < expected_mor;
            assert_eq!(pair.is_complete(), complete);
            assert_eq!(pair.is_placeholder(), !complete);
            let rebuilt: <MorAlignment as TierAlignmentResult>::Pair =
                IndexPair::from_indices(pair.source(), pair.target());
            assert_eq!(&rebuilt, pair, "generic adapter preserves domain indices");
            builder = builder.with_pair(rebuilt.clone());
            collector.push_pair(rebuilt);
        }
        for error in &alignment.errors {
            builder = builder.with_error(error.clone());
            collector.push_error(error.clone());
        }
        assert_eq!(
            builder, alignment,
            "builder preserves rows and diagnostic order"
        );
        assert_eq!(
            collector, alignment,
            "trait collector preserves all evidence"
        );
        let wire = serde_json::to_vec(&alignment).expect("alignment output");
        let restored: talkbank_model::alignment::MorAlignment =
            serde_json::from_slice(&wire).expect("alignment input");
        assert_eq!(restored, alignment);
    }
}

/// Position labels are a presentation contract, not another alignment counter.
/// The domain type and its existing traversal retain ownership of inclusion.
#[test]
fn grouped_specs_retain_atomic_positions_in_mismatch_diagnostics() {
    use talkbank_model::alignment::{
        PhoAlignment, SinAlignment, TierCountable, align_main_to_pho, align_main_to_sin,
    };
    let parser = TreeSitterParser::new().expect("parser");
    let phonetic = [
        ("non", None),
        ("‹il y a›", Some("phonological group")),
        ("(.)", Some("pause")),
        ("pas", None),
    ];
    let sign = [
        ("0", Some("action")),
        ("foo", None),
        ("0 [=! nods]", Some("action")),
        ("〔bar baz〕", Some("sign group")),
    ];
    for (spec, domain, code, tier, expected) in [
        (
            "E714",
            PositionalDomain::Pho,
            ErrorCode::PhoCountMismatchTooFew,
            "%pho",
            phonetic.as_slice(),
        ),
        (
            "E718",
            PositionalDomain::Sin,
            ErrorCode::SinCountMismatchTooFew,
            "%sin",
            sign.as_slice(),
        ),
    ] {
        for example in [3, 4] {
            let source = std::fs::read_to_string(workspace_root().join(format!(
                "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{spec}_{example}.cha",
            ))).expect("canonical grouped alignment spec");
            let mut file =
                strict_parse(parser.parse_chat_file(&source)).expect("clean grouped syntax");
            let utterance = file.utterances().next().expect("spec utterance");
            match domain {
                PositionalDomain::Pho => {
                    let alignment = align_main_to_pho(
                        &utterance.main,
                        utterance.pho_tier().expect("phonology"),
                    );
                    assert_eq!(alignment.is_error_free(), example == 3);
                    assert_result_construction(
                        &alignment,
                        PhoAlignment::with_pair,
                        PhoAlignment::with_error,
                    );
                }
                PositionalDomain::Sin => {
                    let alignment = align_main_to_sin(
                        &utterance.main,
                        utterance.sin_tier().expect("sign tier"),
                    );
                    assert_eq!(alignment.is_error_free(), example == 3);
                    assert_result_construction(
                        &alignment,
                        SinAlignment::with_pair,
                        SinAlignment::with_error,
                    );
                }
                PositionalDomain::Mor => unreachable!("spoken/sign fixture deck"),
            }
            let positions = utterance.main.content.content.extract_alignable(domain);
            let rendered: Vec<_> = positions
                .iter()
                .map(|item| (item.text.as_str(), item.description.as_deref()))
                .collect();
            assert_eq!(
                rendered, expected,
                "{spec}_{example}: authored domain presentation"
            );
            // Editor offsets count preceding top-level items, not all words
            // inside an atomic group. Morphology deliberately expands groups
            // and excludes pauses/actions instead.
            let content = &utterance.main.content.content;
            assert_eq!(content.len(), 4, "authored top-level items");
            assert_eq!(
                content.count_alignable(domain),
                4,
                "atomic domain positions"
            );
            let mor_prefix = match domain {
                PositionalDomain::Pho => [0, 1, 4, 4, 5],
                PositionalDomain::Sin => [0, 0, 1, 1, 3],
                PositionalDomain::Mor => unreachable!("this deck has spoken/sign controls"),
            };
            // The public typed count describes morphology's expanded view,
            // not the four atomic phonological/sign positions above.
            let mor_count = utterance.mor_alignable_word_count();
            let expected_mor_count = mor_prefix[4];
            assert_eq!(
                content.count_alignable(PositionalDomain::Mor),
                expected_mor_count
            );
            assert_eq!(mor_count.get(), expected_mor_count);
            assert_eq!(mor_count.is_zero(), expected_mor_count == 0);
            assert_eq!(mor_count.to_string(), expected_mor_count.to_string());
            for (index, mor_count) in mor_prefix.into_iter().enumerate() {
                assert_eq!(count_tier_positions_until(content, index, domain), index);
                assert_eq!(
                    count_tier_positions_until(content, index, PositionalDomain::Mor),
                    mor_count,
                    "{spec}_{example}: editor prefix {index}",
                );
            }
            assert_eq!(count_tier_positions_until(content, usize::MAX, domain), 4);
            if domain == PositionalDomain::Pho {
                let mut words = Vec::new();
                visit_pho_words(content, &mut |position| {
                    words.push((position.index(), position.word().to_chat_string()));
                });
                let expected_words: Vec<_> =
                    [(0, "non"), (1, "il"), (1, "y"), (1, "a"), (3, "pas")]
                        .into_iter()
                        .map(|(index, word)| {
                            (
                                talkbank_model::alignment::MainWordIndex::new(index),
                                word.to_owned(),
                            )
                        })
                        .collect();
                assert_eq!(
                    words, expected_words,
                    "group words share a position; a pause has no word"
                );
            }
            let errors = ErrorCollector::new();
            file.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let findings = errors.into_vec();
            if example == 3 {
                assert!(findings.is_empty(), "{spec}: valid control: {findings:?}");
            } else {
                assert_eq!(
                    findings.len(),
                    1,
                    "{spec}: one-token deletion: {findings:?}"
                );
                assert_eq!(findings[0].code, code);
                let message = &findings[0].message;
                assert!(
                    message.starts_with(&format!(
                        "Main tier has 4 alignable items, but {tier} tier has 3 items",
                    )),
                    "{message}"
                );
                for (text, _) in expected {
                    assert!(message.contains(text), "{text}: {message}");
                }
                assert!(
                    message.contains('⊖'),
                    "missing target remains visible: {message}"
                );
            }
            assert_eq!(
                file.to_chat_string(),
                source,
                "diagnostics must not rewrite source"
            );
        }
    }
}

#[test]
fn reference_action_only_turn_has_no_morphology_positions() {
    let parser = TreeSitterParser::new().expect("parser");
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/content/words-special-forms.cha"),
    )
    .expect("reference special forms");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let utterance = file
        .utterances()
        .next()
        .expect("action-only reference turn");
    assert_eq!(
        utterance.main.content.to_content_string(),
        "0 [= did nothing] ."
    );
    let count = utterance.mor_alignable_word_count();
    assert!(count.is_zero());
    assert_eq!(count.get(), 0);
    assert_eq!(count.to_string(), "0");
    assert!(collect_tier_items(&utterance.main.content.content, PositionalDomain::Mor).is_empty());
    // Zero describes the observed main-tier projection, not a fabricated
    // count for an absent dependent tier. The source action remains intact.
    assert_eq!(file.to_chat_string(), source);
}

/// Tag separators, including commas, occupy morphology positions without
/// becoming lexical words. Utterance terminators are not item positions.
#[test]
fn reference_separators_preserve_domain_positions_without_inventing_words() {
    use talkbank_model::alignment::helpers::visit_mor_positions;

    let source =
        std::fs::read_to_string(workspace_root().join("corpus/reference/content/separators.cha"))
            .expect("separator reference");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let expected = [
        vec!["well", ",", "I", "think", "so"],
        vec![
            "it", "was", "lowering", "concrete", "girders", "off", "a", "lorry", "„", "wasn't",
            "it",
        ],
        vec![
            "it", "was", "lowering", "concrete", "girders", "off", "a", "lorry", "‡", "wasn't",
            "it",
        ],
    ];
    let utterances: Vec<_> = file.utterances().collect();
    assert_eq!(utterances.len(), expected.len());
    for (utterance, expected) in utterances.into_iter().zip(expected) {
        let content = &utterance.main.content.content;
        let positions = collect_tier_items(content, PositionalDomain::Mor);
        assert_eq!(
            positions
                .iter()
                .map(|position| position.text.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            positions
                .iter()
                .all(|position| position.description.is_none())
        );
        assert_eq!(utterance.mor_alignable_word_count().get(), expected.len());
        let mut visited = 0;
        visit_mor_positions(content, &mut |position| {
            assert_eq!(
                position.index(),
                talkbank_model::alignment::MainWordIndex::new(visited)
            );
            let spelling = expected[visited];
            if matches!(spelling, "," | "„" | "‡") {
                assert!(position.word().is_none(), "a tag position is not a Word");
            } else {
                assert_eq!(
                    position.word().expect("lexical position").to_chat_string(),
                    spelling
                );
            }
            visited += 1;
        });
        assert_eq!(visited, expected.len());
        let lexical: Vec<_> = expected
            .into_iter()
            .filter(|word| !matches!(*word, "," | "„" | "‡"))
            .collect();
        for domain in [PositionalDomain::Pho, PositionalDomain::Sin] {
            let positions = collect_tier_items(content, domain);
            assert_eq!(
                positions
                    .iter()
                    .map(|position| position.text.as_str())
                    .collect::<Vec<_>>(),
                lexical
            );
        }
    }
    assert_eq!(file.to_chat_string(), source);
}

/// Editor/report consumers must retain missing positions rather than compacting
/// either side of a clitic alignment. Serialization carries observations, not
/// a new certificate that arbitrary imported alignment data is valid.
#[test]
fn clitic_specs_preserve_alignment_gaps_through_public_views_and_json() {
    use talkbank_model::alignment::{
        GraAlignment, IndexPair, TierAlignmentResult, align_mor_to_gra,
    };

    enum RelationCase {
        Matched,
        Missing,
        Extra,
    }
    // Model a batch report assembled from independently inspected files.
    let mut batch = talkbank_model::ParseErrors::default();
    let mut expected_diagnostics = Vec::new();
    let parser = TreeSitterParser::new().expect("parser");
    for (example, case) in [
        (2, RelationCase::Matched),
        (3, RelationCase::Missing),
        (4, RelationCase::Extra),
    ] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E720_{example}.cha",
        )))
        .expect("canonical clitic alignment specimen");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax parses");
        let utterance = file.utterances().next().expect("clitic utterance");
        let alignment = align_mor_to_gra(
            utterance.mor_tier().expect("morphology"),
            utterance.gra_tier().expect("relations"),
        );
        assert_result_construction(
            &alignment,
            GraAlignment::with_pair,
            GraAlignment::with_error,
        );
        let diagnostics = TierAlignmentResult::errors(&alignment).to_vec();
        expected_diagnostics.extend(diagnostics.iter().cloned());
        let collection = talkbank_model::ParseErrors::from(diagnostics);
        match case {
            RelationCase::Matched | RelationCase::Missing => batch.extend(collection),
            RelationCase::Extra => batch.extend_from_vec(collection.into_error_vec()),
        }
        let (common, tail) = match case {
            RelationCase::Matched => (5, None),
            RelationCase::Missing => (4, Some((Some(4), None))),
            RelationCase::Extra => (5, Some((None, Some(5)))),
        };
        let expected: Vec<_> = (0..common)
            .map(|index| (Some(index), Some(index)))
            .chain(tail)
            .collect();
        let wire = serde_json::to_vec(&alignment).expect("alignment JSON output");
        let restored: GraAlignment = serde_json::from_slice(&wire).expect("alignment JSON input");
        assert_eq!(
            restored, alignment,
            "indices, spans and diagnostics survive JSON"
        );
        for observed in [&alignment, &restored] {
            let pairs = TierAlignmentResult::pairs(observed);
            assert_eq!(pairs.len(), expected.len());
            for (pair, &(mor, gra)) in pairs.iter().zip(&expected) {
                assert_eq!((pair.source(), pair.target()), (mor, gra));
                let complete = mor.is_some() && gra.is_some();
                assert_eq!(pair.is_complete(), complete);
                assert_eq!(pair.is_placeholder(), !complete);
                assert_eq!(IndexPair::is_complete(pair), complete);
                assert_eq!(IndexPair::is_placeholder(pair), !complete);
            }
            let errors = TierAlignmentResult::errors(observed);
            let expected_codes = match case {
                RelationCase::Matched => vec![],
                RelationCase::Missing | RelationCase::Extra => vec![ErrorCode::MorGraCountMismatch],
            };
            assert_eq!(
                errors.iter().map(|error| error.code).collect::<Vec<_>>(),
                expected_codes
            );
            assert_eq!(
                observed.is_error_free(),
                matches!(case, RelationCase::Matched)
            );
            assert_eq!(
                TierAlignmentResult::is_error_free(observed),
                observed.is_error_free()
            );
        }
        assert_eq!(
            file.to_chat_string(),
            source,
            "inspection preserves the source model"
        );
    }
    assert_eq!(
        batch.errors, expected_diagnostics,
        "batch retains file order and evidence"
    );
    assert_eq!(
        batch.len(),
        2,
        "missing and extra relations each contribute one error"
    );
    let (errors, warnings) = batch.errors_and_warnings();
    assert_eq!(errors, expected_diagnostics.iter().collect::<Vec<_>>());
    assert!(
        warnings.is_empty(),
        "these specimens emit errors, not warnings"
    );
    let admitted = talkbank_model::CompletedDiagnostics::admit(batch.errors)
        .expect("authored alignment errors are not internal tool failures");
    assert_eq!(admitted.diagnostics(), expected_diagnostics);
    assert_eq!(admitted.into_diagnostics(), expected_diagnostics);
}
