//! Empty delimiters cannot stand in for the grammar's required contents.

use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseError, ParseOutcome, Severity};

#[derive(Clone, Copy, Debug)]
enum Api {
    File,
    Utterance,
    Main,
    Dependent,
    Pho,
    Sin,
}

fn observe(
    parser: &impl ChatParser,
    source: &str,
    api: Api,
    origin: usize,
) -> (bool, Vec<ParseError>) {
    let errors = ErrorCollector::new();
    let parsed = match api {
        Api::File => matches!(
            parser.parse_chat_file(source, origin, &errors),
            ParseOutcome::Parsed(_)
        ),
        Api::Utterance => matches!(
            parser.parse_utterance(source, origin, &errors),
            ParseOutcome::Parsed(_)
        ),
        Api::Main => matches!(
            parser.parse_main_tier(source, origin, &errors),
            ParseOutcome::Parsed(_)
        ),
        Api::Dependent => matches!(
            parser.parse_dependent_tier(source, origin, &errors),
            ParseOutcome::Parsed(_)
        ),
        Api::Pho => matches!(
            parser.parse_pho_tier(source, origin, &errors),
            ParseOutcome::Parsed(_)
        ),
        Api::Sin => matches!(
            parser.parse_sin_tier(source, origin, &errors),
            ParseOutcome::Parsed(_)
        ),
    };
    (parsed, errors.into_vec())
}

#[test]
fn required_container_contents_cannot_disappear_without_error() -> Result<(), String> {
    let primary = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let root = talkbank_parser_tests::repo_paths::workspace_root().join("corpus/reference");
    let mut failures = Vec::new();
    let mut observations = 0;
    for (fixture, written, emptied) in [
        ("annotation/groups-regular.cha", "<what in the world>", "<>"),
        ("content/quotation-groups-smart.cha", "“hello”", "“”"),
        ("annotation/groups-phonological.cha", "‹il y a›", "‹›"),
        ("annotation/groups-sign.cha", "〔bar baz〕", "〔〕"),
    ] {
        let control =
            std::fs::read_to_string(root.join(fixture)).map_err(|error| error.to_string())?;
        assert_eq!(
            control.matches(written).count(),
            1,
            "unique mutation target: {fixture}"
        );
        let invalid = control.replacen(written, emptied, 1);
        for (source, needle, faulty) in [(&control, written, false), (&invalid, emptied, true)] {
            let main = source
                .lines()
                .find(|line| line.starts_with('*') && line.contains(needle))
                .expect("reference owns the selected main tier");
            for (api, input) in [
                (Api::File, source.as_str()),
                (Api::Utterance, main),
                (Api::Main, main),
            ] {
                for origin in [0, "Préface café\n".len()] {
                    let reports = [
                        observe(&primary, input, api, origin),
                        observe(&native, input, api, origin),
                    ];
                    observations += 1;
                    for (backend, (parsed, diagnostics)) in
                        ["primary", "native"].into_iter().zip(reports)
                    {
                        let has_error = diagnostics
                            .iter()
                            .any(|error| error.severity == Severity::Error);
                        let tool_failure = diagnostics
                            .iter()
                            .any(|error| error.code == ErrorCode::InternalError);
                        if tool_failure || has_error != faulty || (!faulty && !parsed) {
                            failures.push(format!("{fixture} {api:?} origin={origin} faulty={faulty} {backend}: parsed={parsed}, {diagnostics:?}"));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(observations, 48);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[test]
fn dependent_group_contents_cannot_disappear_without_error() -> Result<(), String> {
    let primary = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let root = talkbank_parser_tests::repo_paths::workspace_root().join("corpus/reference");
    let mut failures = Vec::new();
    let mut observations = 0;
    for (fixture, original_prefix, prefix, written, emptied, body_api) in [
        (
            "tiers/pho-groupings.cha",
            "%pho:",
            "%pho:",
            "‹a b›",
            "‹›",
            Api::Pho,
        ),
        (
            "tiers/pho-groupings.cha",
            "%pho:",
            "%mod:",
            "‹a b›",
            "‹›",
            Api::Pho,
        ),
        (
            "annotation/groups-sign.cha",
            "%sin:",
            "%sin:",
            "〔c d〕",
            "〔〕",
            Api::Sin,
        ),
    ] {
        let reference =
            std::fs::read_to_string(root.join(fixture)).map_err(|error| error.to_string())?;
        assert_eq!(reference.matches(original_prefix).count(), 1);
        // %mod shares the phonology body grammar; retain the authored body while
        // also exercising its separate dependent-tier dispatch branch.
        let control = reference.replacen(original_prefix, prefix, 1);
        assert_eq!(
            control.matches(written).count(),
            1,
            "unique deletion target"
        );
        let invalid = control.replacen(written, emptied, 1);
        for (source, faulty) in [(&control, false), (&invalid, true)] {
            let main = source
                .lines()
                .find(|line| line.starts_with('*'))
                .expect("reference main tier");
            let dependent = source
                .lines()
                .find(|line| line.starts_with(prefix))
                .expect("reference dependent tier");
            let (_, body) = dependent.split_once('\t').expect("authored separator");
            let utterance = format!("{main}\n{dependent}\n");
            for (api, input) in [
                (Api::File, source.as_str()),
                (Api::Utterance, utterance.as_str()),
                (Api::Dependent, dependent),
                (body_api, body),
            ] {
                for origin in [0, "Préface café\n".len()] {
                    let reports = [
                        observe(&primary, input, api, origin),
                        observe(&native, input, api, origin),
                    ];
                    observations += 1;
                    for (backend, (parsed, diagnostics)) in
                        ["primary", "native"].into_iter().zip(reports)
                    {
                        let has_error = diagnostics
                            .iter()
                            .any(|error| error.severity == Severity::Error);
                        let tool_failure = diagnostics
                            .iter()
                            .any(|error| error.code == ErrorCode::InternalError);
                        if tool_failure || has_error != faulty || (!faulty && !parsed) {
                            failures.push(format!("{prefix} {api:?} origin={origin} faulty={faulty} {backend}: parsed={parsed}, {diagnostics:?}"));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(observations, 48);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
