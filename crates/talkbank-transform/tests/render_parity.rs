// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Cross-surface error-rendering parity.
//!
//! The CLI (plain text) and the desktop GUI (ANSI -> HTML) render the SAME
//! `ParseError` through two sibling functions in `talkbank_transform::rendering`:
//! [`render_error_with_miette_with_source`] (plain) and
//! [`render_error_with_miette_with_source_colored`] (ANSI). They must agree on
//! the source line they point the caret at; if they disagree, the GUI shows a
//! caret at a different line than the CLI for the identical error, which is
//! exactly the 2026-06-05 wrong-line desktop bug.
//!
//! This is the regression guard that makes the two surfaces unable to diverge on
//! line numbering. It drives the committed spec fixture E601 (an invalid `%mor`
//! tier on line 8) through the real parse+validate pipeline, then renders each
//! enhanced error both ways and asserts both reference the error's true line.
//!
//! Run with: `cargo nextest run -p talkbank-transform --test render_parity`

use std::fs;
use std::path::PathBuf;

use talkbank_model::{ParseError, ParseValidateOptions, enhance_errors_with_source};
use talkbank_transform::{
    PipelineError, RenderMode, parse_and_validate, render_diagnostics,
    render_error_with_miette_with_source, render_error_with_miette_with_source_colored,
};

/// Repo root: this crate is `<root>/crates/talkbank-transform`, so pop twice.
fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop(); // talkbank-transform -> crates
    dir.pop(); // crates -> repo root
    dir
}

/// Read a committed error fixture and return its source plus the RAW (un-enhanced)
/// validation errors, exactly as `parse_and_validate` yields them. Callers that
/// need enhanced errors enhance a clone themselves (the direct-function test);
/// `render_diagnostics` enhances internally, so it must receive raw errors.
fn raw_errors_for(rel_fixture: &str) -> (String, Vec<ParseError>) {
    let fixture = workspace_root().join(rel_fixture);
    let content = fs::read_to_string(&fixture)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", fixture.display()));

    let options = ParseValidateOptions::default().with_validation();
    let errors = match parse_and_validate(&content, options) {
        Err(PipelineError::Validation(errors)) => errors,
        Err(PipelineError::Parse(parse_errors)) => parse_errors.errors,
        other => panic!("expected validation/parse errors from {rel_fixture}, got {other:?}"),
    };
    (content, errors)
}

const E601_FIXTURE: &str = "tests/error_corpus/validation_errors/E601_invalid_dependent_tier.cha";

/// The plain and ANSI renders of the same enhanced error must point at the same
/// source line, the one the error itself reports (`location.line`). The CLI uses
/// the plain render and is the reference; the desktop uses the ANSI render. A
/// divergence here is the wrong-line GUI bug.
#[test]
fn plain_and_colored_renders_agree_on_line() {
    let (content, raw) = raw_errors_for(E601_FIXTURE);
    assert!(
        !raw.is_empty(),
        "fixture {E601_FIXTURE} should produce at least one validation error"
    );
    let mut errors = raw;
    enhance_errors_with_source(&mut errors, &content);

    // The name passed is the fallback source name; enhanced errors supply their
    // own "input"-named windowed source, so this matches the real call sites.
    let name = workspace_root().join(E601_FIXTURE).display().to_string();

    for error in &errors {
        let line = error
            .location
            .line
            .expect("enhance_errors_with_source populates location.line");
        let marker = format!(":{line}:");

        let plain = render_error_with_miette_with_source(error, &name, &content);
        let colored = render_error_with_miette_with_source_colored(error, &name, &content);

        assert!(
            plain.contains(&marker),
            "plain render should reference the error's true line {line} ({marker}):\n{plain}"
        );
        assert!(
            colored.contains(&marker),
            "colored (ANSI/GUI) render should reference the SAME line {line} ({marker}) as the \
             plain CLI render, but did not:\n{colored}"
        );
    }
}

/// The shared `render_diagnostics` orchestration (which both surfaces now use)
/// must: (a) enhance, so `RenderedDiagnostic.error.location.line` is populated;
/// (b) render `text` at that true line; (c) under `Ansi`, also render `ansi` at
/// the SAME line. This guards the consolidated entry point itself, not just the
/// two underlying functions.
#[test]
fn render_diagnostics_text_and_ansi_agree_on_line() {
    let (content, raw) = raw_errors_for(E601_FIXTURE);
    let name = workspace_root().join(E601_FIXTURE).display().to_string();

    let diagnostics = render_diagnostics(&raw, &name, &content, RenderMode::Ansi);
    assert_eq!(
        diagnostics.len(),
        raw.len(),
        "render_diagnostics should yield one RenderedDiagnostic per input error"
    );
    assert!(!diagnostics.is_empty(), "expected at least one diagnostic");

    for d in &diagnostics {
        let line = d
            .error
            .location
            .line
            .expect("render_diagnostics enhances internally -> location.line is set");
        let marker = format!(":{line}:");

        assert!(
            d.text.contains(&marker),
            "RenderedDiagnostic.text should reference line {line} ({marker}):\n{}",
            d.text
        );
        let ansi = d
            .ansi
            .as_ref()
            .expect("RenderMode::Ansi must populate RenderedDiagnostic.ansi");
        assert!(
            ansi.contains(&marker),
            "RenderedDiagnostic.ansi should reference the SAME line {line} ({marker}) as text:\n{ansi}"
        );
    }

    // Plain mode must omit the ANSI form (no wasted colored render for the CLI).
    let plain = render_diagnostics(&raw, &name, &content, RenderMode::Plain);
    assert!(
        plain.iter().all(|d| d.ansi.is_none()),
        "RenderMode::Plain must not produce an ANSI form"
    );
}
