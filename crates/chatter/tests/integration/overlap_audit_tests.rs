//! Real-process report preservation and completion contracts.

use std::path::Path;
use std::process::Output;

fn audit(input: &Path, report: &Path) -> Output {
    crate::common::chatter_cmd()
        .args(["debug", "overlap-audit"])
        .arg(input)
        .arg("--database")
        .arg(report)
        .output()
        .expect("audit process")
}

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/reference/ca/overlap-siblings.cha")
}

#[test]
fn overlap_audit_preserves_existing_report_and_input_aliases() {
    for alias in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.cha");
        std::fs::copy(fixture(), &input).unwrap();
        let report = dir.path().join("report.jsonl");
        if alias {
            std::fs::hard_link(&input, &report).unwrap();
        } else {
            std::fs::write(&report, "retained report\n").unwrap();
        }
        let original = std::fs::read(&input).unwrap();
        let retained = std::fs::read(&report).unwrap();
        let result = audit(&input, &report);
        assert!(!result.status.success(), "existing destination must refuse");
        assert_eq!(std::fs::read(&input).unwrap(), original);
        assert_eq!(std::fs::read(&report).unwrap(), retained);
        assert!(!String::from_utf8_lossy(&result.stderr).contains("Database written:"));
    }
}

#[test]
fn overlap_audit_discovery_failure_does_not_create_report() {
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("report.jsonl");
    let result = audit(&dir.path().join("missing.cha"), &report);
    assert!(!result.status.success());
    assert!(!report.exists());
}

#[cfg(unix)]
#[test]
fn overlap_audit_refuses_symlink_without_touching_target() {
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("report.jsonl");
    let missing_target = dir.path().join("absent.jsonl");
    std::os::unix::fs::symlink(&missing_target, &report).unwrap();
    let result = audit(&fixture(), &report);
    assert!(!result.status.success());
    assert!(!missing_target.exists());
    assert!(
        std::fs::symlink_metadata(&report)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn overlap_audit_new_report_is_complete_jsonl() {
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("report.jsonl");
    let result = audit(&fixture(), &report);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = std::fs::read_to_string(&report).unwrap();
    let records: Vec<serde_json::Value> = output
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 1);
    assert!(records[0]["groups"].as_u64().unwrap() > 0);
    assert!(String::from_utf8_lossy(&result.stderr).contains("Database written:"));
}
