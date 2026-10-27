//! Authored dependency edges retain their diagnostic meaning across backends.

use talkbank_model::model::TranscriptName;
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};

struct Case {
    example: usize,
    required: &'static [ErrorCode],
    forbidden: &'static [ErrorCode],
}

#[test]
fn gra_cycle_specs_preserve_all_edges_and_independent_index_findings() {
    use ErrorCode::{
        GraCircularDependency as Cycle, GraMultipleRoots as Roots, GraNoRoot as NoRoot,
        GraNonSequentialIndex as Index,
    };
    let cases = [
        Case {
            example: 1,
            required: &[Cycle],
            forbidden: &[NoRoot, Index],
        },
        Case {
            example: 2,
            required: &[Cycle],
            forbidden: &[NoRoot, Index],
        },
        Case {
            example: 3,
            required: &[Cycle],
            forbidden: &[NoRoot, Index],
        },
        Case {
            example: 4,
            required: &[],
            forbidden: &[Cycle, NoRoot, Index, Roots],
        },
        Case {
            example: 5,
            required: &[],
            forbidden: &[Cycle, NoRoot, Index, Roots],
        },
        Case {
            example: 6,
            required: &[Cycle],
            forbidden: &[NoRoot, Index],
        },
        Case {
            example: 7,
            required: &[Cycle, Index],
            forbidden: &[NoRoot],
        },
        Case {
            example: 8,
            required: &[Cycle, Index],
            forbidden: &[NoRoot],
        },
        Case {
            example: 9,
            required: &[Index],
            forbidden: &[Cycle, NoRoot, Roots],
        },
        Case {
            example: 10,
            required: &[Cycle],
            forbidden: &[NoRoot, Index],
        },
    ];

    fn check(parser: &impl ChatParser, source: &str, case: &Case) {
        let errors = ErrorCollector::new();
        let mut file = parser
            .parse_chat_file(source, 41, &errors)
            .into_option()
            .unwrap();
        assert!(
            errors.is_empty(),
            "{} example {}: {:?}",
            parser.parser_name(),
            case.example,
            errors.to_vec()
        );
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let findings = errors.into_vec();
        for code in case.required {
            let matching = findings
                .iter()
                .filter(|error| error.code == *code)
                .collect::<Vec<_>>();
            assert_eq!(
                matching.len(),
                1,
                "{} example {} {code}: {findings:?}",
                parser.parser_name(),
                case.example
            );
            let span = matching[0].location.span().unwrap();
            assert!(span.start() >= 41 && span.end() <= 41 + source.len());
            assert!(source[span.start() - 41..].starts_with("%gra:"));
        }
        for code in case.forbidden {
            assert!(
                !findings.iter().any(|error| error.code == *code),
                "{} example {} {code}: {findings:?}",
                parser.parser_name(),
                case.example
            );
        }
        let wire = serde_json::to_value(&file).unwrap();
        let mut restored: talkbank_model::model::ChatFile = serde_json::from_value(wire).unwrap();
        let errors = ErrorCollector::new();
        restored.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let findings = errors.into_vec();
        for code in case.required {
            let matching = findings
                .iter()
                .filter(|error| error.code == *code)
                .collect::<Vec<_>>();
            assert_eq!(
                matching.len(),
                1,
                "JSON example {} {code}: {findings:?}",
                case.example
            );
            assert!(matching[0].location.span().is_none());
        }
        for code in case.forbidden {
            assert!(
                !findings.iter().any(|error| error.code == *code),
                "JSON example {} {code}: {findings:?}",
                case.example
            );
        }
    }

    let primary = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    for case in cases {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "tests/error_corpus/validation_errors/E724_{}.cha",
            case.example
        ));
        let source = std::fs::read_to_string(path).unwrap();
        check(&primary, &source, &case);
        check(&native, &source, &case);
    }
}
