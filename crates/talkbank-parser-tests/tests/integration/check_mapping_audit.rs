use std::path::Path;
use talkbank_parser_tests::{check_mapping_audit::report, test_error::TestError};

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
