//! Actions cannot silently consume suffixes that require lexical word content.

use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome, Severity};

#[test]
fn action_suffixes_require_a_word_body_through_public_file_and_tier_routes() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E342_action_suffixes.md")
        .unwrap();
    assert_eq!(spec.examples().len(), 7);
    let native = talkbank_parser_re2c::Re2cParser::new();
    let tree = talkbank_parser::TreeSitterParser::new().unwrap();
    let mut failures = Vec::new();
    for (index, example) in spec.examples().iter().enumerate() {
        let source = example.chat.as_str();
        let main = source.lines().find(|line| line.starts_with('*')).unwrap();
        let invalid = example.claim.demonstrates_own_code();
        for (name, observations) in [
            ("native", observe(&native, source, main)),
            ("tree", observe(&tree, source, main)),
        ] {
            for (route, origin, parsed, errors) in observations {
                if (!invalid && !parsed)
                    || errors
                        .iter()
                        .any(|error| error.code == ErrorCode::InternalError)
                    || errors.iter().any(|error| error.severity == Severity::Error) != invalid
                {
                    failures.push(format!(
                        "{name} {route:?} example {} origin {origin}: {errors:?}",
                        index + 1
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[derive(Debug)]
enum Route {
    File,
    MainTier,
    Utterance,
}

fn observe(
    parser: &impl ChatParser,
    source: &str,
    main: &str,
) -> Vec<(Route, usize, bool, Vec<talkbank_model::ParseError>)> {
    let mut observations = Vec::new();
    for origin in [0, 200] {
        for route in [Route::File, Route::MainTier, Route::Utterance] {
            let errors = ErrorCollector::new();
            let parsed = match route {
                Route::File => matches!(
                    parser.parse_chat_file(source, origin, &errors),
                    ParseOutcome::Parsed(_)
                ),
                Route::MainTier => matches!(
                    parser.parse_main_tier(main, origin, &errors),
                    ParseOutcome::Parsed(_)
                ),
                Route::Utterance => matches!(
                    parser.parse_utterance(main, origin, &errors),
                    ParseOutcome::Parsed(_)
                ),
            };
            observations.push((route, origin, parsed, errors.into_vec()));
        }
    }
    observations
}
