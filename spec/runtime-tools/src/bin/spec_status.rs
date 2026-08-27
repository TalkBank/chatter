//! `spec-status`: what state is the spec system in, right now?
//!
//! # Why this exists
//!
//! Every basic question about the spec system used to be answered by reading
//! source. How many specs are there and how many assert anything? What does an
//! example's `**Source**` line do? What happens to a spec marked
//! `not_implemented`? Which runner checks which artifact? All of it was
//! discoverable only by grepping, and one session answered several of those
//! questions wrongly on the way to answering them right.
//!
//! Prose alone cannot fix that, because prose goes stale and nothing checks it.
//! This command DERIVES its answers from the same code the gates use, so it
//! cannot drift from them: the example counts come from
//! [`error_spec_validation::run`], the runner the CI gate itself calls.
//!
//! ```bash
//! just spec-status
//! ```

use generators::repo_paths::RepoRoot;
use generators::spec::error::ErrorSpec;
use generators::spec::metadata::Status;
use spec_runtime_tools::error_spec_validation::{self, Request, spec_dir};
use std::collections::BTreeMap;

/// The parity manifest, read for a COUNT only.
///
/// Deliberately a minimal shape rather than the full typed model that
/// `talkbank-parser-tests` owns: that crate is in the other cargo workspace,
/// and this is a report, not an authority. The manifest's own gate
/// (`manifest_agrees_with_clan_reference`) decides whether it is right; this
/// only says how big each bucket is.
#[derive(serde::Deserialize)]
struct ParityManifest {
    entries: Vec<ParityEntry>,
}

#[derive(serde::Deserialize)]
struct ParityEntry {
    status: String,
    #[serde(default)]
    no_obligation_reason: Option<String>,
}

/// The repository root, resolved by the workspace's one resolver.
///
/// Fallible, because the resolver now PROVES the directory is a chatter
/// checkout rather than counting two levels up and trusting the result.
fn repo_root() -> Result<RepoRoot, String> {
    RepoRoot::resolve(None).map_err(|why| why.to_string())
}

/// Count the loaded specs by the `Status` their CODE carries.
///
/// # Derived from the LOADER, not from the file text
///
/// This walked the directory itself and scanned every line for a
/// `- **Status**:` prefix, which made it a sixth reader of a format that now
/// has a schema. When the format moved to frontmatter on 2026-08-21 the scan
/// matched nothing, so the report put all 238 files in a `(none declared,
/// defaulted)` bucket and exited 0: a status report that had stopped reading
/// statuses, printing a confident wrong answer under a heading that says
/// STATUS.
///
/// Taking the tally from the loaded specs also deletes the bucket. Since R1 a
/// spec file does not declare a status at all: it names a code, and the code's
/// status lives in `spec/codes/error-codes.toml`, so a spec that could report
/// "declared nothing" is now one that does not load.
fn spec_statuses(specs: &[ErrorSpec]) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for spec in specs {
        *counts.entry(spec.status().as_str().to_owned()).or_default() += 1;
    }
    counts
}

/// List deferred specs whose code the validator ALREADY emits.
///
/// A spec marked `not_implemented` is skipped by the gate and its generated
/// tests carry `#[ignore]`. If the rule has since been implemented and nobody
/// updated the status, that is coverage sitting switched off: the work is done
/// and nothing checks it. This asks the validator directly.
fn list_deferred(root: &RepoRoot) -> Result<(), String> {
    let parser = talkbank_parser::TreeSitterParser::new().map_err(|e| e.to_string())?;
    let specs = generators::spec::error::ErrorSpec::load_for_repo(root)?;
    let (mut ready, mut genuine) = (0usize, 0usize);
    for spec in &specs {
        if spec.status() == Status::Implemented {
            continue;
        }
        let definition = &spec.error;
        for (index, example) in definition.examples.iter().enumerate() {
            let codes = error_spec_validation::emit_for(&parser, example).all_distinct_codes();
            let own = &definition.code;
            let emits_own = codes.iter().any(|c| c == own.as_str());
            if emits_own {
                ready += 1;
            } else {
                genuine += 1;
            }
            println!(
                "  {:<44} ex{} {:<16} {} emits: {}",
                spec.source_file(),
                index + 1,
                spec.status(),
                if emits_own {
                    "IMPLEMENTED ->"
                } else {
                    "still deferred"
                },
                if codes.is_empty() {
                    "nothing".to_owned()
                } else {
                    codes.join(", ")
                },
            );
        }
    }
    println!(
        "\n  {ready} deferred example(s) ALREADY emit their own code: the rule exists\n           and the spec still says it does not, so the test is skipped for nothing.\n           {genuine} are genuinely unimplemented."
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let root = repo_root()?;
    if std::env::args().any(|arg| arg == "--deferred") {
        return list_deferred(&root);
    }
    let spec_dir = spec_dir(&root);

    println!("SPEC SYSTEM STATUS");
    println!("==================\n");

    let specs = ErrorSpec::load_for_repo(&root)?;
    println!("Error specs in {}:", spec_dir.display());
    for (status, count) in spec_statuses(&specs) {
        println!("  {count:>4}  {status}");
    }
    println!(
        "\n  `status` is a fact about a CODE, declared once in\n  \
         spec/codes/error-codes.toml and reached through the code a spec names.\n  \
         A spec naming an unregistered code does not load, and the file is\n  \
         named, so these counts are the LOADED specs and a non-spec file in the\n  \
         directory cannot appear as a row."
    );

    // The same REQUEST the CI gate builds, not a copy of its four fields. The
    // comment here used to say these numbers cannot disagree with the gate's,
    // beside a hand-written struct literal that could drift from it silently.
    let report = error_spec_validation::run(&Request::for_repo(&root)?)?;

    println!("\nExamples ({} in total):", report.total());
    println!(
        "  {:>4}  verified: emitted every code they declare",
        report.verified
    );
    println!(
        "  {:>4}  deferred: spec is not_implemented / deprecated / unreachable",
        report.deferred
    );
    println!("  {:>4}  failing", report.failures.len());
    for failure in &report.failures {
        println!("        {failure}");
    }

    println!(
        "\n  Every example carries a CLAIM: `violates` (the spec's code must\n  \
         appear), `legal` (it must not), or `subsumed_by` (the targets appear\n  \
         and the spec's code does not). Extra emitted codes are allowed; the\n  \
         exact per-stage sets are the snapshot's business."
    );

    let manifest_path = root.join("crates/talkbank-parser-tests/tests/check_parity/manifest.json");
    let manifest: ParityManifest = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("read {manifest_path:?}: {e}"))?,
    )
    .map_err(|e| format!("parse {manifest_path:?}: {e}"))?;

    let mut by_status: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_reason: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in &manifest.entries {
        *by_status.entry(entry.status.as_str()).or_default() += 1;
        if let Some(reason) = &entry.no_obligation_reason {
            *by_reason.entry(reason.as_str()).or_default() += 1;
        }
    }

    println!(
        "\nCLAN CHECK parity ({} codes adjudicated):",
        manifest.entries.len()
    );
    for (status, count) in &by_status {
        println!("  {count:>4}  {status}");
    }
    for (reason, count) in &by_reason {
        println!("        {count:>4}  no_obligation: {reason}");
    }

    println!(
        "\nGATES (what actually checks what)\n\
         \x20 error_spec_codes            every example emits its declared codes\n\
         \x20 manifest_agrees_with_clan_reference  parity manifest vs check.cpp\n\
         \x20 clan_check_grounding        fixtures vs the REAL CLAN binary (needs CLAN)\n\
         \x20 generated_form_marker_sites_are_current  form-marker registry outputs\n\
         \x20 generated_symbol_sets_are_current        symbol registry outputs\n\
         \nProcedure: book/src/contributing/spec-workflow.md
 Reference: book/src/architecture/spec-system.md"
    );

    Ok(())
}
