//! Separator provenance, CA exemption and canonical serialization at file boundaries.

use talkbank_model::model::{ChatFile, Line, TranscriptName, WriteChat};
use talkbank_model::{
    ChatParser, ErrorCode, ErrorCollector, ParseOutcome, SemanticEq, SourceDiagnostic, SourceIndex,
    Span,
};

fn separator_spans(file: &ChatFile) -> Vec<Span> {
    let mut spans = Vec::new();
    for line in &file.lines {
        match line {
            Line::Header { separator, .. } => spans.extend(separator.trailing_space()),
            Line::Utterance(utterance) => {
                spans.extend(utterance.main.separator.trailing_space());
                for tier in &utterance.dependent_tiers {
                    spans.extend(tier.separator.trailing_space());
                }
            }
        }
    }
    spans
}

/// Recovery does not promise equivalent serialized models across backends.
/// Keep that state distinct from clean admission, where equivalence is required.
enum Serialization {
    Clean(String),
    Recovered,
}

fn verify_separator_source(
    parser: &impl ChatParser,
    source: &str,
    offset: usize,
    expected: &[Span],
) -> Serialization {
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(mut actual) = parser.parse_chat_file(source, offset, &errors) else {
        panic!("separator examples must retain a document through recovery");
    };
    let parse_errors = errors.into_vec();
    let expected_shifted: Vec<_> = expected
        .iter()
        .map(|span| Span::new(span.start() + offset, span.end() + offset))
        .collect();
    assert_eq!(separator_spans(&actual), expected_shifted);
    let errors = ErrorCollector::new();
    actual.validate_with_alignment(&errors, TranscriptName::Anonymous);
    let diagnoses: Vec<_> = errors
        .into_vec()
        .into_iter()
        .filter(|error| error.code == ErrorCode::LeadingSpaceOnMainTier)
        .map(|error| {
            // Boundary behavior: validation owns absolute spans, not source text.
            assert!(
                error.context.is_none(),
                "validation must not invent source context"
            );
            let span = error.location.span().expect("located diagnostic");
            if offset == 0 {
                let diagnostic = SourceDiagnostic::with_index(error, &SourceIndex::new(source));
                let context = diagnostic.context().unwrap();
                assert_eq!(
                    &context.source_text()[context.span().to_range()],
                    &source[span.start()..span.end()],
                );
            }
            span
        })
        .collect();
    let is_ca = source
        .lines()
        .any(|line| line.starts_with("@Options:") && line.contains("CA"));
    assert_eq!(diagnoses, if is_ca { vec![] } else { expected_shifted });
    let serialized = actual.to_chat_string();
    assert!(!serialized.contains(":\t "));
    match parse_errors.as_slice() {
        [] => Serialization::Clean(serialized),
        _ => Serialization::Recovered,
    }
}

#[test]
fn separators_keep_source_spans_and_ca_policy_through_both_backends() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E758.md")
        .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    let canonical = talkbank_parser::TreeSitterParser::new().unwrap();
    for example in spec.examples() {
        let source = example.chat.as_str();
        let expected: Vec<_> = source
            .match_indices(":\t")
            .filter_map(|(start, _)| {
                let start = start + 2;
                let count = source[start..]
                    .bytes()
                    .take_while(|byte| *byte == b' ')
                    .count();
                (count > 0).then(|| Span::new(start, start + count))
            })
            .collect();
        for offset in [0, 200] {
            match (
                verify_separator_source(&parser, source, offset, &expected),
                verify_separator_source(&canonical, source, offset, &expected),
            ) {
                (Serialization::Clean(actual), Serialization::Clean(reference)) => {
                    assert_eq!(actual, reference);
                }
                // Backends can reject the same malformed content at different
                // phases. This test asserts source provenance for both; it only
                // compares serialization when neither parser recovered.
                (Serialization::Recovered, _) | (_, Serialization::Recovered) => {}
            }
        }
    }
}

/// A reference witness carries CA policy; the negative control removes only it.
#[derive(Clone, Copy, Debug)]
enum ReferenceSeparatorPolicy {
    Ca,
    Ordinary,
}

impl ReferenceSeparatorPolicy {
    fn source(self, reference: &str) -> String {
        match self {
            Self::Ca => reference.to_owned(),
            Self::Ordinary => {
                let option = "@Options:\t  CA\n";
                assert_eq!(reference.matches(option).count(), 1);
                reference.replacen(option, "", 1)
            }
        }
    }
}

#[test]
fn authored_padding_reference_requires_ca_policy_and_preserves_content() {
    const REFERENCE: &str = include_str!("../../../../corpus/reference/ca/separator-padding.cha");
    fn check(parser: &impl ChatParser, source: &str, policy: ReferenceSeparatorPolicy) -> ChatFile {
        let errors = ErrorCollector::new();
        let mut file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .unwrap();
        assert!(errors.is_empty(), "{policy:?}: {:?}", errors.into_vec());
        let spans = separator_spans(&file);
        let expected_count = match policy {
            ReferenceSeparatorPolicy::Ca => 12,
            ReferenceSeparatorPolicy::Ordinary => 11,
        };
        assert_eq!(spans.len(), expected_count);
        for span in &spans {
            assert_eq!(&source[span.start()..span.end()], "  ");
        }
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let diagnoses = errors.into_vec();
        match policy {
            ReferenceSeparatorPolicy::Ca => assert!(diagnoses.is_empty(), "{diagnoses:?}"),
            ReferenceSeparatorPolicy::Ordinary => {
                assert_eq!(diagnoses.len(), spans.len(), "{diagnoses:?}");
                for (diagnostic, span) in diagnoses.iter().zip(&spans) {
                    assert_eq!(diagnostic.code, ErrorCode::LeadingSpaceOnMainTier);
                    assert_eq!(diagnostic.location.span(), Some(*span));
                }
            }
        }
        let canonical = file.to_chat_string();
        assert!(!canonical.contains(":\t "));
        let errors = ErrorCollector::new();
        let mut reparsed = parser
            .parse_chat_file(&canonical, 0, &errors)
            .into_option()
            .unwrap();
        assert!(errors.is_empty(), "{:?}", errors.into_vec());
        reparsed.validate_with_alignment(&errors, TranscriptName::Anonymous);
        assert!(errors.is_empty(), "{:?}", errors.into_vec());
        assert_eq!(reparsed.to_chat_string(), canonical);
        assert!(
            file.semantic_eq(&reparsed),
            "separator normalization must preserve meaning"
        );
        file
    }
    let native = talkbank_parser_re2c::Re2cParser::new();
    let tree_sitter = talkbank_parser::TreeSitterParser::new().unwrap();
    for policy in [
        ReferenceSeparatorPolicy::Ca,
        ReferenceSeparatorPolicy::Ordinary,
    ] {
        for newline in ["\n", "\r\n", "\r"] {
            let source = policy.source(REFERENCE).replace('\n', newline);
            let native_file = check(&native, &source, policy);
            let tree_sitter_file = check(&tree_sitter, &source, policy);
            assert!(native_file.semantic_eq(&tree_sitter_file));
        }
    }
}

#[test]
fn content_and_wor_separators_keep_exact_source_locations() {
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    use talkbank_model::model::{DependentTier, dependent_tier::wor::WorItem};
    const SEPARATORS: &str = include_str!("../../../../corpus/reference/content/separators.cha");
    const INTONATION: &str = include_str!("../../../../corpus/reference/ca/intonation.cha");
    const WOR: &str = include_str!("../../../../corpus/reference/content/split-metadata.cha");
    let nested = SEPARATORS.replace("well , I", "<well ,> [/] I");
    fn check(parser: &impl ChatParser, source: &str, origin: usize, expected: (usize, usize)) {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, origin, &errors)
            .into_option()
            .unwrap();
        assert!(
            errors.is_empty(),
            "{}: {:?}",
            parser.parser_name(),
            errors.to_vec()
        );
        let mut counts = (0, 0);
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                if let WordItem::Separator(separator) = item {
                    let span = separator.location().span().expect("located separator");
                    assert_eq!(
                        &source[span.start() - origin..span.end() - origin],
                        separator.to_chat_string()
                    );
                    counts.0 += 1;
                }
            });
            for entry in &utterance.dependent_tiers {
                if let DependentTier::Wor(tier) = &entry.tier {
                    for item in &tier.items {
                        if let WorItem::Separator { text, location } = item {
                            let span = location.span().expect("located WOR separator");
                            assert_eq!(&source[span.start() - origin..span.end() - origin], text);
                            counts.1 += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(counts, expected, "{}", parser.parser_name());
    }
    let native = talkbank_parser_re2c::Re2cParser::new();
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    for (fixture, expected) in [
        (SEPARATORS, (3, 0)),
        (&nested, (3, 0)),
        (INTONATION, (10, 0)),
        (WOR, (1, 2)),
    ] {
        for newline in ["\n", "\r\n", "\r"] {
            let source = fixture.replace('\n', newline);
            for origin in [0, 200] {
                check(&native, &source, origin, expected);
                check(&tree, &source, origin, expected);
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ColonPlacement {
    Standalone,
    Attached,
}

#[test]
fn authored_colon_reference_distinguishes_separator_from_lengthening() {
    use talkbank_model::Separator;
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    const REFERENCE: &str =
        include_str!("../../../../corpus/reference/content/colon-separator.cha");
    fn check(
        parser: &impl ChatParser,
        source: &str,
        origin: usize,
        placement: ColonPlacement,
    ) -> ChatFile {
        let errors = ErrorCollector::new();
        let mut file = parser
            .parse_chat_file(source, origin, &errors)
            .into_option()
            .unwrap();
        assert!(errors.is_empty(), "{placement:?}: {:?}", errors.to_vec());
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        assert!(errors.is_empty(), "{placement:?}: {:?}", errors.to_vec());
        let mut separators = Vec::new();
        let mut lengthenings = Vec::new();
        for utterance in file.utterances() {
            walk_words(
                &utterance.main.content.content,
                None,
                &mut |item| match item {
                    WordItem::Separator(separator) => {
                        assert!(matches!(separator, Separator::Colon { .. }));
                        let span = separator.location().span().unwrap();
                        assert_eq!(&source[span.start() - origin..span.end() - origin], ":");
                        separators.push(span);
                    }
                    WordItem::Word(word) => {
                        for item in word.content().iter() {
                            if let talkbank_model::WordContent::Lengthening(run) = item {
                                assert_eq!(run.count.get(), 1);
                                let span = run.location.span().unwrap();
                                assert_eq!(
                                    &source[span.start() - origin..span.end() - origin],
                                    ":"
                                );
                                lengthenings.push(span);
                            }
                        }
                    }
                    WordItem::ReplacedWord(_) => panic!("this spec has no replacement"),
                },
            );
        }
        let (expected_count, expected_lengthenings, expected_line) = match placement {
            ColonPlacement::Standalone => (1, 0, "*CHI:\thello : world ."),
            ColonPlacement::Attached => (0, 1, "*CHI:\thello: world ."),
        };
        assert_eq!(separators.len(), expected_count);
        assert_eq!(lengthenings.len(), expected_lengthenings);
        let colon_start = origin
            + match placement {
                ColonPlacement::Standalone => source.find("hello : world").unwrap() + 6,
                ColonPlacement::Attached => source.find("hello: world").unwrap() + 5,
            };
        let expected_span = Span::new(colon_start, colon_start + 1);
        for span in separators.iter().chain(&lengthenings) {
            assert_eq!(*span, expected_span);
        }
        let canonical = file.to_chat_string();
        assert!(
            canonical.lines().any(|line| line == expected_line),
            "{canonical}"
        );
        let reparsed = parser
            .parse_chat_file(&canonical, 0, &errors)
            .into_option()
            .unwrap();
        assert!(errors.is_empty(), "{:?}", errors.to_vec());
        assert!(file.semantic_eq(&reparsed));
        file
    }
    let native = talkbank_parser_re2c::Re2cParser::new();
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    for placement in [ColonPlacement::Standalone, ColonPlacement::Attached] {
        let source = match placement {
            ColonPlacement::Standalone => REFERENCE.to_owned(),
            ColonPlacement::Attached => {
                assert_eq!(REFERENCE.matches("hello : world").count(), 1);
                REFERENCE.replacen("hello : world", "hello: world", 1)
            }
        };
        for newline in ["\n", "\r\n", "\r"] {
            let source = source.replace('\n', newline);
            for origin in [0, 200] {
                let actual = check(&native, &source, origin, placement);
                let expected = check(&tree, &source, origin, placement);
                assert!(actual.semantic_eq(&expected));
            }
        }
    }
}
