//! Semantic comparison and bounded diagnostic reports over canonical documents.
#![allow(clippy::expect_used)]

use talkbank_model::model::{ChatFile, WriteChat};
use talkbank_model::{
    SemanticDiff, SemanticDiffContext, SemanticDiffReport, SemanticEq, SemanticPath,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};

/// Check actual parsed payloads without assigning validity to either model.
fn compare_content_pair(
    left: &talkbank_model::model::UtteranceContent,
    right: &talkbank_model::model::UtteranceContent,
) -> bool {
    let full = report(left, right, usize::MAX);
    assert_eq!(full.is_empty(), left.semantic_eq(right));
    assert!(!full.is_truncated());
    assert_eq!(full.is_empty(), right.semantic_eq(left));
    for limit in [0, 1, 3] {
        let bounded = report(left, right, limit);
        assert_eq!(bounded.is_truncated(), full.differences().len() > limit);
        assert_eq!(
            bounded.differences().len(),
            full.differences().len().min(limit)
        );
        for (actual, expected) in bounded.differences().iter().zip(full.differences()) {
            assert_eq!(
                (
                    &actual.path,
                    actual.kind,
                    &actual.left,
                    &actual.right,
                    actual.span
                ),
                (
                    &expected.path,
                    expected.kind,
                    &expected.left,
                    &expected.right,
                    expected.span
                )
            );
        }
    }
    !full.is_empty()
}

fn report<T: SemanticDiff>(left: &T, right: &T, limit: usize) -> SemanticDiffReport {
    let mut report = SemanticDiffReport::new(limit);
    let mut path = SemanticPath::new();
    let mut context = SemanticDiffContext::new();
    left.semantic_diff_into(right, &mut path, &mut report, &mut context);
    assert_eq!(
        path.to_string(),
        "/",
        "traversal restores the caller's path"
    );
    assert_eq!(
        context.current_span(),
        None,
        "traversal restores the caller's source context"
    );
    report
}

#[test]
fn reference_word_projections_preserve_tuple_order_and_report_limits() {
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    use talkbank_model::model::Word;
    use talkbank_parser_tests::repo_paths::workspace_root;

    fn exercise<T: SemanticDiff + talkbank_model::SemanticEq>(left: &T, right: &T, fields: usize) {
        assert!(
            left.semantic_eq(left),
            "unchanged projections compare equal"
        );
        let equal = report(left, left, 0);
        assert!(equal.is_empty() && !equal.is_truncated());
        let full = report(left, right, usize::MAX);
        assert_eq!(
            left.semantic_eq(right),
            full.is_empty(),
            "tuple comparison agrees with its report"
        );
        assert!(!full.is_truncated());
        let mut previous = 0;
        for difference in full.differences() {
            let field = (0..fields)
                .find(|index| difference.path.starts_with(&format!("[{index}]")))
                .expect("tuple field owns each difference");
            assert!(field >= previous, "field order must be stable");
            previous = field;
        }
        for field in 0..fields {
            assert!(
                full.differences()
                    .iter()
                    .any(|difference| difference.path.starts_with(&format!("[{field}]")))
            );
        }
        for limit in 0..=full.differences().len() + 1 {
            let mut bounded = report(left, right, limit);
            assert_eq!(bounded.is_truncated(), limit < full.differences().len());
            assert_eq!(
                bounded.differences().len(),
                limit.min(full.differences().len())
            );
            for (actual, expected) in bounded.differences().iter().zip(full.differences()) {
                assert_eq!(
                    (
                        &actual.path,
                        actual.kind,
                        &actual.left,
                        &actual.right,
                        actual.span
                    ),
                    (
                        &expected.path,
                        expected.kind,
                        &expected.left,
                        &expected.right,
                        expected.span
                    )
                );
            }
            if bounded.is_truncated() {
                let before = bounded.differences().len();
                let mut path = SemanticPath::new();
                let mut context = SemanticDiffContext::new();
                // Appending a second projection cannot overrun a shared report budget.
                left.semantic_diff_into(right, &mut path, &mut bounded, &mut context);
                assert_eq!(bounded.differences().len(), before);
                assert_eq!(path.to_string(), "/");
                assert_eq!(context.current_span(), None);
            }
        }
    }

    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/basic-conversation.cha"),
    )
    .expect("canonical conversation");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let turns: Vec<Vec<Word>> = file
        .utterances()
        .take(2)
        .map(|utterance| {
            let mut words = Vec::new();
            walk_words(
                &utterance.main.content.content,
                None,
                &mut |item| match item {
                    WordItem::Word(word) => words.push(word.clone()),
                    WordItem::ReplacedWord(word) => words.push(word.word.clone()),
                    WordItem::Separator(_) => {}
                },
            );
            words
        })
        .collect();
    assert_eq!(turns.len(), 2);
    assert!(turns.iter().all(|turn| turn.len() >= 3));
    let left = &turns[0];
    let right = &turns[1];
    let mut textual_mismatches = 0;
    for (left, right) in left.iter().zip(right) {
        use std::borrow::Cow;
        use talkbank_model::SemanticEq;
        let left = left.to_chat_string();
        let right = right.to_chat_string();
        let borrowed = Cow::Borrowed(left.as_str());
        let owned = Cow::Owned(left.clone());
        assert!(
            borrowed.semantic_eq(&owned),
            "ownership does not alter a word view"
        );
        assert_eq!(
            borrowed.semantic_eq(&Cow::Borrowed(right.as_str())),
            left == right
        );
        assert_eq!(
            SemanticEq::semantic_eq(&left.as_str(), &right.as_str()),
            left == right
        );
        // Consumer projections may own or borrow the same parsed spelling.
        // Both equality and diagnostic reports must ignore that storage choice.
        for equal in [
            report(&left, &left, 0),
            report(&left.as_str(), &left.as_str(), 0),
            report(&borrowed, &owned, 0),
            report(&owned, &borrowed, 0),
        ] {
            assert!(equal.is_empty() && !equal.is_truncated());
        }
        for limit in [0, 1, 2] {
            for actual in [
                report(&left, &right, limit),
                report(&left.as_str(), &right.as_str(), limit),
                report(&borrowed, &Cow::Borrowed(right.as_str()), limit),
                report(&owned, &Cow::Owned(right.clone()), limit),
            ] {
                let differs = left != right;
                assert_eq!(actual.is_truncated(), differs && limit == 0);
                assert_eq!(
                    actual.differences().len(),
                    usize::from(differs && limit > 0)
                );
                if let Some(difference) = actual.differences().first() {
                    assert_eq!(difference.path, "/");
                    assert_eq!(
                        difference.kind,
                        talkbank_model::SemanticDiffKind::ValueMismatch
                    );
                    assert_eq!(difference.left, format!("{left:?}"));
                    assert_eq!(difference.right, format!("{right:?}"));
                    assert_eq!(
                        difference.span, None,
                        "text views do not invent source provenance"
                    );
                }
            }
        }
        textual_mismatches += usize::from(left != right);
    }
    assert!(
        textual_mismatches > 0,
        "reference turns must supply differing word views"
    );
    exercise(
        &(left[0].clone(), left[1].clone()),
        &(right[0].clone(), right[1].clone()),
        2,
    );
    exercise(
        &(left[0].clone(), left[1].clone(), left[2].clone()),
        &(right[0].clone(), right[1].clone(), right[2].clone()),
        3,
    );
}

#[test]
fn reference_speaker_removal_reports_sequence_tail_and_bounded_prefix() {
    use talkbank_model::SemanticDiffKind;
    use talkbank_parser_tests::repo_paths::workspace_root;
    use talkbank_transform::speaker_id::{apply_mapping_chat, parse_mapping_spec};

    let parser = TreeSitterParser::new().expect("parser");
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/basic-conversation.cha"),
    )
    .expect("reference transcript");
    let original = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    for mapping in ["CHI=drop", "MOT=drop"] {
        let mapping = parse_mapping_spec(mapping).expect("authored removal control");
        let changed_text = apply_mapping_chat(&original, &mapping);
        let changed =
            strict_parse(parser.parse_chat_file(&changed_text)).expect("transformed CHAT parses");
        assert!(changed.lines.len() < original.lines.len());
        for (left, right, kind) in [
            (&original.lines, &changed.lines, SemanticDiffKind::ExtraKey),
            (
                &changed.lines,
                &original.lines,
                SemanticDiffKind::MissingKey,
            ),
        ] {
            let full = report(left, right, usize::MAX);
            assert!(!full.is_empty() && !full.is_truncated());
            let tail = full.differences().last().expect("sequence tail difference");
            // ChatFileLines is a tuple newtype: [0] names its sequence field.
            assert_eq!(tail.path, format!("[0][{}]", changed.lines.len()));
            assert_eq!(tail.kind, kind);
            for limit in [0, 1, full.differences().len(), full.differences().len() + 1] {
                let bounded = report(left, right, limit);
                assert_eq!(bounded.is_truncated(), full.differences().len() > limit);
                assert_eq!(
                    bounded.differences().len(),
                    full.differences().len().min(limit)
                );
                for (actual, expected) in bounded.differences().iter().zip(full.differences()) {
                    assert_eq!(
                        (
                            &actual.path,
                            actual.kind,
                            &actual.left,
                            &actual.right,
                            actual.span
                        ),
                        (
                            &expected.path,
                            expected.kind,
                            &expected.left,
                            &expected.right,
                            expected.span
                        )
                    );
                }
            }
        }
    }
}

#[test]
fn reference_participant_reports_preserve_order_keys_and_truncation() {
    use talkbank_model::SemanticDiffKind;
    use talkbank_parser_tests::repo_paths::workspace_root;
    let parser = TreeSitterParser::new().expect("parser");
    let parse = |path: &str| {
        let source = std::fs::read_to_string(workspace_root().join("corpus/reference").join(path))
            .expect("canonical reference transcript");
        strict_parse(parser.parse_chat_file(&source)).expect("reference parses")
    };
    let basic = parse("core/basic-conversation.cha");
    let overlap = parse("ca/overlaps.cha");
    let extended = parse("core/headers-speaker-info.cha");
    let equal = report(&basic.participants, &basic.participants, 0);
    assert!(equal.is_empty() && !equal.is_truncated());

    // An authored speaker change must identify the first declared key, not
    // reorder maps or skip the key when the report budget is exhausted.
    let first = report(&basic.participants, &overlap.participants, 1);
    assert!(first.is_truncated());
    assert_eq!(first.differences().len(), 1);
    let difference = &first.differences()[0];
    assert_eq!(difference.path, "[0].key[0]");
    assert_eq!(difference.kind, SemanticDiffKind::ValueMismatch);
    assert_eq!(difference.left, "\"CHI\"");
    assert_eq!(difference.right, "\"SPK\"");
    assert_eq!(
        difference.span, None,
        "speaker keys do not carry source spans"
    );

    let length = report(&basic.participants, &extended.participants, 1);
    assert!(length.is_truncated());
    assert_eq!(length.differences().len(), 1);
    assert_eq!(length.differences()[0].path, "/");
    assert_eq!(
        length.differences()[0].kind,
        SemanticDiffKind::LengthMismatch
    );
    assert_eq!(length.differences()[0].left, "len=2");
    assert_eq!(length.differences()[0].right, "len=3");

    for (left, right) in [
        (&basic.participants, &overlap.participants),
        (&overlap.participants, &basic.participants),
        (&basic.participants, &extended.participants),
        (&extended.participants, &basic.participants),
    ] {
        let full = report(left, right, usize::MAX);
        assert!(!full.is_empty() && !full.is_truncated());
        assert!(
            full.differences()
                .iter()
                .any(|difference| difference.path.starts_with("[0].value"))
        );
        for limit in [
            0,
            1,
            2,
            full.differences().len(),
            full.differences().len() + 1,
        ] {
            let bounded = report(left, right, limit);
            assert_eq!(bounded.is_truncated(), full.differences().len() > limit);
            assert_eq!(
                bounded.differences().len(),
                full.differences().len().min(limit)
            );
            for (actual, expected) in bounded.differences().iter().zip(full.differences()) {
                assert_eq!(
                    (
                        &actual.path,
                        actual.kind,
                        &actual.left,
                        &actual.right,
                        actual.span
                    ),
                    (
                        &expected.path,
                        expected.kind,
                        &expected.left,
                        &expected.right,
                        expected.span
                    )
                );
            }
        }
    }
}

#[test]
fn reference_word_reports_preserve_semantics_and_budget_boundaries() {
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    use talkbank_model::model::Word;
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut previous: Option<Word> = None;
    let mut unequal = 0;
    let mut compared = 0;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => &replaced.word,
                    WordItem::Separator(_) => return,
                };
                // Zero capacity is not itself evidence of a difference.
                let same = report(word, word, 0);
                assert!(same.is_empty() && !same.is_truncated());
                if let Some(left) = previous.as_ref() {
                    let full = report(left, word, usize::MAX);
                    assert!(!full.is_truncated());
                    assert_eq!(full.is_empty(), left.semantic_eq(word));
                    let reverse = report(word, left, usize::MAX);
                    assert_eq!(reverse.is_empty(), full.is_empty());
                    for limit in [
                        0,
                        1,
                        full.differences().len(),
                        full.differences().len() + 1,
                        usize::MAX,
                    ] {
                        let bounded = report(left, word, limit);
                        assert_eq!(
                            bounded.differences().len(),
                            full.differences().len().min(limit)
                        );
                        assert_eq!(bounded.is_truncated(), full.differences().len() > limit);
                        for (actual, expected) in
                            bounded.differences().iter().zip(full.differences())
                        {
                            assert_eq!(
                                (
                                    &actual.path,
                                    actual.kind,
                                    &actual.left,
                                    &actual.right,
                                    actual.span
                                ),
                                (
                                    &expected.path,
                                    expected.kind,
                                    &expected.left,
                                    &expected.right,
                                    expected.span
                                )
                            );
                        }
                        assert!(
                            bounded
                                .render()
                                .contains(&format!("Differences (first {limit}):")),
                            "render the requested budget even after exhaustion"
                        );
                    }
                    unequal += usize::from(!full.is_empty());
                    compared += 1;
                }
                previous = Some(word.clone());
            });
        }
    }
    assert!(
        compared > 0 && unequal > 0,
        "parsed reference words must exercise both reporting and limits"
    );
}

#[test]
fn reference_semantic_reports_agree_with_equality_and_preserve_bounded_prefixes() {
    use talkbank_model::model::UtteranceContent;

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut previous: Option<ChatFile> = None;
    let mut unequal = 0;
    let mut located = 0;
    let mut kinds = std::collections::HashSet::new();
    // Compare within enum variants so an earlier document/header mismatch
    // cannot short-circuit every generated payload comparison. These are
    // original parsed items, not mutations manufactured to reach a branch.
    let mut previous_content = std::collections::HashMap::new();
    let mut content_mismatches = [0usize; 4];
    for fixture in corpus.fixtures() {
        let current =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for item in current
            .utterances()
            .flat_map(|utterance| &utterance.main.content.content)
        {
            let discriminator = std::mem::discriminant(item);
            if let Some(left) = previous_content.insert(discriminator, item.clone())
                && compare_content_pair(&left, item)
            {
                let witness = match item {
                    UtteranceContent::Word(_) => Some(0),
                    UtteranceContent::Pause(_) => Some(1),
                    UtteranceContent::ReplacedWord(_) => Some(2),
                    UtteranceContent::AnnotatedGroup(_) => Some(3),
                    _ => None,
                };
                if let Some(index) = witness {
                    content_mismatches[index] += 1;
                }
            }
        }
        // Wire decoding loses source provenance but not CHAT semantics. The
        // diagnostic traversal must agree, not report missing spans as edits.
        let decoded: ChatFile =
            serde_json::from_str(&serde_json::to_string(&current).expect("encode reference model"))
                .expect("decode reference model");
        assert!(current.semantic_eq(&decoded));
        let equal = current.semantic_diff(&decoded);
        assert!(
            equal.is_empty() && !equal.is_truncated(),
            "wire provenance is not semantic content: {}",
            fixture.path().display()
        );
        assert_eq!(equal.render(), equal.to_string());
        if let Some(left) = previous.as_ref() {
            // Adjacent actual documents supply diverse differences; their
            // authored content is not modified to force any report variant.
            let full = report(left, &current, usize::MAX);
            assert_eq!(full.is_empty(), left.semantic_eq(&current));
            assert!(!full.is_truncated());
            assert_eq!(full.render(), full.to_string());
            let reverse = report(&current, left, usize::MAX);
            assert_eq!(
                reverse.is_empty(),
                full.is_empty(),
                "semantic inequality is symmetric"
            );
            for difference in full.differences() {
                assert!(!difference.path.is_empty());
                kinds.insert(difference.kind.as_str());
                located += usize::from(difference.span.is_some());
            }
            for limit in [0, 1, 3] {
                let bounded = report(left, &current, limit);
                assert_eq!(
                    bounded.differences().len(),
                    full.differences().len().min(limit)
                );
                assert_eq!(bounded.is_truncated(), full.differences().len() > limit);
                for (actual, expected) in bounded.differences().iter().zip(full.differences()) {
                    assert_eq!(
                        (
                            &actual.path,
                            actual.kind,
                            &actual.left,
                            &actual.right,
                            actual.span
                        ),
                        (
                            &expected.path,
                            expected.kind,
                            &expected.left,
                            &expected.right,
                            expected.span
                        )
                    );
                }
                assert_eq!(bounded.render(), bounded.to_string());
            }
            unequal += usize::from(!full.is_empty());
        }
        previous = Some(current);
    }
    assert!(
        unequal > 0 && located > 0,
        "nonempty, source-located corpus differences required"
    );
    assert!(
        content_mismatches.iter().all(|count| *count > 0),
        "word, pause, replacement and annotated-group payload differences need real witnesses: {content_mismatches:?}"
    );
    for kind in [
        "value_mismatch",
        "length_mismatch",
        "variant_mismatch",
        "missing_key",
        "extra_key",
    ] {
        assert!(kinds.contains(kind), "corpus must witness {kind}");
    }
}

#[test]
fn spec_recovery_semantic_reports_preserve_payload_comparison_without_admission() {
    use talkbank_parser_tests::repo_paths::workspace_root;
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical specification corpus");
    let parser = TreeSitterParser::new().expect("parser");
    let mut previous_content = std::collections::HashMap::new();
    let mut recovered = 0;
    let mut compared = 0;
    let mut unequal = 0;
    for fixture in corpus.fixtures() {
        let diagnostics = talkbank_model::ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(fixture.source(), &diagnostics);
        if diagnostics.is_empty() {
            continue;
        }
        recovered += 1;
        // Recovery can be inspected by editors and reporting tools; comparison
        // is not validation or a promise that recovered output reparses.
        let same = report(&file, &file, 0);
        assert!(same.is_empty() && !same.is_truncated());
        assert!(file.semantic_eq(&file));
        for item in file
            .utterances()
            .flat_map(|utterance| &utterance.main.content.content)
        {
            if let Some(left) = previous_content.insert(std::mem::discriminant(item), item.clone())
            {
                unequal += usize::from(compare_content_pair(&left, item));
                compared += 1;
            }
        }
    }
    assert!(
        recovered > 0 && compared > 0 && unequal > 0,
        "recovered spec models and unequal payload pairs must execute: {recovered}/{compared}/{unequal}"
    );
}
