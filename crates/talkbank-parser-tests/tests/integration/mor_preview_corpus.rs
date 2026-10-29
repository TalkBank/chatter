//! MOR mismatch previews keep lexical positions separate from terminators.

use talkbank_model::model::TranscriptName;
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
use talkbank_parser_tests::{error_specs, repo_paths::workspace_root};

#[derive(Clone, Copy, Debug)]
enum PreviewCase {
    TooFew,
    TooMany,
    MatchedCount,
}

impl PreviewCase {
    fn specimen(self) -> (&'static str, &'static str, Vec<ErrorCode>) {
        match self {
            Self::TooFew => (
                "A question with a missing MOR item",
                "Alignment (index: main | mor)\n  0: I | pro|I\n  1: want | v|want\n  2: cookie | <none>\nend: ? | .\n",
                vec![
                    ErrorCode::MorTerminatorValueMismatch,
                    ErrorCode::MorCountMismatchTooFew,
                ],
            ),
            Self::TooMany => (
                "A question with an extra MOR item",
                "Alignment (index: main | mor)\n  0: want | pro|I\n  1: cookie | v|want\n  2: <none> | n|cookie\nend: ? | .\n",
                vec![
                    ErrorCode::MorTerminatorValueMismatch,
                    ErrorCode::MorCountMismatchTooMany,
                ],
            ),
            Self::MatchedCount => (
                "A question with a period MOR terminator",
                "Alignment (index: main | mor)\n  0: huh | co|huh\nend: ? | .\n",
                vec![ErrorCode::MorTerminatorValueMismatch],
            ),
        }
    }
}

#[test]
fn mor_previews_separate_lexical_mismatches_from_terminators() {
    let specs = error_specs::load(workspace_root()).expect("authored specs");
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E716.md")
        .expect("spec");
    let tree = talkbank_parser::TreeSitterParser::new().expect("parser");
    let native = talkbank_parser_re2c::Re2cParser::new();
    for case in [
        PreviewCase::TooFew,
        PreviewCase::TooMany,
        PreviewCase::MatchedCount,
    ] {
        let (title, expected, codes) = case.specimen();
        let example = spec
            .examples()
            .iter()
            .find(|example| example.title.as_deref() == Some(title))
            .expect("named case");
        fn check(parser: &impl ChatParser, source: &str, codes: &[ErrorCode], expected: &str) {
            let findings = ErrorCollector::new();
            let _admission = talkbank_transform::parse_validated_with_parser(
                parser,
                source,
                talkbank_model::validation::ValidationPolicy::new(
                    talkbank_model::RuleSelection::new(),
                    talkbank_model::validation::AlignmentValidation::IncludeTierAlignment,
                ),
                TranscriptName::Anonymous,
                &findings,
            );
            let findings = findings.into_vec();
            assert_eq!(
                findings.iter().map(|error| error.code).collect::<Vec<_>>(),
                codes,
                "{}",
                parser.parser_name()
            );
            let mismatch = findings
                .iter()
                .find(|error| error.code == ErrorCode::MorTerminatorValueMismatch)
                .expect("terminator mismatch");
            let preview = mismatch
                .suggestion
                .as_deref()
                .expect("suggestion")
                .split_once("Alignment preview:\n")
                .expect("preview")
                .1;
            assert_eq!(preview, expected, "{}", parser.parser_name());
        }
        check(&tree, example.chat.as_str(), &codes, expected);
        check(&native, example.chat.as_str(), &codes, expected);
    }
}
