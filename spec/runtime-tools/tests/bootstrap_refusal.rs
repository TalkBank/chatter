//! A runtime writer cannot repair source files already compiled into itself.

use std::fs;
use std::process::Command;

#[test]
fn stale_bootstrap_sources_refuse_before_snapshot_or_other_artifact_writes() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    fs::create_dir_all(root.join("spec/errors")).unwrap();
    fs::create_dir_all(root.join("spec/codes")).unwrap();
    fs::create_dir_all(root.join("crates")).unwrap();
    fs::write(root.join("Cargo.toml"), "# isolated artifact fixture\n").unwrap();
    fs::write(root.join("spec/codes/error-codes.toml"),
        "[[code]]\ncode = 'E999'\nvariant = 'BootstrapExample'\nsummary = 'Bootstrap example.'\nkind = 'Invalidity'\nstatus = 'implemented'\n").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_spec_gen"))
        .arg("--repo-root")
        .arg(root)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("bootstrap sources are stale"));
    assert!(!root.join("spec/observations").exists());
    assert!(!root.join("crates/talkbank-model").exists());
}
