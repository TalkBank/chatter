//! The deferred CLI distinguishes code status from executable claim evidence.

#[test]
fn deferred_report_names_verified_claims_without_calling_codes_implemented() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_spec_status"))
        .arg("--deferred")
        .output()
        .expect("run spec status");
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 report");
    // Authored alternate-diagnostic and legal controls, not invented CHAT.
    for file in ["E003.md", "E304.md", "E348.md"] {
        let rows: Vec<_> = stdout.lines().filter(|line| line.contains(file)).collect();
        assert!(!rows.is_empty(), "missing {file}");
        assert!(rows.iter().all(|line| line.contains("claim verified")));
        assert!(rows.iter().all(|line| line.contains("not_implemented")));
    }
    assert!(stdout.contains("verified legal/subsumption claims"));
    assert!(stdout.contains("not a count of missing validation rules"));
}
