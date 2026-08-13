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

use spec_runtime_tools::error_spec_validation::{
    self, CodeCheck, CodeFilter, Request, SkippedSpecs,
};
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("spec/runtime-tools is two levels below the repository root")
        .to_path_buf()
}

/// Count spec files by their declared `Status`, and by NOT declaring one.
///
/// The absent case is still reported separately, even though the loader now
/// refuses a spec without the bullet: this reader walks the directory, so it
/// also sees the non-spec files (`README.md` and the enhancement guide) that
/// the loader filters out. Folding them into a status tally would misreport
/// them as specs.
fn spec_statuses(spec_dir: &Path) -> Result<BTreeMap<String, usize>, String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let entries = std::fs::read_dir(spec_dir).map_err(|e| format!("read {spec_dir:?}: {e}"))?;
    for entry in entries {
        let path = entry.map_err(|e| format!("entry: {e}"))?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path:?}: {e}"))?;
        let declared = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("- **Status**:"))
            .map(|value| value.trim().to_owned());
        let key = declared.unwrap_or_else(|| "(none declared, defaulted)".to_owned());
        *counts.entry(key).or_default() += 1;
    }
    Ok(counts)
}

/// List every example that declares no `Expected Error Codes`, with what it
/// actually emits.
///
/// This is what makes the backlog workable rather than merely counted: the
/// codes come from `emit_for`, the same path the gate runs, so what you see is
/// what the example would be asserting. Deciding whether those codes are RIGHT
/// is still adjudication; this only removes the archaeology.
fn list_unasserted(spec_dir: &Path) -> Result<(), String> {
    let parser = talkbank_parser::TreeSitterParser::new().map_err(|e| e.to_string())?;
    let specs = generators::spec::error::ErrorSpec::load_all(spec_dir)?;
    let mut listed = 0usize;
    for spec in &specs {
        for definition in &spec.errors {
            for (index, example) in definition.examples.iter().enumerate() {
                if !example.expected_codes.is_empty() {
                    continue;
                }
                let mut codes: Vec<String> = error_spec_validation::emit_for(&parser, example)
                    .iter()
                    .map(|error| error.code.as_str().to_owned())
                    .collect();
                codes.sort();
                codes.dedup();
                let emitted = if codes.is_empty() {
                    "nothing at all".to_owned()
                } else {
                    codes.join(", ")
                };
                println!(
                    "  {} (example {})  status={}  emits: {emitted}",
                    spec.source_file,
                    index + 1,
                    spec.metadata.status
                );
                listed += 1;
            }
        }
    }
    println!("\n  {listed} example(s) assert nothing. An example emitting NOTHING is the");
    println!("  worse case: it is invalid CHAT that chatter accepts, or it is not");
    println!("  invalid and the spec is wrong about its own premise.");
    Ok(())
}

/// List deferred specs whose code the validator ALREADY emits.
///
/// A spec marked `not_implemented` is skipped by the gate and its generated
/// tests carry `#[ignore]`. If the rule has since been implemented and nobody
/// updated the status, that is coverage sitting switched off: the work is done
/// and nothing checks it. This asks the validator directly.
fn list_deferred(spec_dir: &Path) -> Result<(), String> {
    let parser = talkbank_parser::TreeSitterParser::new().map_err(|e| e.to_string())?;
    let specs = generators::spec::error::ErrorSpec::load_all(spec_dir)?;
    let (mut ready, mut genuine) = (0usize, 0usize);
    for spec in &specs {
        if spec.metadata.status == "implemented" {
            continue;
        }
        for definition in &spec.errors {
            for (index, example) in definition.examples.iter().enumerate() {
                let mut codes: Vec<String> = error_spec_validation::emit_for(&parser, example)
                    .iter()
                    .map(|error| error.code.as_str().to_owned())
                    .collect();
                codes.sort();
                codes.dedup();
                let own = &definition.code;
                let emits_own = codes.iter().any(|c| c == own.as_str());
                if emits_own {
                    ready += 1;
                } else {
                    genuine += 1;
                }
                println!(
                    "  {:<44} ex{} {:<16} {} emits: {}",
                    spec.source_file,
                    index + 1,
                    spec.metadata.status,
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
    }
    println!(
        "\n  {ready} deferred example(s) ALREADY emit their own code: the rule exists\n           and the spec still says it does not, so the test is skipped for nothing.\n           {genuine} are genuinely unimplemented."
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let root = repo_root();
    if std::env::args().any(|arg| arg == "--deferred") {
        return list_deferred(&root.join("spec").join("errors"));
    }
    if std::env::args().any(|arg| arg == "--unasserted") {
        return list_unasserted(&root.join("spec").join("errors"));
    }
    let spec_dir = root.join("spec").join("errors");

    println!("SPEC SYSTEM STATUS");
    println!("==================\n");

    println!("Error specs in {}:", spec_dir.display());
    for (status, count) in spec_statuses(&spec_dir)? {
        println!("  {count:>4}  {status}");
    }
    println!(
        "\n  `Status` is REQUIRED: a spec without the bullet fails to load, naming\n  \
         the file. It used to default to `implemented`, so 104 of 238 specs said\n  \
         nothing and had an answer invented for them (fixed 2026-08-11). Any row\n  \
         above reading `(none declared)` is a non-spec file in the directory."
    );

    // The same call the CI gate makes, so these numbers cannot disagree with it.
    let report = error_spec_validation::run(&Request {
        spec_dir: spec_dir.clone(),
        code_check: CodeCheck::Verify,
        skipped: SkippedSpecs::Omit,
        filter: CodeFilter::All,
    })?;

    println!("\nExamples ({} in total):", report.total());
    println!(
        "  {:>4}  verified: emitted every code they declare",
        report.verified
    );
    println!(
        "  {:>4}  deferred: spec is not_implemented / deprecated / unreachable",
        report.deferred
    );
    println!(
        "  {:>4}  assert NOTHING: they declare no Expected Error Codes",
        report.no_expected_codes
    );
    println!("  {:>4}  failing", report.failures.len());
    for failure in &report.failures {
        println!("        {failure}");
    }

    println!(
        "\n  The check is a SUBSET test: an example passes when every code it\n  \
         declares was emitted. Extra codes are allowed, so declaring fewer is\n  \
         always safe and an example declaring NONE can never fail."
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
