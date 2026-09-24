//! Semantic comparison and bounded diagnostic reports over canonical documents.
#![allow(clippy::expect_used)]

use talkbank_model::model::ChatFile;
use talkbank_model::{
    SemanticDiff, SemanticDiffContext, SemanticDiffReport, SemanticEq, SemanticPath,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};

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
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut previous: Option<ChatFile> = None;
    let mut unequal = 0;
    let mut located = 0;
    let mut kinds = std::collections::HashSet::new();
    for fixture in corpus.fixtures() {
        let current =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
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
