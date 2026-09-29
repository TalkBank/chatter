//! Fragment API equivalence and coordinate contracts over admitted CHAT sources.
//! Models come from parsing; translation compares independent API entry points.
//! Fragment calls use the public `ChatParser` contract while retaining the
//! source-bound outcomes, coordinate checks and independent streaming baselines.

#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{Header, Line, SemanticEq, WriteChat};
use talkbank_model::{
    ChatParser, ErrorCollector, FragmentSemanticContext, ParseOutcome, SpanShift,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;

#[path = "token_corpus.rs"]
mod token_contracts;

#[path = "tier_construction_corpus.rs"]
mod tier_construction_contracts;

#[path = "overlap_onset_corpus.rs"]
mod overlap_onset_contracts;

#[path = "gem_label_corpus.rs"]
mod gem_label_contracts;

#[path = "pause_corpus.rs"]
mod pause_contracts;

#[path = "header_boundary_corpus.rs"]
mod header_boundary_contracts;

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
                // A caller can explicitly attach the original parser-owned span;
                // lexical recognition alone above still carries no provenance.
                assert_eq!(actual.with_span(expected.span()), *expected);
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
        for label in &mut error.labels {
            let span = label.span;
            assert!(
                input.get(span.start as usize..span.end as usize).is_some(),
                "label must address caller bytes: {} {input:?} {label:?}",
                path.display()
            );
        }
        // Retained diagnostic context remains snippet-relative.
        error.shift_spans_after(0, 17);
    }
    assert_eq!(
        shifted_errors.to_vec(),
        expected_errors,
        "diagnostics: {} {input:?}",
        path.display()
    );
    let mut restored_errors = shifted_errors.to_vec();
    restored_errors.shift_spans_after(17, -17);
    assert_eq!(
        restored_errors,
        local_errors.to_vec(),
        "inverse diagnostic rebase: {}",
        path.display(),
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
                    |offset, errors| ChatParser::parse_main_tier(&parser, input, offset, errors),
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
            assert!(
                !parser
                    .parse_header(input)
                    .expect_err("strict one-header API must refuse adjacent reference headers")
                    .is_empty()
            );
            assert_fragment_rebases(fixture.path(), input, |offset, errors| {
                let outcome = ChatParser::parse_header(&parser, input, offset, errors);
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
                    let header_input = fixture
                        .source()
                        .get(span.start as usize..span.end as usize)
                        .expect("reference header source");
                    assert!(
                        !parser
                            .parse_main_tier(header_input)
                            .expect_err("strict main-tier API must refuse a reference header")
                            .is_empty()
                    );
                    assert!(
                        !parser
                            .parse_word(header_input)
                            .expect_err("strict word API must refuse a reference header")
                            .is_empty()
                    );
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
                        let outcome = ChatParser::parse_id_header(&parser, input, offset, &errors);
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
                    assert!(
                        !parser
                            .parse_word(input)
                            .expect_err("strict word API must refuse an entire reference main tier")
                            .is_empty()
                    );
                    assert_fragment_rebases(fixture.path(), input, |offset, errors| {
                        let outcome = ChatParser::parse_header(&parser, input, offset, errors);
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
                        |offset, errors| ChatParser::parse_header(&parser, input, offset, errors),
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
                                ChatParser::parse_dependent_tier(&parser, input, offset, errors)
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
                |offset, errors| ChatParser::parse_utterance(&parser, input, offset, errors),
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
    use talkbank_parser::generated_traversal::{FromNodeKind, ParticipantNode};

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut compared = 0;
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        let parsed_source = parser
            .parse_source_incremental(fixture.source(), None)
            .expect("reference CST");
        let mut pending = vec![parsed_source.root_node()];
        let mut ranges = Vec::new();
        while let Some(node) = pending.pop() {
            if ParticipantNode::from_node(node).is_some() {
                ranges.push(node.byte_range());
            }
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
        }
        ranges.sort_by_key(|range| range.start);
        let entries: Vec<_> = file
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Header { header, .. } => match header.as_ref() {
                    Header::Participants { entries } => Some(entries.iter()),
                    _ => None,
                },
                Line::Utterance(_) => None,
            })
            .flatten()
            .collect();
        assert_eq!(
            ranges.len(),
            entries.len(),
            "{}: CST/AST participant census",
            fixture.path().display()
        );
        let context = FragmentSemanticContext::new();
        for (entry, range) in entries.into_iter().zip(ranges) {
            let input = fixture
                .source()
                .get(range.clone())
                .expect("participant source span");
            for offset in [0, range.start, range.start + 17] {
                let errors = ErrorCollector::new();
                let ParseOutcome::Parsed(actual) =
                    ChatParser::parse_participant_entry(&parser, input, offset, &errors)
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
                let contextual_errors = ErrorCollector::new();
                let ParseOutcome::Parsed(contextual) =
                    ChatParser::parse_participant_entry_with_context(
                        &parser,
                        input,
                        offset,
                        &context,
                        &contextual_errors,
                    )
                else {
                    panic!(
                        "contextual entry rejected: {} {input:?}",
                        fixture.path().display()
                    );
                };
                assert!(
                    contextual_errors.is_empty(),
                    "{:?}",
                    contextual_errors.to_vec()
                );
                assert_eq!(contextual, actual, "context-free participant semantics");
                compared += 1;
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
    let mut overlap_lexical_witnesses = std::collections::BTreeSet::new();
    let mut replacement_lexical_witnesses = 0;
    let mut ca_omission_witnesses = 0;
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        let ca_mode = file.lines.iter().any(|line| matches!(line,
            Line::Header { header, .. } if matches!(header.as_ref(),
                Header::Options { options } if options.iter().any(|option|
                    option.has_effect(talkbank_model::model::CaOptionEffect::ParentheticalIsCaOmission)))));
        let context = if ca_mode {
            FragmentSemanticContext::new().with_option_flag(talkbank_model::ChatOptionFlag::Ca)
        } else {
            FragmentSemanticContext::new()
        };
        for line in &file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => {
                        if fixture
                            .path()
                            .ends_with("annotation/errors-and-replacements.cha")
                            && replaced.word.raw_text() == "child"
                        {
                            assert_eq!(replaced.word.cleaned_text(), "child");
                            assert!(!replaced.replacement.words.is_empty());
                            for replacement in &replaced.replacement.words {
                                assert!(!replacement.cleaned_text().contains(['[', ']']));
                            }
                            replacement_lexical_witnesses += 1;
                        }
                        &replaced.word
                    }
                    WordItem::Separator(_) => return,
                };
                let input = fixture
                    .source()
                    .get(word.span.start as usize..word.span.end as usize)
                    .expect("parsed word belongs to reference source");
                let errors = ErrorCollector::new();
                let parsed =
                    ChatParser::parse_word(&parser, input, word.span.start as usize, &errors);
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
                if fixture.path().ends_with("ca/overlaps.cha") {
                    let expected = match word.raw_text().as_str() {
                        "b⌉" => Some("b"),
                        "h⌋" => Some("h"),
                        _ => None,
                    };
                    if let Some(expected) = expected {
                        assert_eq!(word.cleaned_text(), expected);
                        assert_eq!(actual.cleaned_text(), expected);
                        overlap_lexical_witnesses.insert(expected);
                    }
                }
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
                let contextual_errors = ErrorCollector::new();
                let ParseOutcome::Parsed(contextual) = ChatParser::parse_word_with_context(
                    &parser,
                    input,
                    word.span.start as usize,
                    &context,
                    &contextual_errors,
                ) else {
                    panic!(
                        "contextual word rejected: {} {input:?}",
                        fixture.path().display()
                    );
                };
                assert!(
                    contextual_errors.is_empty(),
                    "{:?}",
                    contextual_errors.to_vec()
                );
                assert_eq!(contextual.span, word.span);
                assert_eq!(contextual.raw_text(), word.raw_text());
                assert!(
                    contextual.semantic_eq(word),
                    "contextual word semantics: {} {input:?}",
                    fixture.path().display()
                );
                if matches!(
                    word.category,
                    Some(talkbank_model::model::WordCategory::CAOmission)
                ) {
                    use talkbank_model::validation::{Validate, ValidationContext};
                    assert!(ca_mode, "reference CA omissions retain their file context");
                    for enabled in [true, false] {
                        let errors = ErrorCollector::new();
                        contextual
                            .validate(&ValidationContext::default().with_ca_mode(enabled), &errors);
                        let findings = errors.into_vec();
                        let invalid_format: Vec<_> = findings
                            .iter()
                            .filter(|error| {
                                error.code == talkbank_model::ErrorCode::InvalidWordFormat
                            })
                            .collect();
                        assert_eq!(invalid_format.len(), usize::from(!enabled), "{findings:?}");
                        if let Some(finding) = invalid_format.first() {
                            assert_eq!(finding.location.span, word.span);
                            assert_eq!(
                                finding.message,
                                "CA omission '(word)' used outside CA mode"
                            );
                        }
                    }
                    assert!(
                        contextual.semantic_eq(word),
                        "validation must not rewrite the omission"
                    );
                    ca_omission_witnesses += 1;
                }
                compared += 1;
            });
        }
    }
    assert!(compared > 0, "reference population must contain words");
    assert!(
        ca_omission_witnesses > 0,
        "CA context contract needs reference witnesses"
    );
    assert_eq!(overlap_lexical_witnesses, ["b", "h"].into_iter().collect());
    assert_eq!(
        replacement_lexical_witnesses, 2,
        "both authored child replacements must be exercised"
    );
}

#[test]
fn contextual_word_options_do_not_turn_a_reference_tier_into_a_word() {
    let source = include_str!("../../../../corpus/reference/ca/nonvocal-and-long-features.cha");
    let parser = TreeSitterParser::new().expect("parser");
    let file = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(source))
        .expect("reference parses");
    let span = file
        .utterances()
        .next()
        .expect("reference speech")
        .main
        .span;
    let input = source
        .get(span.start as usize..span.end as usize)
        .expect("main-tier source");
    for offset in [0, span.start as usize, span.start as usize + 17] {
        let errors = ErrorCollector::new();
        let baseline = ChatParser::parse_word(&parser, input, offset, &errors);
        assert!(matches!(baseline, ParseOutcome::Rejected));
        assert!(!errors.is_empty());
        for context in [
            FragmentSemanticContext::new(),
            FragmentSemanticContext::new().with_option_flag(talkbank_model::ChatOptionFlag::Ca),
        ] {
            let contextual_errors = ErrorCollector::new();
            let actual = ChatParser::parse_word_with_context(
                &parser,
                input,
                offset,
                &context,
                &contextual_errors,
            );
            assert_eq!(actual, baseline);
            assert_eq!(contextual_errors.to_vec(), errors.to_vec());
        }
    }
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
                let span = tier.span();
                let input = fixture
                    .source()
                    .get(span.start as usize..span.end as usize)
                    .expect("dependent tier belongs to reference source");
                let actual = parser
                    .parse_tiers(input)
                    .expect("source-backed reference dependent tier parses cleanly");
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
                    ChatParser::parse_chat_file(&parser, fixture.source(), offset, &errors)
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
    use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
    use talkbank_model::{RuleSelection, TranscriptName};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut compared = 0;
    let context = FragmentSemanticContext::new();
    let mut contextual_variants = std::collections::BTreeSet::new();
    let mut morphology_whitespace_controls = [0; 2];
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        if file
            .utterances()
            .any(|utterance| utterance.mor_tier().is_some_and(|tier| tier.is_empty()))
        {
            let errors = ErrorCollector::new();
            file.clone()
                .validate_with_policy(
                    ValidationPolicy::new(
                        RuleSelection::new(),
                        AlignmentValidation::IncludeTierAlignment,
                    ),
                    &errors,
                    TranscriptName::for_path(fixture.path()),
                )
                .expect("terminator-only reference requires validation, not just parsing");
            assert!(errors.is_empty(), "{:?}", errors.to_vec());
        }
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
                    ChatParser::parse_dependent_tier(&parser, input, span.start as usize, &errors)
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
                let contextual_errors = ErrorCollector::new();
                let contextual = ChatParser::parse_dependent_tier_with_context(
                    &parser,
                    input,
                    span.start as usize,
                    &context,
                    &contextual_errors,
                );
                assert_eq!(contextual, ParseOutcome::Parsed(actual));
                assert_eq!(contextual_errors.to_vec(), errors.to_vec());

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
                    ($variant:ident, $method:ident, $contextual:ident) => {
                        if let DependentTier::$variant(expected) = &entry.tier {
                            let errors = ErrorCollector::new();
                            let ParseOutcome::Parsed(actual) = ChatParser::$method(
                                &parser,
                                body,
                                body_span.start as usize,
                                &errors,
                            ) else {
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
                            let contextual_errors = ErrorCollector::new();
                            let contextual = ChatParser::$contextual(
                                &parser,
                                body,
                                body_span.start as usize,
                                &context,
                                &contextual_errors,
                            );
                            assert_eq!(
                                contextual,
                                ParseOutcome::Parsed(actual),
                                "context adapter: {} {body:?}",
                                fixture.path().display()
                            );
                            assert_eq!(contextual_errors.to_vec(), errors.to_vec());
                            contextual_variants.insert(stringify!($variant));
                            compared += 1;
                        }
                    };
                }
                compare_content!(Mor, parse_mor_tier, parse_mor_tier_with_context);
                if let DependentTier::Mor(expected) = &entry.tier {
                    // Count an observed tier, never substitute zero for absence.
                    // Existing witnesses require both lexical and terminator-only
                    // tiers, so the public count's zero/nonzero views are exercised.
                    let count = talkbank_model::alignment::helpers::MorItemCount::new(
                        expected.items().len(),
                    );
                    assert_eq!(count.get(), expected.items().len());
                    assert_eq!(count.is_zero(), expected.is_empty());
                    assert_eq!(count.to_string(), expected.items().len().to_string());
                    // mor_contents admits spaces and newline-tab continuations,
                    // not a bare tab inside the tier body.
                    // Derive variants from actual reference bodies, retaining
                    // their words, clitics, features and terminator rather than
                    // fabricating an expected morphology model.
                    for trailing in [" ", "  ", "\n\t"] {
                        let variant = format!("{body}{trailing}");
                        let errors = ErrorCollector::new();
                        let ParseOutcome::Parsed(actual) = ChatParser::parse_mor_tier(
                            &parser,
                            &variant,
                            body_span.start as usize,
                            &errors,
                        ) else {
                            panic!(
                                "whitespace variant rejected: {} {variant:?}",
                                fixture.path().display()
                            );
                        };
                        assert!(errors.is_empty(), "{variant:?}: {:?}", errors.to_vec());
                        assert!(actual.semantic_eq(expected), "{variant:?}");
                        morphology_whitespace_controls[usize::from(expected.is_empty())] += 1;
                    }
                    for trailing in ["\t", " \t"] {
                        let variant = format!("{body}{trailing}");
                        let errors = ErrorCollector::new();
                        let outcome = ChatParser::parse_mor_tier(
                            &parser,
                            &variant,
                            body_span.start as usize,
                            &errors,
                        );
                        assert!(matches!(outcome, ParseOutcome::Rejected), "{variant:?}");
                        assert!(!errors.is_empty(), "refusal needs evidence: {variant:?}");
                    }
                }
                compare_content!(Gra, parse_gra_tier, parse_gra_tier_with_context);
                compare_content!(Pho, parse_pho_tier, parse_pho_tier_with_context);
                compare_content!(Sin, parse_sin_tier, parse_sin_tier_with_context);
                compare_content!(Act, parse_act_tier, parse_act_tier_with_context);
                compare_content!(Cod, parse_cod_tier, parse_cod_tier_with_context);
                compare_content!(Com, parse_com_tier, parse_com_tier_with_context);
                compare_content!(Exp, parse_exp_tier, parse_exp_tier_with_context);
                compare_content!(Add, parse_add_tier, parse_add_tier_with_context);
                compare_content!(Gpx, parse_gpx_tier, parse_gpx_tier_with_context);
                compare_content!(Int, parse_int_tier, parse_int_tier_with_context);
                compare_content!(Spa, parse_spa_tier, parse_spa_tier_with_context);
                compare_content!(Sit, parse_sit_tier, parse_sit_tier_with_context);
                compare_content!(Wor, parse_wor_tier, parse_wor_tier_with_context);
            }
        }
    }
    assert!(
        compared > 0,
        "reference corpus must exercise content-only tier APIs"
    );
    assert!(
        morphology_whitespace_controls
            .iter()
            .all(|count| *count > 0),
        "lexical and terminator-only morphology witnesses required"
    );
    assert_eq!(
        contextual_variants.len(),
        14,
        "each public tier context adapter needs a reference witness: {contextual_variants:?}"
    );
}

#[test]
fn reference_tier_fragments_refuse_trailing_tiers() {
    let source =
        std::fs::read_to_string(workspace_root().join("corpus/reference/tiers/mor-gra.cha"))
            .expect("canonical adjacent tiers");
    let parser = TreeSitterParser::new().expect("parser");
    let file = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(&source))
        .expect("reference parses");
    let mut checked = 0;
    for utterance in file.utterances() {
        let [first, second, ..] = utterance.dependent_tiers.as_slice() else {
            continue;
        };
        assert!(matches!(
            first.tier,
            talkbank_model::model::DependentTier::Mor(_)
        ));
        let full = source
            .get(first.span().start as usize..second.span().end as usize)
            .expect("adjacent tier spans");
        let body = source
            .get(first.content_span().expect("body").start as usize..second.span().end as usize)
            .expect("body followed by next tier");
        assert_fragment_rebases(
            std::path::Path::new("mor-gra.cha"),
            full,
            |offset, errors| {
                let outcome = ChatParser::parse_dependent_tier(&parser, full, offset, errors);
                assert!(
                    matches!(outcome, ParseOutcome::Rejected),
                    "generic tier API discarded another tier"
                );
                outcome
            },
        );
        assert_fragment_rebases(
            std::path::Path::new("mor-gra.cha"),
            body,
            |offset, errors| {
                let outcome = ChatParser::parse_mor_tier(&parser, body, offset, errors);
                assert!(
                    matches!(outcome, ParseOutcome::Rejected),
                    "MOR body API discarded another tier"
                );
                outcome
            },
        );
        checked += 1;
    }
    let utterances: Vec<_> = file.utterances().collect();
    for pair in utterances.windows(2) {
        let entry = pair[0].dependent_tiers.last().expect("reference has GRA");
        assert!(matches!(
            entry.tier,
            talkbank_model::model::DependentTier::Gra(_)
        ));
        let end = pair[1].main.span.end as usize;
        let full = source
            .get(entry.span().start as usize..end)
            .expect("tier followed by speech");
        let body = source
            .get(entry.content_span().expect("body").start as usize..end)
            .expect("body followed by speech");
        assert_fragment_rebases(
            std::path::Path::new("mor-gra.cha"),
            full,
            |offset, errors| {
                let outcome = ChatParser::parse_dependent_tier(&parser, full, offset, errors);
                assert!(
                    matches!(outcome, ParseOutcome::Rejected),
                    "tier API discarded speech"
                );
                outcome
            },
        );
        assert_fragment_rebases(
            std::path::Path::new("mor-gra.cha"),
            body,
            |offset, errors| {
                let outcome = ChatParser::parse_gra_tier(&parser, body, offset, errors);
                assert!(
                    matches!(outcome, ParseOutcome::Rejected),
                    "GRA body API discarded speech"
                );
                outcome
            },
        );
        checked += 1;
    }
    assert!(checked > 0);
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
                    ChatParser::parse_dependent_tier(&parser, input, span.start as usize, &errors)
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
    use talkbank_parser::generated_traversal::{
        FromNodeKind, GraRelationNode, MorWordNode, PhoWordsNode,
    };
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Kind {
        Mor,
        Gra,
        Pho,
    }

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("admitted reference corpus");
    let mut counts = [0; 3];
    let mut morphology_feature_shapes = [false; 2];
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference parses cleanly");
        let parsed_source = parser
            .parse_source_incremental(fixture.source(), None)
            .expect("reference CST");
        let mut pending = vec![parsed_source.root_node()];
        let mut ranges = Vec::new();
        while let Some(node) = pending.pop() {
            let kind = if MorWordNode::from_node(node).is_some() {
                Some(Kind::Mor)
            } else if GraRelationNode::from_node(node).is_some() {
                Some(Kind::Gra)
            } else if PhoWordsNode::from_node(node).is_some() {
                // The AST keeps a '+' compound as one word, not its components.
                Some(Kind::Pho)
            } else {
                None
            };
            if let Some(kind) = kind {
                ranges.push((kind, node.byte_range()));
            }
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
        }
        ranges.sort_by_key(|(_, range)| range.start);
        let context = FragmentSemanticContext::new();
        macro_rules! compare_item {
            ($expected:expr, $sources:ident, $method:ident, $contextual:ident, $counter:expr) => {{
                let expected = $expected;
                let (_, range) = $sources.next().expect("each AST item has a CST witness");
                let input = fixture
                    .source()
                    .get(range.clone())
                    .expect("item source span");
                for offset in [0, range.start, range.start + 17] {
                    let errors = ErrorCollector::new();
                    let ParseOutcome::Parsed(actual) =
                        ChatParser::$method(&parser, input, offset, &errors)
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
                    let contextual_errors = ErrorCollector::new();
                    let ParseOutcome::Parsed(contextual) = ChatParser::$contextual(
                        &parser,
                        input,
                        offset,
                        &context,
                        &contextual_errors,
                    ) else {
                        panic!(
                            "contextual item rejected: {} {input:?}",
                            fixture.path().display()
                        );
                    };
                    assert!(
                        contextual_errors.is_empty(),
                        "{:?}",
                        contextual_errors.to_vec()
                    );
                    assert_eq!(contextual, actual, "empty-context item contract");
                }
                counts[$counter] += 1;
            }};
        }
        for line in &file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            for entry in &utterance.dependent_tiers {
                let kind = match &entry.tier {
                    DependentTier::Mor(_) => Kind::Mor,
                    DependentTier::Gra(_) => Kind::Gra,
                    DependentTier::Pho(_) => Kind::Pho,
                    _ => continue,
                };
                let span = entry.span();
                let mut sources = ranges.iter().filter(|(candidate, range)| {
                    *candidate == kind
                        && range.start >= span.start as usize
                        && range.end <= span.end as usize
                });
                match &entry.tier {
                    DependentTier::Mor(tier) => {
                        for item in tier.items() {
                            for word in std::iter::once(&item.main).chain(item.post_clitics.iter())
                            {
                                // Consumers can keep POS and analysis separate without
                                // serializing and then reparsing the whole morphology word.
                                let rebuilt = word.features.iter().cloned().fold(
                                    talkbank_model::model::MorWord::new(
                                        word.pos.clone(),
                                        word.lemma.clone(),
                                    ),
                                    |draft, feature| draft.with_feature(feature),
                                );
                                assert_eq!(&rebuilt, word, "feature insertion preserves order");
                                assert_eq!(
                                    format!("{}|{}", word.pos, word.analysis()),
                                    word.to_chat_string(),
                                    "typed analysis view retains lemma and every feature",
                                );
                                morphology_feature_shapes[usize::from(!word.features.is_empty())] =
                                    true;
                                compare_item!(
                                    word,
                                    sources,
                                    parse_mor_word,
                                    parse_mor_word_with_context,
                                    0
                                );
                            }
                        }
                    }
                    DependentTier::Gra(tier) => {
                        for relation in tier.relations() {
                            compare_item!(
                                relation,
                                sources,
                                parse_gra_relation,
                                parse_gra_relation_with_context,
                                1
                            );
                        }
                    }
                    DependentTier::Pho(tier) => {
                        for item in tier.items.iter() {
                            match item {
                                PhoItem::Word(word) => {
                                    compare_item!(
                                        word,
                                        sources,
                                        parse_pho_word,
                                        parse_pho_word_with_context,
                                        2
                                    );
                                }
                                PhoItem::Group(words) => {
                                    for word in words.iter() {
                                        compare_item!(
                                            word,
                                            sources,
                                            parse_pho_word,
                                            parse_pho_word_with_context,
                                            2
                                        );
                                    }
                                }
                            }
                        }
                    }
                    // Other dependent-tier kinds have no item API in this test;
                    // their complete tiers are exercised above.
                    _ => {}
                }
                assert!(
                    sources.next().is_none(),
                    "{}: every CST item must be compared",
                    fixture.path().display()
                );
            }
        }
    }
    assert!(
        counts.iter().all(|count| *count > 0),
        "each item API needs corpus witnesses: {counts:?}"
    );
    assert_eq!(
        morphology_feature_shapes, [true; 2],
        "bare and featured source words required",
    );
}

/// A word projection must not silently discard another item or a post-clitic.
#[test]
fn reference_mor_word_fragments_refuse_lossy_projection() {
    use talkbank_parser::generated_traversal::{FromNodeKind, MorContentNode};
    let source =
        std::fs::read_to_string(workspace_root().join("corpus/reference/tiers/mor-gra.cha"))
            .expect("canonical morphology control");
    let parser = TreeSitterParser::new().expect("parser");
    let parsed = parser
        .parse_source_incremental(&source, None)
        .expect("reference tree");
    let mut pending = vec![parsed.root_node()];
    let mut ranges = Vec::new();
    while let Some(node) = pending.pop() {
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
        if MorContentNode::from_node(node).is_some() {
            ranges.push(node.byte_range());
        }
    }
    ranges.sort_by_key(|range| range.start);
    assert!(
        ranges.len() >= 3,
        "reference has a clitic item and following words"
    );
    for range in [ranges[0].clone(), ranges[1].start..ranges[2].end] {
        let input = source.get(range).expect("CST-owned source slice");
        assert_fragment_rebases(
            std::path::Path::new("mor-gra.cha"),
            input,
            |offset, errors| {
                let outcome = ChatParser::parse_mor_word(&parser, input, offset, errors);
                assert!(
                    matches!(outcome, ParseOutcome::Rejected),
                    "lossy word projection accepted {input:?}"
                );
                let diagnostics = errors.to_vec();
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(
                    diagnostics[0].code,
                    talkbank_model::ErrorCode::InvalidWordFormat
                );
                outcome
            },
        );
    }
}

#[test]
fn duplicate_end_spec_keeps_its_diagnostic_with_or_without_final_newline() {
    let parser = TreeSitterParser::new().expect("parser");
    let fixture = include_str!("../error_corpus/validation_errors/E501_5.cha");
    for source in [fixture, fixture.trim_end_matches('\n')] {
        for offset in [0, 17] {
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(file) =
                ChatParser::parse_chat_file(&parser, source, offset, &errors)
            else {
                panic!("duplicate End still retains the document");
            };
            assert_eq!(file.utterances().count(), 1);
            let diagnostics = errors.to_vec();
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
            assert_eq!(
                diagnostics[0].code,
                talkbank_model::ErrorCode::UnparsableContent
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
fn reference_single_item_fragments_refuse_extra_input() {
    use talkbank_parser::generated_traversal::{
        FromNodeKind, GraRelationNode, NodeSlot, ParticipantNode, PhoGroupChoice, PhoGroupNode,
        PhoWordNode, extract_pho_group,
    };
    enum Kind {
        Relation,
        Participant,
        Phone,
    }
    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, kind) in [
        ("mor-gra.cha", Kind::Relation),
        ("mor-gra.cha", Kind::Participant),
        ("pho-groupings.cha", Kind::Phone),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("corpus/reference/tiers")
                .join(fixture),
        )
        .expect("canonical item source");
        let parsed = parser
            .parse_source_incremental(&source, None)
            .expect("reference tree");
        let mut pending = vec![parsed.root_node()];
        let mut ranges = Vec::new();
        let mut group_ranges = Vec::new();
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let selected = match kind {
                Kind::Relation => GraRelationNode::from_node(node).is_some(),
                Kind::Participant => ParticipantNode::from_node(node).is_some(),
                Kind::Phone => PhoWordNode::from_node(node).is_some(),
            };
            if selected {
                ranges.push(node.byte_range());
            }
            if matches!(kind, Kind::Phone)
                && let Some(group) = PhoGroupNode::from_node(node)
                && matches!(
                    extract_pho_group(group)
                        .expect("producer reconstruction")
                        .content
                        .slot(),
                    NodeSlot::Present(PhoGroupChoice::PhoBeginGroup(_))
                )
            {
                group_ranges.push(node.byte_range());
            }
        }
        ranges.sort_by_key(|range| range.start);
        assert!(ranges.len() >= 2);
        if matches!(kind, Kind::Relation) {
            let errors = ErrorCollector::new();
            let input = source
                .get(ranges[0].clone())
                .expect("single relation source");
            let ParseOutcome::Parsed(tier) = ChatParser::parse_gra_tier(&parser, input, 0, &errors)
            else {
                panic!("one relation is already a complete GRA tier body");
            };
            assert_eq!(tier.relations().len(), 1);
            assert!(errors.is_empty());

            // API adapters must preserve an actual parser refusal and its separately
            // streamed diagnostics, not turn it into a successful partial relation.
            let accepted = ChatParser::parse_gra_relation(&parser, input, 0, &errors);
            let refused_errors = ErrorCollector::new();
            let refused = ChatParser::parse_gra_relation(
                &parser,
                source
                    .get(ranges[0].start..ranges[1].end)
                    .expect("two relations"),
                0,
                &refused_errors,
            );
            assert!(accepted.is_some());
            assert!(refused.is_rejected());
            assert!(!refused_errors.is_empty());
            for outcome in [&accepted, &refused] {
                let optional: Option<_> = outcome.clone().into();
                let restored = ParseOutcome::from(optional);
                assert!(restored.semantic_eq(outcome));
                assert_eq!(outcome.as_ref().is_some(), outcome.is_parsed());
                assert_eq!(outcome.as_ref().is_none(), outcome.is_rejected());
                let eager = outcome.clone().ok_or("no complete relation");
                let mut calls = 0;
                let lazy = outcome.clone().ok_or_else(|| {
                    calls += 1;
                    "no complete relation"
                });
                assert_eq!(eager, lazy);
                assert_eq!(calls, usize::from(outcome.is_rejected()));
            }
            assert!(!accepted.semantic_eq(&refused));
            assert!(!refused.semantic_eq(&accepted));
            for left in [&accepted, &refused] {
                for right in [&accepted, &refused] {
                    let paired = left.clone().zip(right.clone());
                    assert_eq!(paired.is_parsed(), left.is_parsed() && right.is_parsed());
                    if let ParseOutcome::Parsed((left, right)) = paired {
                        assert!(left.semantic_eq(&right));
                        assert!(left.semantic_eq(&tier.relations()[0]));
                    }
                }
            }
            assert!(
                !refused_errors.is_empty(),
                "adapters cannot consume the diagnostic sink"
            );
        }
        let cases = std::iter::once(ranges[0].start..ranges[1].end).chain(group_ranges);
        for range in cases {
            let input = source
                .get(range)
                .expect("CST-derived multi-item or empty slice");
            assert_fragment_rebases(std::path::Path::new(fixture), input, |offset, errors| {
                let admitted = match kind {
                    Kind::Relation => {
                        ChatParser::parse_gra_relation(&parser, input, offset, errors).is_parsed()
                    }
                    Kind::Participant => {
                        ChatParser::parse_participant_entry(&parser, input, offset, errors)
                            .is_parsed()
                    }
                    Kind::Phone => {
                        ChatParser::parse_pho_word(&parser, input, offset, errors).is_parsed()
                    }
                };
                assert!(!admitted, "single-item API silently accepted {input:?}");
                ParseOutcome::<talkbank_model::Span>::Rejected
            });
        }
        // Empty input has a real insertion point, not an unlocated DUMMY span.
        // The generic comparison helper cannot distinguish those at offset zero.
        for offset in [0, 17] {
            let errors = ErrorCollector::new();
            let admitted = match kind {
                Kind::Relation => {
                    ChatParser::parse_gra_relation(&parser, "", offset, &errors).is_parsed()
                }
                Kind::Participant => {
                    ChatParser::parse_participant_entry(&parser, "", offset, &errors).is_parsed()
                }
                Kind::Phone => ChatParser::parse_pho_word(&parser, "", offset, &errors).is_parsed(),
            };
            assert!(!admitted, "empty single-item input cannot produce a model");
            assert!(!errors.is_empty());
            for error in errors.into_vec() {
                assert_eq!(
                    error.location.span,
                    talkbank_model::Span::from_usize(offset, offset)
                );
            }
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
                    let parsed = ChatParser::parse_header_with_context(
                        &parser,
                        input,
                        span.start as usize,
                        &context,
                        &errors,
                    );
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
                        let ParseOutcome::Parsed(actual) = ChatParser::parse_id_header_with_context(
                            &parser,
                            input,
                            span.start as usize,
                            &context,
                            &errors,
                        ) else {
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
            let parsed = ChatParser::parse_main_tier_with_context(
                &parser,
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
            // File-level headers are exercised above. The utterance fragment
            // comes from the original tier spans, never serialized CHAT.
            assert!(
                utterance.preceding_headers.is_empty(),
                "attached headers need their own source range: {}",
                fixture.path().display()
            );
            let end = utterance
                .dependent_tiers
                .last()
                .map_or(main.span.end, |entry| entry.span().end);
            let utterance_source = fixture
                .source()
                .get(main.span.start as usize..end as usize)
                .expect("parsed utterance tiers belong to fixture");
            let errors = ErrorCollector::new();
            let parsed = ChatParser::parse_utterance_with_context(
                &parser,
                utterance_source,
                main.span.start as usize,
                &context,
                &errors,
            );
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
