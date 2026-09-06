//! Separator provenance, CA exemption and canonical serialization at file boundaries.

use talkbank_model::model::{ChatFile, Line, TranscriptName, WriteChat};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome, Span};

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
                (count > 0).then(|| Span::from_usize(start, start + count))
            })
            .collect();
        for offset in [0, 200] {
            let errors = ErrorCollector::new();
            let ParseOutcome::Parsed(mut actual) = parser.parse_chat_file(source, offset, &errors)
            else {
                panic!("separator spaces must not prevent parsing");
            };
            let expected_shifted: Vec<_> = expected
                .iter()
                .map(|span| {
                    Span::from_usize(span.start as usize + offset, span.end as usize + offset)
                })
                .collect();
            assert_eq!(separator_spans(&actual), expected_shifted);
            actual.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let diagnoses: Vec<_> = errors
                .into_vec()
                .into_iter()
                .filter(|error| error.code == ErrorCode::LeadingSpaceOnMainTier)
                .map(|error| error.location.span)
                .collect();
            let is_ca = source
                .lines()
                .any(|line| line.starts_with("@Options:") && line.contains("CA"));
            assert_eq!(diagnoses, if is_ca { vec![] } else { expected_shifted });

            let serialized = actual.to_chat_string();
            assert!(!serialized.contains(":\t "));
            let reference = canonical.parse_chat_file(source).expect_built();
            assert_eq!(serialized, reference.to_chat_string());
        }
    }
}
