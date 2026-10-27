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

#[test]
fn deferred_admission_refuses_a_stale_subsumption_claim() {
    use generators::{repo_paths::RepoRoot, spec::error::ErrorSpec};
    use spec_runtime_tools::deferred_specs::ReviewedDeferred;

    let root = RepoRoot::resolve(None).unwrap();
    let path = root.join("spec/errors/E999.md");
    let source = std::fs::read_to_string(&path).unwrap();
    let registry = root.code_registry().unwrap();
    let current = ErrorSpec::from_frontmatter(&path, &source, &registry).unwrap();
    assert!(ReviewedDeferred::from_specs(&[current]).is_ok());

    // Restore the historical claim, keeping the authored CHAT bytes identical.
    let claim = "subsumed_by = ['E305', 'E316']";
    assert_eq!(source.matches(claim).count(), 1);
    let stale = source.replace(claim, "subsumed_by = ['E305', 'E600']");
    let stale = ErrorSpec::from_frontmatter(&path, &stale, &registry).unwrap();
    let Err(refusal) = ReviewedDeferred::from_specs(&[stale]) else {
        panic!("stale deferred claim admitted");
    };
    assert!(refusal.contains("E999.md ex1"), "{refusal}");
    assert!(refusal.contains("claim CONTRADICTED"), "{refusal}");
    assert!(refusal.contains("observed: E305, E316"), "{refusal}");
}

#[test]
fn deferred_admission_refuses_an_emitted_code_despite_a_planned_claim() {
    use generators::{repo_paths::RepoRoot, spec::error::ErrorSpec};
    use spec_runtime_tools::deferred_specs::ReviewedDeferred;
    use talkbank_spec_vocabulary::registry::CodeRegistry;

    let root = RepoRoot::resolve(None).unwrap();
    let registry = std::fs::read_to_string(root.join("spec/codes/error-codes.toml")).unwrap();
    let start = "code = \"E305\"";
    let (before, rest) = registry.split_once(start).unwrap();
    let (entry, after) = rest.split_once("[[code]]").unwrap();
    let status = "status = \"implemented\"";
    assert_eq!(entry.matches(status).count(), 1);
    let entry = entry.replace(status, "status = \"not_implemented\"");
    let registry = CodeRegistry::parse(&format!("{before}{start}{entry}[[code]]{after}")).unwrap();
    let path = root.join("spec/errors/E305.md");
    let source = std::fs::read_to_string(&path).unwrap();
    let spec = ErrorSpec::from_frontmatter(&path, &source, &registry).unwrap();
    let Err(refusal) = ReviewedDeferred::from_specs(&[spec]) else {
        panic!("emitted deferred code admitted");
    };
    assert!(
        refusal.contains("deferred code E305 already emitted"),
        "{refusal}"
    );
}

#[test]
fn an_absent_deferred_code_can_retain_a_planned_violation() {
    use generators::{repo_paths::RepoRoot, spec::error::ErrorSpec};
    use spec_runtime_tools::deferred_specs::ReviewedDeferred;
    use talkbank_spec_vocabulary::frontmatter::Claim;

    let root = RepoRoot::resolve(None).unwrap();
    let path = root.join("spec/errors/E003.md");
    let source = std::fs::read_to_string(&path).unwrap();
    let registry = root.code_registry().unwrap();
    let mut spec = ErrorSpec::from_frontmatter(&path, &source, &registry).unwrap();
    assert_eq!(spec.error.examples.len(), 1);
    spec.error.examples[0].claim = Claim::Violates;
    let reviewed = ReviewedDeferred::from_specs(&[spec]).unwrap();
    assert!(reviewed.to_string().contains("1 planned violation claims"));
}
