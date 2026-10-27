//! Linker anomaly reports share the audit output lifecycle, not a validity verdict.

use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/reference/edge-cases/linker-patterns.cha")
}

fn audit(input: &Path, report: &Path) -> std::process::Output {
    crate::common::chatter_cmd()
        .args(["inspect", "linkers"])
        .arg(input)
        .arg("--anomalies")
        .arg(report)
        .output()
        .unwrap()
}

#[test]
fn linker_audit_refuses_existing_input_alias() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.cha");
    std::fs::copy(fixture(), &input).unwrap();
    let original = std::fs::read(&input).unwrap();
    let output = audit(&input, &input);
    assert!(!output.status.success());
    assert_eq!(std::fs::read(&input).unwrap(), original);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Anomalies written:"));
}

#[test]
fn linker_audit_failed_discovery_leaves_destination_absent() {
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("report.jsonl");
    let output = audit(&dir.path().join("missing.cha"), &report);
    assert!(!output.status.success());
    assert!(!report.exists());
}

#[test]
fn linker_audit_new_report_retains_anomaly_wire_fields() {
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("report.jsonl");
    let output = audit(&fixture(), &report);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(report).unwrap();
    let rows: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["type"], "qf_wrong_terminator");
    assert_eq!(rows[1]["type"], "sc_wrong_terminator");
    for row in rows {
        assert!(row["utt_index"].is_number());
        assert_eq!(row["speaker"], "CHI");
        assert!(row["text"].is_string());
        assert!(row["suggested_fix"].is_object());
    }
    assert!(String::from_utf8_lossy(&output.stderr).contains("Anomalies written:"));
}
