//! CI gate: every error spec's example produces the code it claims.
//!
//! `validate_error_specs` is named as THE validation step in ten documents
//! under `spec/`, and until this file existed it ran only when a human typed
//! `cargo run`. CI runs `cargo test --manifest-path spec/Cargo.toml
//! --workspace`, which never invokes a `main`.
//!
//! Running it found exactly one disagreement in 330 examples. That is the
//! argument for gates stated as a number: the discrepancy was neither large
//! nor subtle, it was simply never looked at.

use spec_runtime_tools::error_spec_validation::{Request, run};

/// SURVIVES: policy. WHICH specs are exempt, and on what grounds, is a
/// judgement with real alternatives; no type holds it. What the types hold is
/// that a finding carries its own code (so an exemption cannot be matched
/// against a prefix of rendered prose) and that the verdict and its text are
/// one value the renderer shares, so `cargo run` and CI cannot disagree.
///
/// The exemption list itself, and both directions of its check, live in the
/// library beside the harness limitation they describe.
#[test]
fn every_error_spec_example_emits_its_declared_code() -> Result<(), String> {
    let root = generators::repo_paths::RepoRoot::resolve(None).map_err(|why| why.to_string())?;
    let report = run(&Request::for_repo(&root))?;

    // The spec corpus has been ~330 examples for a long time, so a collapse
    // means a loading fault rather than a real reduction. Checked HERE, off the
    // same run: the predecessor made this a second `#[test]` that re-parsed and
    // re-validated all 330 examples through tree-sitter to read one number.
    if report.total() < 100 {
        return Err(format!(
            "only {} examples examined; expected the full spec corpus. \
             A gate over a near-empty set reports success and means nothing.",
            report.total()
        ));
    }

    report.outcome().map(|summary| println!("{summary}"))
}

/// A spec marked deferred must not already emit its own code.
///
/// `not_implemented` skips the example here and puts `#[ignore]` on its
/// generated tests. If the rule has since been implemented and nobody updated
/// the status, that is finished work with its coverage switched off, and it
/// stays switched off precisely because nothing looks.
///
/// SURVIVES as a cross-artifact agreement no type can hold: one side is a
/// word in a markdown file, the other is the validator's behaviour at runtime.
///
/// Found exactly one on 2026-08-11: E311 carried a `Status note` explaining
/// that tree-sitter recovery made it unreachable because E316 fired first.
/// The parser had since improved to emit a specific "Unclosed replacement
/// bracket" diagnostic, so the note described history, the example declared a
/// code it no longer produced, and the whole spec was skipped.
#[test]
fn deferred_specs_are_not_already_implemented() -> Result<(), String> {
    let parser = talkbank_parser::TreeSitterParser::new().map_err(|e| e.to_string())?;
    let root = generators::repo_paths::RepoRoot::resolve(None).map_err(|why| why.to_string())?;
    let specs = generators::spec::error::ErrorSpec::load_all(
        spec_runtime_tools::error_spec_validation::spec_dir(&root),
    )?;

    let mut stale = Vec::new();
    for spec in &specs {
        if spec.metadata.status == generators::spec::metadata::Status::Implemented {
            continue;
        }
        let definition = &spec.error;
        for example in &definition.examples {
            let emitted = spec_runtime_tools::error_spec_validation::emit_for(&parser, example);
            if emitted
                .all()
                .any(|error| error.code.as_str() == definition.code.as_str())
            {
                stale.push(format!(
                    "{} is `{}` but already emits {}",
                    spec.source_file(),
                    spec.metadata.status,
                    definition.code
                ));
            }
        }
    }

    if stale.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} spec(s) are marked deferred while the rule already works, so their \
         generated tests are `#[ignore]`d for nothing. Set `Status: implemented`, \
         declare the codes the example actually emits, and regenerate:\n  {}",
        stale.len(),
        stale.join("\n  ")
    ))
}
