use std::path::Path;
use talkbank_parser_tests::{check_mapping_audit::report, test_error::TestError};

#[test]
fn assessment_book_is_current() -> Result<(), TestError> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = talkbank_parser_tests::check_mapping_audit::assessment_report(
        &root.join("tests/check_parity/manifest.json"),
    )?;
    assert_eq!(
        actual,
        std::fs::read_to_string(
            root.join("../../book/src/architecture/errors-and-validation/check-parity-audit.md")
        )?
    );
    Ok(())
}

#[test]
fn assessment_completion_is_derived_and_invalid_inventories_are_refused() -> Result<(), TestError> {
    use talkbank_parser_tests::check_mapping_audit::assessment_report_json;
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/check_parity/manifest.json"),
    )?;
    let mut value: serde_json::Value = serde_json::from_str(&source)?;
    value["entries"][0]["status"] = "gap".into();
    let reopened = assessment_report_json(&serde_json::to_string(&value)?)?;
    assert!(reopened.contains("Assessment incomplete"));
    assert!(reopened.contains("1 unresolved gaps"));
    value["entries"][1] = value["entries"][0].clone();
    assert!(assessment_report_json(&serde_json::to_string(&value)?).is_err());
    value["entries"] = serde_json::json!([]);
    assert!(assessment_report_json(&serde_json::to_string(&value)?).is_err());
    Ok(())
}

#[test]
fn mapping_inventory_is_current_and_does_not_claim_runtime_parity() -> Result<(), TestError> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = report(&manifest.join("clan-check-reference/check-error-codes.json"))?;
    let committed =
        std::fs::read_to_string(manifest.join("../../docs/audits/check-parity-audit.md"))?;
    assert_eq!(actual, committed);
    assert!(actual.contains("curated mapping"));
    assert!(!actual.contains("| full |"));
    assert!(actual.contains("reports no verified-parity count"));
    Ok(())
}

#[test]
fn duplicate_reference_ids_are_rejected() -> Result<(), TestError> {
    let reference = r#"{"codes":[
        {"code":6,"messages":["Begin"],"n_call_sites":1},
        {"code":6,"messages":["Begin"],"n_call_sites":1}
    ]}"#;
    assert!(talkbank_parser_tests::check_mapping_audit::report_json(reference).is_err());
    Ok(())
}
