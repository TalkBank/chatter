//! A successful comparison owns two independently admitted parse observations.

use talkbank_model::{ErrorCode, ErrorCollector, ParseOutcome, SemanticDiff, SemanticEq, Severity};

/// The model cannot enter semantic comparison without its own parse diagnostics.
struct CleanModel<T>(T);

impl<T> CleanModel<T> {
    fn observe(parse: impl FnOnce(&ErrorCollector) -> ParseOutcome<T>) -> Result<Self, String> {
        let errors = ErrorCollector::new();
        let outcome = parse(&errors);
        let faults: Vec<_> = errors
            .into_vec()
            .into_iter()
            .filter(|error| {
                error.severity == Severity::Error || error.code == ErrorCode::InternalError
            })
            .collect();
        if !faults.is_empty() {
            return Err(format!("parse findings: {faults:?}"));
        }
        match outcome {
            ParseOutcome::Parsed(model) => Ok(Self(model)),
            ParseOutcome::Rejected => Err("rejected without a model".to_owned()),
        }
    }
}

/// Warnings need not agree; error-bearing or rejected results are not valid controls.
pub(super) fn compare_parses<T: SemanticEq + SemanticDiff>(
    left: impl FnOnce(&ErrorCollector) -> ParseOutcome<T>,
    right: impl FnOnce(&ErrorCollector) -> ParseOutcome<T>,
) -> Result<(), String> {
    match (CleanModel::observe(left), CleanModel::observe(right)) {
        (Ok(CleanModel(left)), Ok(CleanModel(right))) => {
            if left.semantic_eq(&right) {
                Ok(())
            } else {
                Err(format!(
                    "semantic mismatch: {}",
                    left.semantic_diff(&right).render()
                ))
            }
        }
        (Err(left), Err(right)) => Err(format!("tree-sitter {left}; re2c {right}")),
        (Err(left), Ok(_)) => Err(format!("tree-sitter {left}")),
        (Ok(_), Err(right)) => Err(format!("re2c {right}")),
    }
}

/// Retain the whole failure inventory and refuse an empty observation set.
#[derive(Default)]
pub(super) struct ComparisonBatch {
    observed: usize,
    failures: Vec<String>,
}

impl ComparisonBatch {
    pub(super) fn record(&mut self, label: String, comparison: Result<(), String>) {
        self.observed += 1;
        if let Err(failure) = comparison {
            self.failures.push(format!("{label}: {failure}"));
        }
    }

    pub(super) fn finish(self) -> Result<(), String> {
        eprintln!(
            "{} comparisons; {} failures",
            self.observed,
            self.failures.len()
        );
        if self.observed == 0 {
            Err("no comparisons were observed".to_owned())
        } else if self.failures.is_empty() {
            Ok(())
        } else {
            Err(self.failures.join("\n"))
        }
    }
}

#[test]
fn planted_mismatches_rejections_and_errors_cannot_pass() {
    use talkbank_model::{ErrorSink, ParseError};
    use talkbank_parser::TreeSitterParser;

    let parser = TreeSitterParser::new().expect("parser");
    // Existing word-equivalence controls, parsed through the real API.
    let word = parser.parse_word("hello").expect("word control");
    let different = parser.parse_word("ice+cream").expect("distinct control");
    let accepted = || ParseOutcome::Parsed(word.clone());
    assert!(compare_parses(|_| accepted(), |_| accepted()).is_ok());
    for reverse in [false, true] {
        let changed = || ParseOutcome::Parsed(different.clone());
        let mismatch = if reverse {
            compare_parses(|_| changed(), |_| accepted())
        } else {
            compare_parses(|_| accepted(), |_| changed())
        };
        assert!(mismatch.is_err());
        let rejected = if reverse {
            compare_parses(|_| ParseOutcome::Rejected, |_| accepted())
        } else {
            compare_parses(|_| accepted(), |_| ParseOutcome::Rejected)
        };
        assert!(rejected.is_err());
        for (code, severity) in [
            (ErrorCode::UnparsableContent, Severity::Error),
            (ErrorCode::InternalError, Severity::Error),
            (ErrorCode::InternalError, Severity::Warning),
        ] {
            let diagnosed = |errors: &ErrorCollector| {
                errors.report(ParseError::at_location(
                    code,
                    severity,
                    talkbank_model::Location::Unlocated,
                    "planted finding",
                ));
                accepted()
            };
            let result = if reverse {
                compare_parses(diagnosed, |_| accepted())
            } else {
                compare_parses(|_| accepted(), diagnosed)
            };
            assert!(result.is_err(), "{code:?} {severity:?} reverse={reverse}");
        }
    }
    assert!(
        compare_parses::<talkbank_model::model::Word>(
            |_| ParseOutcome::Rejected,
            |_| ParseOutcome::Rejected,
        )
        .is_err()
    );
    // Warning identity is deliberately not the backend contract.
    assert!(
        compare_parses(
            |errors| {
                errors.report(ParseError::at_location(
                    ErrorCode::MediaFilenameNonCanonicalUnicode,
                    Severity::Warning,
                    talkbank_model::Location::Unlocated,
                    "planted warning",
                ));
                accepted()
            },
            |_| accepted()
        )
        .is_ok()
    );
    assert!(ComparisonBatch::default().finish().is_err());
    let mut batch = ComparisonBatch::default();
    batch.record("valid control".to_owned(), Ok(()));
    batch.record(
        "planted mismatch".to_owned(),
        Err("different models".to_owned()),
    );
    assert!(batch.finish().is_err());
}
