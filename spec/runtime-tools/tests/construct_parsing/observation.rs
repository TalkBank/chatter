//! A model enters comparison only together with its own completed diagnostics.

use talkbank_model::{
    CompletedDiagnostics, ErrorCollector, ParseOutcome, SemanticDiff, SemanticEq, Severity,
};

struct CleanModel<T>(T);

impl<T> CleanModel<T> {
    fn observe(parse: impl FnOnce(&ErrorCollector) -> ParseOutcome<T>) -> Result<Self, String> {
        let sink = ErrorCollector::new();
        let outcome = parse(&sink);
        let completed = CompletedDiagnostics::admit(sink.into_vec())
            .map_err(|failure| format!("internal parse failure: {failure:?}"))?;
        let errors: Vec<_> = completed
            .diagnostics()
            .iter()
            .filter(|finding| finding.severity == Severity::Error)
            .collect();
        if !errors.is_empty() {
            return Err(format!("parse errors: {errors:?}"));
        }
        match outcome {
            ParseOutcome::Parsed(model) => Ok(Self(model)),
            ParseOutcome::Rejected => Err("rejected without a model".to_owned()),
        }
    }
}

/// Policy witnesses may inspect only a model admitted with its own diagnostics.
pub(super) fn check_parse<T>(
    parse: impl FnOnce(&ErrorCollector) -> ParseOutcome<T>,
    check: impl FnOnce(&T) -> Result<(), String>,
) -> Result<(), String> {
    let CleanModel(model) = CleanModel::observe(parse)?;
    check(&model)
}

pub(super) fn compare_parses<T: SemanticEq + SemanticDiff>(
    primary: impl FnOnce(&ErrorCollector) -> ParseOutcome<T>,
    native: impl FnOnce(&ErrorCollector) -> ParseOutcome<T>,
) -> Result<(), String> {
    match (CleanModel::observe(primary), CleanModel::observe(native)) {
        (Ok(CleanModel(left)), Ok(CleanModel(right))) if left.semantic_eq(&right) => Ok(()),
        (Ok(CleanModel(left)), Ok(CleanModel(right))) => Err(format!(
            "semantic mismatch: {}",
            left.semantic_diff(&right).render()
        )),
        (Err(left), Err(right)) => Err(format!("tree-sitter {left}; re2c {right}")),
        (Err(left), Ok(_)) => Err(format!("tree-sitter {left}")),
        (Ok(_), Err(right)) => Err(format!("re2c {right}")),
    }
}

#[test]
fn comparison_refuses_mismatch_rejection_and_error_bearing_models() -> Result<(), super::Failure> {
    use super::{Api, ConstructDeck};
    use generators::repo_paths::RepoRoot;
    use talkbank_model::{ChatParser, ErrorCode, ErrorSink, Location, ParseError};

    let deck = ConstructDeck::load(&RepoRoot::resolve(None)?)?;
    let mut words = deck.0.iter().filter(|case| matches!(case.api, Api::Word));
    let first = &words.next().ok_or("missing word control")?.input;
    let second = &words.next().ok_or("missing second word control")?.input;
    let parser = talkbank_parser::TreeSitterParser::new()?;
    let parse = |sink: &ErrorCollector| ChatParser::parse_word(&parser, first, 0, sink);
    let different = |sink: &ErrorCollector| ChatParser::parse_word(&parser, second, 0, sink);
    assert!(compare_parses(parse, parse).is_ok());
    assert!(compare_parses(parse, different).is_err());
    assert!(compare_parses(different, parse).is_err());
    assert!(compare_parses(parse, |_| ParseOutcome::Rejected).is_err());
    assert!(compare_parses(|_| ParseOutcome::Rejected, parse).is_err());
    assert!(
        compare_parses::<talkbank_model::Word>(
            |_| ParseOutcome::Rejected,
            |_| ParseOutcome::Rejected,
        )
        .is_err()
    );
    for (code, severity) in [
        (ErrorCode::UnparsableContent, Severity::Error),
        (ErrorCode::InternalError, Severity::Error),
        (ErrorCode::InternalError, Severity::Warning),
    ] {
        let diagnosed = |sink: &ErrorCollector| {
            sink.report(ParseError::at_location(
                code,
                severity,
                Location::Unlocated,
                "planted finding",
            ));
            parse(sink)
        };
        assert!(
            compare_parses(parse, diagnosed).is_err(),
            "{code:?} {severity:?}"
        );
        assert!(
            compare_parses(diagnosed, parse).is_err(),
            "{code:?} {severity:?}"
        );
    }
    let ordinary_warning = |sink: &ErrorCollector| {
        sink.report(ParseError::at_location(
            ErrorCode::MediaFilenameNonCanonicalUnicode,
            Severity::Warning,
            Location::Unlocated,
            "planted ordinary warning",
        ));
        parse(sink)
    };
    assert!(compare_parses(parse, ordinary_warning).is_ok());
    Ok(())
}
