//! Caller-supplied diagnostic evidence cannot become a CHAT validity verdict.
//! This is injected API-boundary evidence, not another malformed CHAT fixture.

use std::io::Write;
use talkbank_model::model::{FileStem, Line, TranscriptName};
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{
    ChatFile, ErrorCode, ErrorCollector, NullErrorSink, ParseError, RuleSelection, Severity,
};
use talkbank_model::{WriteChat, validation::ValidationFailure};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::parse_source_with_parser;

const SOURCE: &str = include_str!("../../../../corpus/reference/tiers/wor.cha");

fn install_diagnostics(document: &mut ChatFile, diagnostics: Vec<ParseError>) {
    let first = (&mut document.lines)
        .into_iter()
        .find_map(|line| match line {
            Line::Utterance(utterance) => Some(utterance),
            Line::Header { .. } => None,
        })
        .expect("reference speech");
    first.alignment_diagnostics = diagnostics;
}

fn assert_failure_evidence(
    failure: &ValidationFailure,
    policy: ValidationPolicy,
    name: TranscriptName<'_>,
    expected: &[ParseError],
) {
    assert!(failure.has_internal_failure());
    assert!(!failure.has_incomplete_parse());
    assert_eq!(failure.policy(), policy);
    assert_eq!(failure.name(), name);
    assert_eq!(failure.document().to_chat_string(), SOURCE);
    assert_eq!(failure.diagnostics(), expected);
}

#[test]
fn structure_admission_retains_tool_failure_through_reporting_and_refused_rejection() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let admitted = parse_source_with_parser(&parser, SOURCE)
        .admit(TranscriptName::Anonymous, &NullErrorSink)
        .expect("authored timed reference is valid");
    let baseline = admitted.document();
    assert_eq!(baseline.to_chat_string(), SOURCE);
    let policy = ValidationPolicy::new(RuleSelection::new(), AlignmentValidation::Structure);
    let name = TranscriptName::Named(FileStem::from_stem("WOR").expect("stored spelling"));

    // A warning-only positive control prevents a blanket refusal from passing.
    // W110 is real named-media evidence; diagnostics injected below are not.
    let observed = ErrorCollector::new();
    let valid = baseline
        .clone()
        .validate_with_policy(policy, &observed, name)
        .expect("the case-difference warning permits admission");
    let [warning] = valid.diagnostics() else {
        panic!("the named reference emits exactly its W110 warning");
    };
    assert_eq!(warning.code.as_str(), "W110");
    assert_eq!(warning.severity, Severity::Warning);
    assert!(matches!(
        warning.location,
        talkbank_model::DiagnosticLocation::Known { .. }
    ));
    assert_eq!(observed.to_vec(), valid.diagnostics());
    let warning = warning.clone();
    let span = baseline
        .utterances()
        .next()
        .expect("reference speech")
        .main
        .location
        .span()
        .expect("located main tier");

    for fault_severity in [Severity::Error, Severity::Warning] {
        for finding_severity in [Severity::Error, Severity::Warning] {
            let mut fault = ParseError::internal("independent producer failure", span);
            fault.severity = fault_severity;
            let finding = ParseError::at_location(
                ErrorCode::SyntaxError,
                finding_severity,
                span,
                "independent input finding",
            );
            for injected in [
                vec![fault.clone(), finding.clone()],
                vec![finding.clone(), fault.clone()],
            ] {
                let mut document = baseline.clone();
                // Structure-only policy consumes existing alignment diagnostics;
                // it does not recompute them as IncludeTierAlignment would.
                // Public mutable evidence is a producer boundary, not source gold.
                install_diagnostics(&mut document, injected.clone());
                let observed = ErrorCollector::new();
                let failure = document
                    .validate_with_policy(policy, &observed, name)
                    .expect_err("fault kind, not severity or ordering, prevents admission");
                let mut expected = vec![warning.clone()];
                expected.extend(injected);
                assert_eq!(observed.to_vec(), expected);
                assert_failure_evidence(&failure, policy, name, &expected);

                let display = format!(
                    "internal tool failure; CHAT validity was not determined{}",
                    expected
                        .iter()
                        .map(|d| format!("\n  {} {}", d.code.as_str(), d.message))
                        .collect::<String>()
                );
                assert_eq!(failure.to_string(), display);
                // A consumer's truncated report must propagate the failed write,
                // including truncation inside the headline or a retained finding.
                for capacity in 0..display.len() {
                    let mut bytes = vec![0; capacity];
                    let error = bytes
                        .as_mut_slice()
                        .write_fmt(format_args!("{failure}"))
                        .expect_err("truncated failure reports are failed writes");
                    assert_eq!(error.kind(), std::io::ErrorKind::WriteZero);
                    assert_eq!(bytes, display.as_bytes()[..capacity]);
                }
                let failure = failure
                    .into_rejection()
                    .expect_err("tool evidence cannot authorize an input-repair rejection");
                assert_failure_evidence(&failure, policy, name, &expected);
                assert_eq!(failure.to_string(), display);
                let recovered = failure.into_unchecked();
                assert_eq!(recovered.to_chat_string(), SOURCE);
            }
        }
    }

    // The discriminator is not simply "any diagnostic is a tool failure".
    let finding = ParseError::at_location(
        ErrorCode::SyntaxError,
        Severity::Error,
        span,
        "independent input finding",
    );
    let mut document = baseline.clone();
    install_diagnostics(&mut document, vec![finding.clone()]);
    let failure = document
        .validate_with_policy(policy, &NullErrorSink, name)
        .expect_err("ordinary error findings prevent valid admission");
    assert!(!failure.has_internal_failure());
    assert!(!failure.has_incomplete_parse());
    let (document, diagnostics) = failure
        .into_rejection()
        .expect("ordinary input rejection is available for evidence-bound repair");
    assert_eq!(document.to_chat_string(), SOURCE);
    assert_eq!(diagnostics, [warning, finding]);
    assert_eq!(
        baseline.to_chat_string(),
        SOURCE,
        "the admitted control is unchanged"
    );
}
