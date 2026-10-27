//! Test-instrument admission contracts, not canonical CHAT coverage.

use talkbank_model::{ChatFile, ErrorCode, ParseError, Severity, Span};
use talkbank_parser::{ParseProduct, TreeSitterParser};
use talkbank_parser_tests::test_error::{ParseProductTestExt, strict_parse};

#[test]
fn fixture_extraction_preserves_a_built_model_despite_diagnostics() {
    let original = reference();
    let product = ParseProduct::Built {
        file: original.clone(),
        diagnostics: vec![syntax(Severity::Error)],
    };
    assert_eq!(product.expect_built(), original);
}

#[test]
#[should_panic(expected = "input finding")]
fn fixture_extraction_reports_unbuildable_diagnostics() {
    ParseProduct::Unbuildable {
        diagnostics: vec![syntax(Severity::Error)],
    }
    .expect_built();
}

fn reference() -> ChatFile {
    let product = TreeSitterParser::new()
        .expect("grammar")
        .parse_chat_file(include_str!(
            "../../../../corpus/reference/core/basic-conversation.cha"
        ));
    assert!(product.diagnostics().is_empty());
    strict_parse(product).expect("clean reference")
}

fn syntax(severity: Severity) -> ParseError {
    ParseError::at_span(
        ErrorCode::SyntaxError,
        severity,
        Span::new(0, 1),
        "input finding",
    )
}

#[test]
fn internal_fault_cannot_pass_as_a_warning() {
    for severity in [Severity::Warning, Severity::Error] {
        let mut fault = ParseError::internal("injected producer failure", Span::new(1, 2));
        fault.severity = severity;
        let diagnostics = vec![syntax(Severity::Warning), fault];
        let result = strict_parse(ParseProduct::Built {
            file: reference(),
            diagnostics: diagnostics.clone(),
        });
        assert_eq!(
            result.expect_err("internal kind must reject").errors,
            diagnostics
        );
    }
}

#[test]
fn completed_findings_keep_the_strict_severity_policy() {
    let original = reference();
    for diagnostics in [vec![], vec![syntax(Severity::Warning)]] {
        let admitted = strict_parse(ParseProduct::Built {
            file: original.clone(),
            diagnostics,
        })
        .expect("completed warning-only parse");
        assert_eq!(admitted, original);
    }
    let diagnostics = vec![syntax(Severity::Warning), syntax(Severity::Error)];
    let rejected = strict_parse(ParseProduct::Built {
        file: original,
        diagnostics: diagnostics.clone(),
    })
    .expect_err("error-severity finding");
    assert_eq!(rejected.errors, diagnostics);
}

#[test]
fn unbuildable_never_becomes_success() {
    let mut fault = ParseError::internal("injected producer failure", Span::new(1, 2));
    fault.severity = Severity::Warning;
    for diagnostics in [vec![], vec![syntax(Severity::Warning)], vec![fault]] {
        let rejected = strict_parse(ParseProduct::Unbuildable {
            diagnostics: diagnostics.clone(),
        })
        .expect_err("absence of a model cannot pass");
        assert_eq!(rejected.errors, diagnostics);
    }
}
