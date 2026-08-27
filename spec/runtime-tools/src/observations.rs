//! R3 of the spec-system redesign: observations are RECORDED, never authored.
//!
//! # What this is
//!
//! For every example of every error spec, the exact diagnostic codes the
//! CURRENT binary produces, split by the stage that produced them. Committed as
//! a generated artifact under `spec/observations/` and held current by the same
//! byte-compare gate as every other artifact, so **a change in what the specs'
//! own examples trigger is a review event**: the diff names each example whose
//! behaviour moved, and each movement is adjudicated INTENDED (the behaviour
//! change was the point; commit the regenerated snapshot in the same change)
//! or UNINTENDED (a regression; fix the code, never the snapshot). Because
//! every example is also checked for a byte-exact round trip, the snapshot is
//! the regression instrument for both what the parser SAYS and what it
//! WRITES, over data that ships with the repository.
//!
//! # What it exists to enable, which is Phase 2's other two steps
//!
//! - **R2's `subsumed by <code>` claim becomes VERIFIABLE**: the snapshot says
//!   what actually fires on the example, so "chatter reports E316 today" is a
//!   recorded observation rather than an assertion nobody re-checks.
//! - **R4 derives layer-of-capture instead of trusting an authored `layer`
//!   field**: the parse/validation split per example is exactly "which stage
//!   of our pipeline catches this", observed rather than declared.
//!
//! # Every example, unconditionally
//!
//! The snapshot covers `not_implemented`, `deprecated` and
//! `unreachable_from_chat` specs too. An observation is not an assertion: for
//! the six unimplemented linker specs the honest record is "nothing fires",
//! and the 2026-08-20 adjudication of the inheriting examples was possible
//! only by RUNNING exactly the specs the gates skip. Skipping them here would
//! rebuild the blind spot the adjudication had to climb out of.
//!
//! # What is deliberately NOT in the file
//!
//! No timestamp, no commit hash, no message text. The first two would make the
//! byte-compare gate fail on every regeneration; messages are part of a
//! release's observable behaviour (their wording is contract-noted elsewhere)
//! but here they would turn every rewording into snapshot churn without adding
//! adjudication power, since the code and the stage are what R2 and R4 consume.
//! Codes are recorded as a sorted, deduplicated SET per stage: that a
//! malformed line raises one diagnostic twice is noise for this instrument,
//! and the normalization is stated here rather than applied silently.

use std::fmt::Write as _;

use anyhow::{Context, Result, bail};
use generators::artifacts::GeneratedFiles;
use generators::spec::ErrorSpec;
use talkbank_parser::TreeSitterParser;

use crate::error_spec_validation::{StagedDiagnostics, emit_for, spec_dir};

pub use talkbank_spec_vocabulary::observations::{
    ExampleObservation, ObservationSnapshot, SNAPSHOT_FILE,
};

/// The constant provenance sentence.
const GENERATED_BY: &str = "just spec-gen (spec-runtime-tools::observations); adjudicate every diff as intended or unintended";

/// Build the snapshot by running every example through the ONE sanctioned
/// example-running path, [`emit_for`].
///
/// # Errors
///
/// When the specs cannot be loaded, the parser cannot be constructed, or an
/// example PANICS the pipeline. A panic is a defect to fix, not an observation
/// to record: recording it would need a message string, which is
/// nondeterministic across builds, and a snapshot that quietly carries a
/// panicking example reads as a finished observation of it.
pub fn build_snapshot(repo_root: &std::path::Path) -> Result<GeneratedFiles> {
    let registry = talkbank_spec_vocabulary::registry::CodeRegistry::load(repo_root)?;
    let specs = ErrorSpec::load_all(spec_dir(repo_root), &registry)
        .map_err(|err| anyhow::anyhow!("loading specs: {err}"))?;
    if specs.is_empty() {
        bail!("no specs found; a snapshot of nothing must not be written as though observed");
    }
    let parser = TreeSitterParser::new().context("constructing the parser")?;

    let mut examples = Vec::new();
    for spec in &specs {
        for (index, example) in spec.error.examples.iter().enumerate() {
            let staged = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                emit_for(&parser, example)
            }))
            .map_err(|_| {
                anyhow::anyhow!(
                    "{} example {} PANICKED the pipeline; fix that before snapshotting",
                    spec.source_file(),
                    index + 1
                )
            })?;
            examples.push(observe(
                talkbank_spec_vocabulary::observations::ExampleId::from_enumerate(
                    spec.source_file(),
                    index,
                ),
                &staged,
            ));
        }
    }

    let snapshot = ObservationSnapshot {
        generated_by: GENERATED_BY.to_owned(),
        examples,
    };

    let mut files = GeneratedFiles::new();
    files.insert(
        SNAPSHOT_FILE.into(),
        serde_json::to_string_pretty(&snapshot).context("serializing the snapshot")? + "\n",
    );
    files.insert("README.md".into(), readme());
    Ok(files)
}

/// One example's staged codes, normalized by the ONE owner of that rule
/// (`error_spec_validation::distinct_codes`).
fn observe(
    id: talkbank_spec_vocabulary::observations::ExampleId<'_>,
    staged: &StagedDiagnostics,
) -> ExampleObservation {
    ExampleObservation {
        spec: id.spec_file().to_owned(),
        example: id.position(),
        parse: crate::error_spec_validation::distinct_codes(&staged.parse),
        validation: crate::error_spec_validation::distinct_codes(&staged.validation),
        roundtrip: staged.roundtrip,
    }
}

/// The directory's own explanation, generated so the wholesale clear cannot
/// delete a hand-written one.
fn readme() -> String {
    let mut out = String::new();
    // Assembled line by line for the same reason the book table is: a
    // continued string literal keeps source indentation.
    let _ = writeln!(
        out,
        "# Observations: what the spec examples actually trigger"
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "GENERATED by `just spec-gen`; checked by `just spec-check`. Do not edit."
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "`{SNAPSHOT_FILE}` records, for every example of every error spec, the exact"
    );
    let _ = writeln!(
        out,
        "diagnostic codes the current binary produces, split by the stage (parse or"
    );
    let _ = writeln!(
        out,
        "validation) that emitted them. It is an OBSERVATION, never an assertion:"
    );
    let _ = writeln!(
        out,
        "the normative claims live in the specs themselves, and this file is what"
    );
    let _ = writeln!(out, "they are checked against and derived from.");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "**A diff here is a review event, adjudicated as intended or unintended:**"
    );
    let _ = writeln!(
        out,
        "every changed entry is classified INTENDED (the behaviour change was the"
    );
    let _ = writeln!(
        out,
        "point; commit the regenerated snapshot in the same change) or UNINTENDED"
    );
    let _ = writeln!(out, "(a regression; fix the code, not the snapshot).");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Design: R3 of `spec-system` (see the architecture book chapter)."
    );
    out
}
