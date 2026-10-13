//! Source observations and the one-way error-detection contract.
//!
//! Diagnostic identity is a measurement, not an acceptance condition. Keep
//! actual severity and stage; a warning cannot stand in for a missed error.

use std::collections::BTreeSet;
use std::fmt;

use talkbank_model::errors::diagnostic_kind::{DiagnosticKind, kind_of};
use talkbank_model::{ErrorCode, ErrorCollector, ParseError, Severity};

/// Specification examples are numbered from one, independently of siblings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExampleOrdinal(usize);

impl ExampleOrdinal {
    pub(super) fn from_zero_based(index: usize) -> Self {
        Self(index + 1)
    }
}

impl fmt::Display for ExampleOrdinal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The source specification and its unconditional, one-based example ordinal.
#[derive(Clone, Debug)]
pub(super) struct SpecLabel {
    file: String,
    case: ExampleOrdinal,
}

impl SpecLabel {
    pub(super) fn new(file: &str, case: ExampleOrdinal) -> Self {
        Self {
            file: file.to_owned(),
            case,
        }
    }
}

impl fmt::Display for SpecLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.file, self.case)
    }
}

/// Findings captured before model validation starts.
pub(super) struct ParsingFindings(Vec<ParseError>);

impl ParsingFindings {
    pub(super) fn from_sink(sink: ErrorCollector) -> Self {
        Self(sink.into_vec())
    }
}

/// Findings captured by the shared validator, not by the parser.
pub(super) struct ValidationFindings(Vec<ParseError>);

impl ValidationFindings {
    pub(super) fn from_sink(sink: ErrorCollector) -> Self {
        Self(sink.into_vec())
    }
}

/// An immutable observation; the two phase types cannot be interchanged.
pub(super) struct BackendReport {
    parsing: ParsingFindings,
    validation: ValidationFindings,
}

impl BackendReport {
    pub(super) fn new(parsing: ParsingFindings, validation: ValidationFindings) -> Self {
        Self {
            parsing,
            validation,
        }
    }

    pub(super) fn diagnostics(&self) -> impl Iterator<Item = &ParseError> {
        self.parsing.0.iter().chain(&self.validation.0)
    }

    /// Code-set equality remains informative, but never determines acceptance.
    pub(super) fn codes(&self) -> BTreeSet<ErrorCode> {
        self.diagnostics().map(|finding| finding.code).collect()
    }

    fn first_error(&self) -> Option<ErrorFinding<'_>> {
        self.diagnostics()
            .find(|finding| finding.severity == Severity::Error)
            .map(ErrorFinding)
    }

    fn internal_failure(&self) -> Option<&ParseError> {
        self.diagnostics()
            .find(|finding| kind_of(finding.code) == DiagnosticKind::InternalFailure)
    }
}

impl fmt::Display for BackendReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (name, findings) in [
            ("parse", &self.parsing.0),
            ("validation", &self.validation.0),
        ] {
            write!(f, "{name} [")?;
            for (index, finding) in findings.iter().enumerate() {
                if index > 0 {
                    f.write_str(", ")?;
                }
                write!(
                    f,
                    "{} {:?} {:?}",
                    finding.code, finding.severity, finding.location.span
                )?;
            }
            f.write_str("] ")?;
        }
        Ok(())
    }
}

/// Only observing actual error severity can issue this evidence.
pub(super) struct ErrorFinding<'report>(&'report ParseError);

impl ErrorFinding<'_> {
    pub(super) fn diagnostic(&self) -> &ParseError {
        self.0
    }
}

/// A backend identity, not an interchangeable display string.
pub(super) enum Backend {
    TreeSitter,
    Re2c,
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TreeSitter => "tree-sitter",
            Self::Re2c => "re2c",
        })
    }
}

/// Derived from immutable observations, with no baseline exemption route.
pub(super) enum Detection<'report> {
    /// Tree-sitter emitted no error; the implication has no antecedent.
    NotRequired,
    /// Both emitted an error, regardless of code or recovery stage.
    Reported,
    /// Tree-sitter's error has no error-severity counterpart in re2c.
    Missing(ErrorFinding<'report>),
    /// Tool failure is broken measurement, never successful rejection proof.
    ToolFailure {
        backend: Backend,
        finding: &'report ParseError,
    },
}

/// One source example, measured through both native backends.
pub(super) struct CaseReport {
    pub(super) label: SpecLabel,
    pub(super) tree_sitter: BackendReport,
    pub(super) re2c: BackendReport,
}

impl CaseReport {
    pub(super) fn detection(&self) -> Detection<'_> {
        for (backend, report) in [
            (Backend::TreeSitter, &self.tree_sitter),
            (Backend::Re2c, &self.re2c),
        ] {
            if let Some(finding) = report.internal_failure() {
                return Detection::ToolFailure { backend, finding };
            }
        }
        match (self.tree_sitter.first_error(), self.re2c.first_error()) {
            (None, None | Some(_)) => Detection::NotRequired,
            (Some(_), Some(_)) => Detection::Reported,
            (Some(finding), None) => Detection::Missing(finding),
        }
    }

    pub(super) fn detail(&self) -> String {
        format!(
            "{}: tree-sitter {}; re2c {}",
            self.label, self.tree_sitter, self.re2c
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{ErrorSink, Span};

    fn findings(code: ErrorCode, severity: Severity) -> ErrorCollector {
        let sink = ErrorCollector::new();
        sink.report(ParseError::at_span(
            code,
            severity,
            Span::DUMMY,
            "instrument policy control",
        ));
        sink
    }

    fn observed(parse: ErrorCollector, validation: ErrorCollector) -> BackendReport {
        BackendReport::new(
            ParsingFindings::from_sink(parse),
            ValidationFindings::from_sink(validation),
        )
    }

    fn case(tree_sitter: BackendReport, re2c: BackendReport) -> CaseReport {
        CaseReport {
            label: SpecLabel::new("instrument", ExampleOrdinal::from_zero_based(0)),
            tree_sitter,
            re2c,
        }
    }

    fn canonical_error() -> BackendReport {
        observed(
            findings(ErrorCode::UnparsableContent, Severity::Error),
            ErrorCollector::new(),
        )
    }

    /// Survives: asymmetric policy, not diagnostic identity or implementation.
    #[test]
    fn different_error_codes_and_stages_satisfy_detection() {
        let report = case(
            canonical_error(),
            observed(
                ErrorCollector::new(),
                findings(ErrorCode::UnparsableUtterance, Severity::Error),
            ),
        );
        assert!(matches!(report.detection(), Detection::Reported));
        assert_ne!(report.tree_sitter.codes(), report.re2c.codes());
    }

    /// Survives: planted silence and warning-only output must both fail.
    #[test]
    fn silence_and_warning_only_output_cannot_satisfy_an_error() {
        for sink in [
            ErrorCollector::new(),
            findings(ErrorCode::UnparsableContent, Severity::Warning),
            findings(
                ErrorCode::MediaFilenameNonCanonicalUnicode,
                Severity::Warning,
            ),
        ] {
            let report = case(canonical_error(), observed(sink, ErrorCollector::new()));
            let Detection::Missing(finding) = report.detection() else {
                panic!("{}", report.detail());
            };
            assert_eq!(finding.diagnostic().code, ErrorCode::UnparsableContent);
        }
    }

    /// Survives: warnings and reverse-direction errors create no obligation.
    #[test]
    fn no_tree_sitter_error_has_no_one_way_obligation() {
        for sink in [
            ErrorCollector::new(),
            findings(
                ErrorCode::MediaFilenameNonCanonicalUnicode,
                Severity::Warning,
            ),
        ] {
            let report = case(observed(sink, ErrorCollector::new()), canonical_error());
            assert!(matches!(report.detection(), Detection::NotRequired));
        }
    }

    /// Survives: internal failure is not detection of an input error.
    #[test]
    fn internal_failure_refuses_measurement_in_either_backend() {
        let failed = || {
            observed(
                findings(ErrorCode::InternalError, Severity::Error),
                ErrorCollector::new(),
            )
        };
        for report in [
            case(failed(), canonical_error()),
            case(canonical_error(), failed()),
        ] {
            assert!(matches!(report.detection(), Detection::ToolFailure { .. }));
        }
    }
}
