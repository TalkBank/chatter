//! Parser-attempt admission shared by corpus comparison reporters.

use std::path::{Path, PathBuf};

use talkbank_model::{
    ChatFile, ChatParser, CompletedDiagnostics, ErrorCollector, InternalFailure, ParseOutcome,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

/// The only parser results a reporter may classify.
pub enum ComparisonModels<'a> {
    /// Both attempts completed, but the native parser rejected the input.
    NativeRejected,
    /// Both attempts completed with models, including ordinary input recovery.
    Built {
        tree_sitter: &'a ChatFile,
        re2c: &'a ChatFile,
    },
}

/// Both unsuppressed diagnostic batches have passed internal-failure admission.
pub struct CompletedParserPair {
    tree_sitter: ChatFile,
    re2c: ParseOutcome<ChatFile>,
    diagnostics: [CompletedDiagnostics; 2],
}

/// Failed attempts retain their complete diagnostic batch, including input findings.
#[derive(Debug, thiserror::Error)]
enum FailedParsers {
    #[error("Tree-sitter: {0}")]
    TreeSitter(InternalFailure),
    #[error("re2c: {0}")]
    Re2c(InternalFailure),
    #[error("Tree-sitter: {tree_sitter}; re2c: {re2c}")]
    Both {
        tree_sitter: InternalFailure,
        re2c: InternalFailure,
    },
}

/// A producer failure belongs to its selected source, never a data category.
#[derive(Debug, thiserror::Error)]
#[error("parser attempt failed for {}: {failure}", path.display())]
pub struct ParserPairFailure {
    path: PathBuf,
    failure: FailedParsers,
}

impl CompletedParserPair {
    /// Parse and admit both attempts before exposing either model or rejection.
    pub fn parse(
        path: &Path,
        source: &str,
        tree_sitter: &TreeSitterParser,
        re2c: &Re2cParser,
    ) -> Result<Self, ParserPairFailure> {
        let primary_findings = ErrorCollector::new();
        let primary = tree_sitter.parse_chat_file_streaming(source, &primary_findings);
        let native_findings = ErrorCollector::new();
        let native = re2c.parse_chat_file(source, 0, &native_findings);
        Self::admit(path, primary, primary_findings, native, native_findings)
    }

    fn admit(
        path: &Path,
        tree_sitter: ChatFile,
        primary_findings: ErrorCollector,
        re2c: ParseOutcome<ChatFile>,
        native_findings: ErrorCollector,
    ) -> Result<Self, ParserPairFailure> {
        // Evaluate both complete batches, even if the first contains a fault.
        let primary = CompletedDiagnostics::admit(primary_findings.into_vec());
        let native = CompletedDiagnostics::admit(native_findings.into_vec());
        let failure = match (primary, native) {
            (Ok(primary), Ok(native)) => {
                return Ok(Self {
                    tree_sitter,
                    re2c,
                    diagnostics: [primary, native],
                });
            }
            (Err(failure), Ok(_)) => FailedParsers::TreeSitter(failure),
            (Ok(_), Err(failure)) => FailedParsers::Re2c(failure),
            (Err(tree_sitter), Err(re2c)) => FailedParsers::Both { tree_sitter, re2c },
        };
        Err(ParserPairFailure {
            path: path.to_owned(),
            failure,
        })
    }

    /// Preserve ordinary diagnostics until the completed pair is classified.
    pub fn diagnostics(&self) -> &[CompletedDiagnostics; 2] {
        &self.diagnostics
    }

    /// Inspect classification while retaining the admitted diagnostic batches.
    pub fn models(&self) -> ComparisonModels<'_> {
        match &self.re2c {
            ParseOutcome::Parsed(re2c) => ComparisonModels::Built {
                tree_sitter: &self.tree_sitter,
                re2c,
            },
            ParseOutcome::Rejected => ComparisonModels::NativeRejected,
        }
    }
}

type ReportResult = Result<(), Box<dyn std::error::Error>>;
type PairParser<'a> = dyn FnMut(&Path, &str) -> Result<CompletedParserPair, ParserPairFailure> + 'a;

/// Run the same publication contract through each actual report implementation.
pub fn assert_report_admission(
    report: impl Fn(&Path, &[PathBuf], &Path, &mut PairParser<'_>) -> ReportResult,
) -> ReportResult {
    use talkbank_model::{ErrorCode, ErrorSink, ParseError, Severity, Span};

    #[derive(Clone, Copy, Debug)]
    enum FaultSite {
        TreeSitter,
        Re2c,
        Both,
    }
    #[derive(Clone, Copy, Debug)]
    enum NativeResult {
        Parsed,
        Rejected,
    }
    #[derive(Clone, Copy)]
    enum Destination {
        New,
        Existing,
    }
    let tree = tempfile::tempdir()?;
    let input = tree.path().join("groups-regular.cha");
    std::fs::write(
        &input,
        include_str!("../../../../corpus/reference/annotation/groups-regular.cha"),
    )?;
    let output = tree.path().join("report.json");
    let inputs = [input];
    let primary_parser = TreeSitterParser::new()?;
    let native_parser = Re2cParser::new();
    for site in [FaultSite::TreeSitter, FaultSite::Re2c, FaultSite::Both] {
        for severity in [Severity::Error, Severity::Warning] {
            for native_result in [NativeResult::Parsed, NativeResult::Rejected] {
                for destination in [Destination::New, Destination::Existing] {
                    match destination {
                        Destination::New => {
                            if output.exists() {
                                std::fs::remove_file(&output)?;
                            }
                        }
                        Destination::Existing => std::fs::write(&output, b"retained report")?,
                    }
                    let mut parse = |path: &Path, source: &str| {
                        let primary_findings = ErrorCollector::new();
                        let primary =
                            primary_parser.parse_chat_file_streaming(source, &primary_findings);
                        let native_findings = ErrorCollector::new();
                        let native = native_parser.parse_chat_file(source, 0, &native_findings);
                        // Ordinary findings survive admission, even alongside a fault.
                        let input_finding = ParseError::at_span(
                            ErrorCode::SyntaxError,
                            Severity::Error,
                            Span::new(0, 1),
                            "ordinary input finding",
                        );
                        primary_findings.report(input_finding.clone());
                        native_findings.report(input_finding);
                        let mut fault =
                            ParseError::internal("injected producer fault", Span::new(0, 1));
                        fault.severity = severity;
                        match site {
                            FaultSite::TreeSitter => primary_findings.report(fault),
                            FaultSite::Re2c => native_findings.report(fault),
                            FaultSite::Both => {
                                primary_findings.report(fault.clone());
                                native_findings.report(fault);
                            }
                        }
                        let native = match native_result {
                            NativeResult::Parsed => {
                                assert!(
                                    matches!(native, ParseOutcome::Parsed(_)),
                                    "reference fixture builds a native model"
                                );
                                native
                            }
                            NativeResult::Rejected => ParseOutcome::Rejected,
                        };
                        CompletedParserPair::admit(
                            path,
                            primary,
                            primary_findings,
                            native,
                            native_findings,
                        )
                    };
                    let failure = report(tree.path(), &inputs, &output, &mut parse)
                        .expect_err("internal faults cannot publish a successful report");
                    let failure = failure.downcast::<ParserPairFailure>()?;
                    assert_eq!(failure.path, inputs[0]);
                    match &failure.failure {
                        FailedParsers::TreeSitter(batch) | FailedParsers::Re2c(batch) => {
                            assert!(
                                batch
                                    .diagnostics()
                                    .iter()
                                    .any(|d| d.code == ErrorCode::SyntaxError)
                            );
                            assert!(
                                batch
                                    .diagnostics()
                                    .iter()
                                    .any(|d| d.code == ErrorCode::InternalError
                                        && d.severity == severity)
                            );
                        }
                        FailedParsers::Both { tree_sitter, re2c } => {
                            for batch in [tree_sitter, re2c] {
                                assert!(
                                    batch
                                        .diagnostics()
                                        .iter()
                                        .any(|d| d.code == ErrorCode::InternalError
                                            && d.severity == severity)
                                );
                            }
                        }
                    }
                    match destination {
                        Destination::New => assert!(!output.exists()),
                        Destination::Existing => {
                            assert_eq!(std::fs::read(&output)?, b"retained report")
                        }
                    }
                }
            }
        }
    }
    // Ordinary input diagnostics and a completed native rejection remain data.
    for native_result in [NativeResult::Parsed, NativeResult::Rejected] {
        let mut parse = |path: &Path, source: &str| {
            let primary_findings = ErrorCollector::new();
            let primary = primary_parser.parse_chat_file_streaming(source, &primary_findings);
            let native_findings = ErrorCollector::new();
            let native = native_parser.parse_chat_file(source, 0, &native_findings);
            for findings in [&primary_findings, &native_findings] {
                findings.report(ParseError::at_span(
                    ErrorCode::SyntaxError,
                    Severity::Error,
                    Span::new(0, 1),
                    "ordinary input finding",
                ));
            }
            let native = match native_result {
                NativeResult::Parsed => {
                    assert!(
                        matches!(native, ParseOutcome::Parsed(_)),
                        "reference fixture builds a native model"
                    );
                    native
                }
                NativeResult::Rejected => ParseOutcome::Rejected,
            };
            let pair = CompletedParserPair::admit(
                path,
                primary,
                primary_findings,
                native,
                native_findings,
            )?;
            assert!(pair.diagnostics().iter().all(|batch| {
                batch
                    .diagnostics()
                    .iter()
                    .any(|d| d.code == ErrorCode::SyntaxError)
            }));
            Ok(pair)
        };
        report(tree.path(), &inputs, &output, &mut parse)?;
        let _: serde_json::Value = serde_json::from_slice(&std::fs::read(&output)?)?;
    }
    Ok(())
}
