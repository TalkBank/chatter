//! Candidate reports must not hide selected-input I/O failures.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "candidate-input-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn extract(input: &Path, output: Option<&Path>, cap: Option<usize>) -> std::io::Result<Output> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new(env!("CARGO_BIN_EXE_extract_corpus_candidates"));
    command
        .arg("--data-dir")
        .arg(input)
        .args(["--languages", "eng", "--json", "--node-types"])
        .arg(root.join("grammar/src/node-types.json"));
    if let Some(path) = output {
        command.arg("--output").arg(path);
    }
    if let Some(limit) = cap {
        command.arg("--max-files").arg(limit.to_string());
    }
    command.output()
}

#[test]
fn selected_read_failure_refuses_stdout_and_preserves_report() -> anyhow::Result<()> {
    let dir = TestDirectory::new()?;
    let input = dir.0.join("inputs");
    std::fs::create_dir(&input)?;
    // An encoding fault at the filesystem boundary, not an authored CHAT case.
    let bad = input.join("unreadable.cha");
    std::fs::write(&bad, [0xff])?;
    let report = dir.0.join("report.json");
    std::fs::write(&report, b"retained report")?;
    for destination in [None, Some(report.as_path())] {
        let result = extract(&input, destination, None)?;
        assert!(!result.status.success(), "{result:?}");
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("unreadable.cha"));
        assert_eq!(std::fs::read(&report)?, b"retained report");
    }
    Ok(())
}

#[test]
fn missing_input_does_not_create_output() -> anyhow::Result<()> {
    let dir = TestDirectory::new()?;
    let report = dir.0.join("new/report.json");
    let result = extract(&dir.0.join("missing"), Some(&report), None)?;
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!report.parent().expect("parent").exists());
    Ok(())
}

#[test]
fn explicit_zero_cap_does_not_read_unselected_files() -> anyhow::Result<()> {
    let dir = TestDirectory::new()?;
    std::fs::write(dir.0.join("unreadable.cha"), [0xff])?;
    let result = extract(&dir.0, None, Some(0))?;
    assert!(result.status.success(), "{result:?}");
    let report: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(report["eng"], serde_json::json!([]));
    Ok(())
}

#[test]
fn canonical_reference_is_still_ranked() -> anyhow::Result<()> {
    let dir = TestDirectory::new()?;
    let input = dir.0.join("sample.cha");
    std::fs::write(
        &input,
        include_bytes!("../../../corpus/reference/content/terminators-standard.cha"),
    )?;
    let result = extract(&dir.0, None, None)?;
    assert!(result.status.success(), "{result:?}");
    let report: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    let candidates = report["eng"].as_array().expect("English candidates");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["path"], input.to_str().expect("test path"));
    Ok(())
}
