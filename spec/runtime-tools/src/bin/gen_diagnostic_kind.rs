//! Generate the exhaustive per-[`ErrorCode`] `DiagnosticKind` registry from
//! `spec/errors/*.md`'s required `Kind` metadata field.
//!
//! This replaces the hand-written match that used to live directly in
//! `crates/talkbank-model/src/errors/diagnostic_kind.rs`: that match was a
//! MIRROR of judgment already recorded in each code's spec file's prose
//! (its `## CHAT Rule` / `## Notes` sections), curated by hand into a
//! second, independently-mutable place, which is exactly the kind of drift
//! hazard the `Kind` spec field exists to remove. This binary is the
//! derivation step: read the one true source (`spec/errors/`), emit the
//! registry.
//!
//! Depends on `talkbank-model` (for [`ErrorCode::iter`] and its `Debug`
//! variant names), which is why this binary lives in `spec/runtime-tools`
//! rather than the plain `spec/tools` "generators" crate: `spec/tools`
//! deliberately stays free of runtime parser/model dependencies for
//! ordinary spec generation workflows (see that crate's module docs), and
//! this is the one generator that genuinely needs the live `ErrorCode` enum
//! to enumerate every variant, not just the codes a spec file happens to
//! name.
//!
//! ## Fails closed on divergence
//!
//! `ErrorCode` (the enum) and `spec/errors/` (the spec files) are two
//! independently hand-maintained sets. This generator refuses to emit
//! anything when they disagree, in either direction:
//!
//! - a variant with no spec file naming it, or
//! - a spec-named code with no matching variant (a retired/renumbered code
//!   whose spec file was never deleted).
//!
//! There is deliberately no default-to-`Invalidity` fallback for the first
//! case: a defaulted arm is exactly how a code with no specification, and
//! therefore no authority to be adjudicated against, stayed invisible until
//! 2026-07-31. A code with no spec file has no basis for ANY classification,
//! not even the status-quo-preserving one.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --manifest-path spec/runtime-tools/Cargo.toml --bin gen_diagnostic_kind
//! ```
//!
//! Regenerate after ANY change to a spec file's `- **Kind**:` bullet, or
//! after adding/removing an `ErrorCode` variant. The output is committed;
//! `diagnostic_kind.rs`'s `kind_of` delegates to it, so a stale generated
//! file silently misclassifies a code until this is rerun.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;
use generators::spec::ErrorSpec;
use generators::spec::error::ErrorKind;
use talkbank_model::ErrorCode;

/// CLI arguments: the spec directory to read and the generated Rust file to
/// write.
#[derive(Parser)]
#[command(name = "gen_diagnostic_kind")]
#[command(about = "Generate the DiagnosticKind registry from spec/errors")]
struct Args {
    /// Root directory containing error specs.
    #[arg(long, default_value = "spec/errors")]
    spec_dir: PathBuf,

    /// Generated Rust file to write.
    #[arg(
        long,
        default_value = "crates/talkbank-model/src/errors/generated_diagnostic_kind.rs"
    )]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let specs = ErrorSpec::load_all(&args.spec_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {e}"))?;
    eprintln!("Loaded {} error specifications", specs.len());

    // code string -> (Kind, spec file it came from). A code can legitimately
    // have more than one spec file (an `_auto.md` plus a hand-authored
    // companion); every one of them must agree on Kind, since Kind is a
    // property of the CODE, not of any one example file. Disagreement is a
    // spec defect and fails the generation run loudly rather than picking a
    // winner silently.
    let mut by_code: BTreeMap<String, (ErrorKind, String)> = BTreeMap::new();
    for spec in &specs {
        for def in &spec.errors {
            let kind = spec.metadata.kind;
            match by_code.get(&def.code) {
                None => {
                    by_code.insert(def.code.clone(), (kind, spec.source_file.clone()));
                }
                Some((existing_kind, existing_file)) if *existing_kind != kind => {
                    bail!(
                        "code {} has conflicting Kind across spec files: {} says {:?}, {} says {:?}",
                        def.code,
                        existing_file,
                        existing_kind,
                        spec.source_file,
                        kind
                    );
                }
                Some(_) => {
                    // Same code, same Kind, from a second spec file: fine.
                }
            }
        }
    }

    // Divergence check 1: every `ErrorCode` variant must be named by at
    // least one spec file. No default is applied here; a gap is a hard
    // failure, not a status-quo-preserving guess.
    let missing_specs: Vec<String> = ErrorCode::iter()
        .map(|code| code.as_str().to_string())
        .filter(|code_str| !by_code.contains_key(code_str))
        .collect();

    // Divergence check 2: every spec-named code must name a live
    // `ErrorCode` variant. A spec file surviving the retirement or
    // renumbering of its code is the same divergence in the other
    // direction (e.g. W602, deleted from the enum on 2026-07-16 without
    // its spec file being removed).
    let orphan_specs: Vec<(String, String)> = by_code
        .iter()
        .filter(|(code_str, _)| ErrorCode::parse_exact(code_str).is_none())
        .map(|(code_str, (_, source_file))| (code_str.clone(), source_file.clone()))
        .collect();

    if !missing_specs.is_empty() || !orphan_specs.is_empty() {
        let mut msg = String::from(
            "spec/errors <-> ErrorCode divergence detected; the DiagnosticKind \
             registry was NOT regenerated.\n",
        );
        if !missing_specs.is_empty() {
            msg.push_str(&format!(
                "\n{} ErrorCode variant(s) have no spec file:\n",
                missing_specs.len()
            ));
            for code in &missing_specs {
                msg.push_str(&format!("  {code}\n"));
            }
        }
        if !orphan_specs.is_empty() {
            msg.push_str(&format!(
                "\n{} spec-named code(s) name no live ErrorCode variant:\n",
                orphan_specs.len()
            ));
            for (code, source_file) in &orphan_specs {
                msg.push_str(&format!("  {code} ({source_file})\n"));
            }
        }
        msg.push_str(
            "\nEvery ErrorCode variant needs exactly one spec file, and every \
             spec-named code must name a live ErrorCode variant: write the \
             missing spec(s), delete the dead variant(s), or delete the \
             orphaned spec file(s).\n",
        );
        bail!(msg);
    }

    // `by_code` is now total over `ErrorCode::iter()` with no orphans: every
    // lookup below is guaranteed to succeed by the checks above, but the
    // lookup itself still returns a `Result` (never `.unwrap()`/`.expect()`)
    // so a future refactor that weakens the guard fails loudly instead of
    // panicking.
    let mut arms = String::new();
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut non_invalidity: Vec<(String, &'static str)> = Vec::new();

    for code in ErrorCode::iter() {
        let code_str = code.as_str();
        let variant_name = format!("{code:?}");
        let (kind, source_file) = by_code.get(code_str).ok_or_else(|| {
            anyhow::anyhow!("{code_str}: missing from by_code after divergence check")
        })?;

        let variant = kind.diagnostic_kind_variant();
        *counts.entry(variant).or_insert(0) += 1;
        if variant != "Invalidity" {
            non_invalidity.push((code_str.to_string(), variant));
        }

        arms.push_str(&format!(
            "        ErrorCode::{variant_name} => DiagnosticKind::{variant}, // {code_str}: {source_file}\n"
        ));
    }

    let source = format!(
        "//! Generated by `gen_diagnostic_kind` from `spec/errors/*.md`'s required\n\
         //! `- **Kind**:` metadata field. DO NOT EDIT BY HAND.\n\
         //!\n\
         //! Regenerate with:\n\
         //! ```text\n\
         //! cargo run --manifest-path spec/runtime-tools/Cargo.toml --bin gen_diagnostic_kind\n\
         //! ```\n\
         //!\n\
         //! Every arm's trailing comment names the spec file its `Kind` came\n\
         //! from. The generator refuses to run at all (see its module docs)\n\
         //! when any `ErrorCode` variant has no spec file, or any spec-named\n\
         //! code has no matching variant, so this match is exhaustive with\n\
         //! no defaulted arms.\n\n\
         use super::codes::ErrorCode;\n\
         use super::diagnostic_kind::DiagnosticKind;\n\n\
         /// Exhaustive per-[`ErrorCode`] [`DiagnosticKind`] lookup, generated from\n\
         /// spec. See [`super::diagnostic_kind::kind_of`] for the stable public\n\
         /// entry point that delegates here.\n\
         pub(crate) fn kind_of_from_spec(code: ErrorCode) -> DiagnosticKind {{\n\
         \x20   match code {{\n\
         {arms}\
         \x20   }}\n\
         }}\n"
    );

    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&args.output, source)
        .with_context(|| format!("writing {}", args.output.display()))?;
    eprintln!("Wrote: {}", args.output.display());

    eprintln!("\nKind counts:");
    for (kind, count) in &counts {
        eprintln!("  {kind}: {count}");
    }

    if !non_invalidity.is_empty() {
        eprintln!("\nNon-Invalidity codes:");
        for (code, kind) in &non_invalidity {
            eprintln!("  {code}: {kind}");
        }
    }

    Ok(())
}
