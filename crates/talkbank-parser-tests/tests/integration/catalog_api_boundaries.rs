//! Public repair requests can only inspect actual diagnosis-issued findings.
//! Construction/association refusals are compile-fail boundary evidence, not CHAT coverage.

use talkbank_model::{
    ErrorCode, RuleSelection,
    model::TranscriptName,
    validation::{AlignmentValidation, ValidationPolicy},
};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::splice::{CatalogResolution, FixDiagnosis};

fn observe<'a>(parser: &TreeSitterParser, source: &'a str) -> FixDiagnosis<'a> {
    FixDiagnosis::observe(
        parser,
        source,
        ValidationPolicy::new(RuleSelection::new(), AlignmentValidation::Structure),
        TranscriptName::Anonymous,
    )
}

#[test]
fn valid_nodes_do_not_issue_independent_repair_findings() {
    let parser = TreeSitterParser::new().expect("parser");
    for source in [
        include_str!("../../../../corpus/reference/core/basic-conversation.cha"),
        include_str!("../../../../corpus/reference/core/headers-media-http-url.cha"),
        include_str!("../error_corpus/validation_errors/W109_4.cha"),
    ] {
        let diagnosis = observe(&parser, source);
        assert!(diagnosis.findings().all(|finding| !matches!(
            finding.code,
            ErrorCode::IllegalUntranscribed | ErrorCode::MediaFilenameNonCanonicalUnicode
        )));
        assert_eq!(diagnosis.source(), source);
        assert!(diagnosis.parsed_source().is_some());
        // There is no diagnostic-code/location constructor for FixFinding.
        // Absence cannot be turned into a request at an otherwise valid node.
    }
}

#[test]
fn actual_findings_distinguish_unsupported_and_offered_policy() {
    let parser = TreeSitterParser::new().expect("parser");
    let source = include_str!("../error_corpus/validation_errors/E301_1.cha");
    let diagnosis = observe(&parser, source);
    let finding = diagnosis
        .findings()
        .find(|finding| finding.code.as_str() == "E316")
        .expect("actual grammar rejection, not an invented repair request");
    assert!(matches!(finding.catalog(), CatalogResolution::Unsupported));
    // Positive controls prevent an always-refuse catalog from passing.
    for (source, code) in [
        (
            include_str!(
                "../error_corpus/validation_errors/E241_illegal_untranscribed_marker_2.cha"
            ),
            ErrorCode::IllegalUntranscribed,
        ),
        (
            include_str!("../error_corpus/validation_errors/E258_1.cha"),
            ErrorCode::ConsecutiveCommas,
        ),
    ] {
        let diagnosis = observe(&parser, source);
        let finding = diagnosis
            .findings()
            .find(|finding| finding.code == code)
            .expect("actual authored finding");
        assert!(matches!(finding.catalog(), CatalogResolution::Offered(_)));
    }
}
