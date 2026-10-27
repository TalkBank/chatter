//! E371 scope and group geometry come from the authored error specification.

use talkbank_model::model::{ContentStructure, Descend, GroupKind, MainTier, TranscriptName};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, Span};
use talkbank_parser_tests::repo_paths::workspace_root;

fn assert_group_locations(tier: &MainTier, source: &str, origin: usize) {
    let expected: Vec<_> = source
        .char_indices()
        .filter_map(|(start, ch)| {
            let close = match ch {
                '‹' => '›',
                '〔' => '〕',
                _ => return None,
            };
            let end = start + source[start..].find(close).unwrap() + close.len_utf8();
            Some(Span::new(origin + start, origin + end))
        })
        .collect();
    let mut observed = Vec::new();
    for item in tier.content.content.iter() {
        item.structure().walk(&mut |structure| {
            if let ContentStructure::Group(group) = structure {
                match group.kind() {
                    GroupKind::Pho | GroupKind::Sin => {
                        observed.push(group.location().span().expect("source group location"))
                    }
                    GroupKind::Angle | GroupKind::Quotation => {}
                }
            }
            Descend::Into
        });
    }
    assert_eq!(observed, expected, "{source}");
}

fn check_phonological_group_contracts(parser: &impl ChatParser) {
    let specs = talkbank_parser_tests::error_specs::load(workspace_root()).unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E371.md")
        .unwrap();
    assert_eq!(spec.examples().len(), 12, "all authored scope cases");
    for example in spec.examples() {
        let source = example.chat.as_str();
        // These fixtures have disjoint phonological groups; the outside-pause
        // control deliberately includes a pause elsewhere on the same tier.
        let expected: Vec<_> = source
            .match_indices('‹')
            .filter_map(|(start, _)| {
                let end = start + source[start..].find('›').unwrap() + '›'.len_utf8();
                source[start..end]
                    .contains("(.")
                    .then_some(Span::new(start, end))
            })
            .collect();
        assert_eq!(
            !expected.is_empty(),
            example.claim.demonstrates_own_code(),
            "authored E371 claim"
        );
        for origin in [0, 41] {
            let errors = ErrorCollector::new();
            let mut file = parser
                .parse_chat_file(source, origin, &errors)
                .into_option()
                .unwrap();
            assert!(
                errors.is_empty(),
                "{} {:?}: {:?}",
                parser.parser_name(),
                example.title,
                errors.to_vec()
            );
            let tier = &file.utterances().next().unwrap().main;
            assert_group_locations(tier, source, origin);
            file.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let findings: Vec<_> = errors
                .into_vec()
                .into_iter()
                .filter(|error| error.code == ErrorCode::PauseInPhoGroup)
                .collect();
            let spans: Vec<_> = findings.iter().map(|error| error.location.span()).collect();
            assert_eq!(
                spans,
                expected
                    .iter()
                    .map(|span| Some(Span::new(span.start() + origin, span.end() + origin)))
                    .collect::<Vec<_>>(),
                "{} {:?}",
                parser.parser_name(),
                example.title
            );
            let wire = serde_json::to_value(&file).unwrap();
            let mut restored: talkbank_model::model::ChatFile =
                serde_json::from_value(wire).unwrap();
            let errors = ErrorCollector::new();
            restored.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let findings: Vec<_> = errors
                .into_vec()
                .into_iter()
                .filter(|error| error.code == ErrorCode::PauseInPhoGroup)
                .collect();
            assert_eq!(findings.len(), expected.len());
            assert!(findings.iter().all(|error| error.location.span().is_none()));
        }
        let line = source
            .lines()
            .find(|line| line.starts_with("*CHI:"))
            .unwrap();
        let errors = ErrorCollector::new();
        let tier = parser
            .parse_main_tier(line, 123, &errors)
            .into_option()
            .unwrap();
        assert_group_locations(&tier, line, 123);
        let utterance = parser
            .parse_utterance(line, 123, &errors)
            .into_option()
            .unwrap();
        assert_group_locations(&utterance.main, line, 123);
        assert!(
            errors.is_empty(),
            "{} {:?}: {:?}",
            parser.parser_name(),
            example.title,
            errors.to_vec()
        );
    }
}

#[test]
fn phonological_group_scope_and_locations_survive_both_source_backends() {
    check_phonological_group_contracts(&talkbank_parser::TreeSitterParser::new().unwrap());
    check_phonological_group_contracts(&talkbank_parser_re2c::Re2cParser::new());
}
