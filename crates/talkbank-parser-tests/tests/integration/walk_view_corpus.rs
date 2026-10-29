//! Read-only/mutable traversal correspondence over parsed reference content.
//! Domain policy is owned by the existing authored alignment specifications;
//! this contract checks that mutable access retains the same leaves and order.

use talkbank_model::alignment::helpers::{
    ContentItem, ContentItemMut, TierDomain, WordItem, WordItemMut, walk_content, walk_content_mut,
    walk_words, walk_words_mut,
};
use talkbank_model::model::{UtteranceContent, WriteChat};

#[test]
fn nested_standalone_underline_specs_preserve_mutable_walks_and_findings() {
    use talkbank_model::model::{Line, TranscriptName};
    use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};

    #[derive(Debug, PartialEq)]
    enum Leaf {
        Word(String),
        UnderlineBegin,
        UnderlineEnd,
    }
    #[derive(Clone, Copy)]
    enum Markers {
        Balanced,
        MissingEnd,
        MissingBegin,
    }
    impl Markers {
        fn leaves(self) -> Vec<Leaf> {
            use Leaf::{UnderlineBegin as B, UnderlineEnd as E, Word as W};
            match self {
                Self::Balanced => vec![
                    W("hello".into()),
                    B,
                    W("world".into()),
                    E,
                    W("again".into()),
                ],
                Self::MissingEnd => {
                    vec![W("hello".into()), B, W("world".into()), W("again".into())]
                }
                Self::MissingBegin => {
                    vec![W("hello".into()), W("world".into()), E, W("again".into())]
                }
            }
        }
        fn findings(self) -> Vec<ErrorCode> {
            match self {
                Self::Balanced => vec![],
                Self::MissingEnd => vec![ErrorCode::UnmatchedUnderlineBegin],
                Self::MissingBegin => vec![ErrorCode::UnmatchedUnderlineEnd],
            }
        }
    }
    fn check(parser: &impl ChatParser, source: &str, path: &std::path::Path, markers: Markers) {
        let errors = ErrorCollector::new();
        let mut file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .unwrap();
        assert!(errors.is_empty(), "{:?}", errors.into_vec());
        let mut turns = 0;
        for line in &mut file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            turns += 1;
            let content = utterance.main.content.content.as_mut_slice();
            assert_walk_views(content, source, path);
            for domain in [
                None,
                Some(TierDomain::Mor),
                Some(TierDomain::Pho),
                Some(TierDomain::Sin),
                Some(TierDomain::Wor),
            ] {
                let mut observed = Vec::new();
                walk_content(content, domain, &mut |item| {
                    observed.push(match item {
                        ContentItem::Word(word) => Leaf::Word(word.raw_text()),
                        ContentItem::UnderlineBegin(_) => Leaf::UnderlineBegin,
                        ContentItem::UnderlineEnd(_) => Leaf::UnderlineEnd,
                        _ => panic!("only words and standalone underline markers are authored"),
                    })
                });
                assert_eq!(observed, markers.leaves(), "{domain:?}");
            }
        }
        assert_eq!(turns, 1);
        let errors = ErrorCollector::new();
        file.validate(&errors, TranscriptName::Anonymous);
        assert_eq!(
            errors
                .into_vec()
                .into_iter()
                .map(|error| error.code)
                .collect::<Vec<_>>(),
            markers.findings()
        );
        assert_eq!(
            file.to_chat_string(),
            format!("{source}\n"),
            "loaded specs omit the final newline; all authored markers remain observable"
        );
    }
    let root = talkbank_parser_tests::repo_paths::workspace_root();
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    for (filename, title, markers) in [
        (
            "E356.md",
            "Balanced standalone markers inside an annotated group",
            Markers::Balanced,
        ),
        (
            "E356.md",
            "Delete the standalone closing marker inside the group",
            Markers::MissingEnd,
        ),
        (
            "E357.md",
            "Delete the standalone opening marker inside the group",
            Markers::MissingBegin,
        ),
    ] {
        let spec = specs.iter().find(|spec| spec.filename == filename).unwrap();
        let example = spec
            .examples()
            .iter()
            .find(|example| example.title.as_deref() == Some(title))
            .unwrap();
        let path = root.join("spec/errors").join(filename);
        check(&tree, example.chat.as_str(), &path, markers);
        check(&native, example.chat.as_str(), &path, markers);
    }
}

#[test]
fn nested_word_annotations_preserve_domain_exclusion_and_language_scope() {
    use talkbank_model::alignment::helpers::walk_words_scoped;
    use talkbank_model::model::{CodeSwitchSpan, Line, TranscriptName};
    use talkbank_model::{ChatParser, ErrorCollector};

    #[derive(Clone, Copy, Debug)]
    enum Control {
        Exclusion,
        LanguageScope,
        GroupExclusion,
    }

    fn check(parser: &impl ChatParser, source: &str, path: &std::path::Path) {
        let errors = ErrorCollector::new();
        let mut file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .unwrap();
        assert!(errors.is_empty(), "{:?}", errors.into_vec());
        let errors = ErrorCollector::new();
        file.validate(&errors, TranscriptName::Anonymous);
        assert!(errors.is_empty(), "{:?}", errors.into_vec());
        let mut utterances: Vec<_> = (&mut file.lines)
            .into_iter()
            .filter_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance),
                Line::Header { .. } => None,
            })
            .collect();
        let [excluded, scoped, group_excluded] = utterances.as_mut_slice() else {
            panic!("three authored annotation controls");
        };
        for (control, utterance) in [
            (Control::Exclusion, excluded),
            (Control::LanguageScope, scoped),
            (Control::GroupExclusion, group_excluded),
        ] {
            assert_walk_views(utterance.main.content.content.as_mut_slice(), source, path);
            for domain in [
                None,
                Some(TierDomain::Mor),
                Some(TierDomain::Pho),
                Some(TierDomain::Sin),
                Some(TierDomain::Wor),
            ] {
                let expected = match (control, domain) {
                    (Control::Exclusion, Some(TierDomain::Mor)) => {
                        vec![("hello", None), ("after", None)]
                    }
                    (Control::GroupExclusion, Some(TierDomain::Mor)) => vec![("after", None)],
                    (
                        Control::Exclusion | Control::GroupExclusion,
                        None | Some(TierDomain::Pho | TierDomain::Sin | TierDomain::Wor),
                    ) => vec![("hello", None), ("there", None), ("after", None)],
                    (Control::LanguageScope, _) => vec![
                        ("hello", Some("eng")),
                        ("hola", Some("spa")),
                        ("after", None),
                    ],
                };
                let mut observed = Vec::new();
                walk_words_scoped(
                    &utterance.main.content.content,
                    domain,
                    &mut |item, scope| {
                        let WordItem::Word(word) = item else {
                            panic!("controls contain only spoken words");
                        };
                        let language = match scope.span() {
                            None => None,
                            Some(CodeSwitchSpan::Explicit(code)) => Some(code.as_str()),
                            Some(CodeSwitchSpan::Shortcut) => {
                                panic!("only explicit scopes are authored")
                            }
                        };
                        observed.push((word.raw_text(), language));
                    },
                );
                assert_eq!(
                    observed
                        .iter()
                        .map(|(word, scope)| (word.as_str(), *scope))
                        .collect::<Vec<_>>(),
                    expected,
                    "control {control:?}, domain {domain:?}"
                );
                let mut content_words = Vec::new();
                walk_content(&utterance.main.content.content, domain, &mut |item| {
                    let ContentItem::Word(word) = item else {
                        panic!("controls contain only spoken words");
                    };
                    content_words.push(word.raw_text());
                });
                assert_eq!(
                    content_words,
                    expected.iter().map(|(word, _)| *word).collect::<Vec<_>>()
                );
            }
        }
        assert_eq!(
            file.to_chat_string(),
            source,
            "walking never changes annotations"
        );
    }
    let path = talkbank_parser_tests::repo_paths::workspace_root()
        .join("corpus/reference/annotation/nested-word-annotations.cha");
    let source = std::fs::read_to_string(&path).expect("authored nested annotations");
    check(
        &talkbank_parser::TreeSitterParser::new().unwrap(),
        &source,
        &path,
    );
    check(&talkbank_parser_re2c::Re2cParser::new(), &source, &path);
}

#[test]
fn reference_groups_keep_bare_and_annotated_actions_distinct() {
    use talkbank_model::model::{BracketedItem, ContentStructure, Descend};
    use talkbank_parser::TreeSitterParser;
    use talkbank_parser_tests::{repo_paths::workspace_root, test_error::strict_parse};
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/edge-cases/bracketed-content-combinations.cha"),
    )
    .expect("authored group combinations");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let mut bare = 0;
    let mut annotated = 0;
    for utterance in file.utterances() {
        for item in &utterance.main.content.content {
            item.structure().walk(&mut |structure| {
                if let ContentStructure::Group(group) = structure {
                    for child in &group.content().content {
                        match child {
                            BracketedItem::Action(action) => {
                                assert_eq!(action.to_chat_string(), "0");
                                bare += 1;
                            }
                            BracketedItem::AnnotatedAction(action) => {
                                assert_eq!(action.to_chat_string(), "0 [=! points]");
                                annotated += 1;
                            }
                            _ => {}
                        }
                    }
                }
                Descend::Into
            });
        }
    }
    assert_eq!((bare, annotated), (1, 1));
}

#[test]
fn reference_overlap_queries_distinguish_open_close_and_lexical_items() {
    use talkbank_parser::TreeSitterParser;
    use talkbank_parser_tests::{repo_paths::workspace_root, test_error::strict_parse};
    #[derive(Clone, Copy)]
    enum Position {
        Open,
        Close,
        Lexical,
        WordClosing(&'static str),
    }
    use Position::{Close as C, Lexical as W, Open as O};
    // Authored item order, independent of the marker classification methods.
    // The first turn includes both plain and indexed top brackets; the next
    // two supply their bottom counterparts. Attached closing marks in the last
    // two turns belong to WordContent, not standalone UtteranceContent.
    let expected: &[&[Position]] = &[
        &[O, W, C, O, W, C],
        &[O, W, C],
        &[O, W, C],
        &[O, W, Position::WordClosing("⌉"), W, W],
        &[W, W, O, W, Position::WordClosing("⌋")],
    ];
    let source = std::fs::read_to_string(workspace_root().join("corpus/reference/ca/overlaps.cha"))
        .expect("authored CA overlaps");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    assert_eq!(file.utterances().count(), expected.len());
    for (utterance, expected) in file.utterances().zip(expected) {
        let content = &utterance.main.content.content;
        assert_eq!(content.len(), expected.len());
        for (item, position) in content.iter().zip(*expected) {
            assert_eq!(item.is_opening_overlap(), matches!(position, O));
            assert_eq!(item.is_closing_overlap(), matches!(position, C));
            match position {
                O | C => assert!(matches!(item, UtteranceContent::OverlapPoint(_))),
                W => assert!(matches!(item, UtteranceContent::Word(_))),
                Position::WordClosing(expected) => {
                    let UtteranceContent::Word(word) = item else {
                        panic!("attached marker belongs to a word");
                    };
                    let Some(talkbank_model::model::WordContent::OverlapPoint(point)) =
                        word.content().last()
                    else {
                        panic!("word retains its closing marker");
                    };
                    assert_eq!(point.to_chat_string(), *expected);
                }
            }
        }
    }
}

pub(super) fn assert_walk_views(
    content: &mut [UtteranceContent],
    source: &str,
    path: &std::path::Path,
) -> usize {
    let original = content.to_vec();
    let mut separators = 0;
    walk_content(&original, None, &mut |item| {
        if let ContentItem::Separator(separator) = item {
            let spelling = separator.to_chat_string();
            assert_eq!(
                source.get(separator.location().span().expect("located").to_range()),
                Some(spelling.as_str())
            );
            assert_eq!(separator.to_string(), spelling);
            assert!(
                super::assert_output_refusals(|writer| write!(writer, "{separator}"), path,) > 0
            );
            separators += 1;
        }
    });
    for domain in [
        None,
        Some(TierDomain::Mor),
        Some(TierDomain::Pho),
        Some(TierDomain::Sin),
        Some(TierDomain::Wor),
    ] {
        let mut expected = Vec::new();
        walk_content(&original, domain, &mut |item| expected.push(item));
        let mut expected = expected.into_iter();
        walk_content_mut(content, domain, &mut |actual| {
            let expected = expected.next().expect("mutable walker must not add a leaf");
            compare_content(expected, actual);
        });
        assert!(
            expected.next().is_none(),
            "mutable walker must not omit a leaf"
        );

        let mut expected = Vec::new();
        walk_words(&original, domain, &mut |item| expected.push(item));
        let mut expected = expected.into_iter();
        walk_words_mut(content, domain, &mut |actual| {
            let expected = expected
                .next()
                .expect("mutable word walker must not add a leaf");
            match actual {
                WordItemMut::Word(right) => {
                    let WordItem::Word(left) = expected else {
                        panic!("word kind/order changed")
                    };
                    assert_eq!(left, &*right);
                }
                WordItemMut::ReplacedWord(right) => {
                    let WordItem::ReplacedWord(left) = expected else {
                        panic!("replacement kind/order changed")
                    };
                    assert_eq!(left, &*right);
                }
                WordItemMut::Separator(right) => {
                    let WordItem::Separator(left) = expected else {
                        panic!("separator kind/order changed")
                    };
                    assert_eq!(left, &*right);
                }
            }
        });
        assert!(
            expected.next().is_none(),
            "mutable word walker must not omit a leaf"
        );
        assert_eq!(
            content,
            original.as_slice(),
            "borrowing views must preserve the complete model"
        );
    }
    separators
}

fn compare_content(expected: ContentItem<'_>, actual: ContentItemMut<'_>) {
    // Expand to an exhaustive match over the mutable view. No debug strings,
    // serialization or handwritten CHAT classification stand in for typed data.
    macro_rules! compare_variants {
        ($($variant:ident),+ $(,)?) => {
            match actual {
                $(ContentItemMut::$variant(right) => {
                    let ContentItem::$variant(left) = expected else {
                        panic!("content kind/order changed: expected {}", stringify!($variant));
                    };
                    assert_eq!(left, &*right);
                }),+
            }
        };
    }
    compare_variants!(
        Word,
        ReplacedWord,
        Separator,
        Event,
        Pause,
        Action,
        OverlapPoint,
        OtherSpokenEvent,
        Freecode,
        OrphanAnnotation,
        InternalBullet,
        LongFeatureBegin,
        LongFeatureEnd,
        UnderlineBegin,
        UnderlineEnd,
        NonvocalBegin,
        NonvocalEnd,
        NonvocalSimple
    );
}
