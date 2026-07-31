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

//! CLI-boundary coverage for invariant 1 of the span-splicing engine design
//! (`ParseProduct`, `crates/talkbank-parser/src/parser/chat_file_parser/chat_file/product.rs`):
//! parsing never discards a model it built.
//!
//! Real consequence this closes (measured 2026-07-30): `chatter debug fix-s`
//! on IISRP `049-1.cha` had the target utterance parsed and healthy, and
//! threw the whole file away over an unrelated `&-` error hundreds of lines
//! later, then `die()`d the entire run. These tests exercise the real CLI
//! subprocess boundary, not an in-process parser call, because the bug lived
//! in how each `chatter debug` command reacted to the parser's return value,
//! not in the parser itself.

use std::path::PathBuf;

use talkbank_parser_tests::test_error::TestError;

use crate::common::{CliHarness, combined_output, stdout_string, write_fixture};

/// A minimal CHAT header for a single-language, single-participant fixture.
const HEADER: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI||female|||Target_Child|||\n";

/// A well-formed first utterance followed by an utterance whose `%mor` tier
/// has an empty POS field (a construct proven, in
/// `talkbank-parser`'s `built_with_diagnostics_is_not_unbuildable` unit
/// test, to produce a diagnostic while the document still builds a model).
fn healthy_then_diagnostic_body() -> String {
    format!("{HEADER}*CHI:\thello world .\n*CHI:\tgoodbye .\n%mor:\t|goodbye .\n@End\n")
}

/// `chatter debug sanitize` on a single file: the FIRST utterance is
/// well-formed and LATER content is not. The command must still produce
/// its result for the healthy region (the sanitized first utterance),
/// rather than aborting the whole file over the later diagnostic, and must
/// report the diagnostic rather than silently dropping it.
#[test]
fn sanitize_produces_healthy_region_and_reports_the_rest() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let input = healthy_then_diagnostic_body();
    let fixture = write_fixture(harness.home_dir(), "mixed.cha", &input)?;

    let output = harness.run_output(&["debug", "sanitize", fixture.to_str().unwrap()])?;
    assert!(
        output.status.success(),
        "sanitize must still succeed for the healthy region; output: {}",
        combined_output(&output)
    );

    // Sanitize replaces word content with `wN` placeholders (that is the
    // whole point of the tool), so "healthy region survives" here means
    // the first utterance still comes out as a normal two-word sanitized
    // utterance (`w1 w2 .`), not that its literal text is preserved.
    let sanitized = stdout_string(&output);
    assert!(
        sanitized.contains("*CHI:\tw1 w2 ."),
        "the healthy first utterance must still be sanitized and emitted, got:\n{sanitized}"
    );

    let combined = combined_output(&output);
    assert!(
        combined.contains("diagnostic"),
        "the later diagnostic must be reported, not silently dropped, got:\n{combined}"
    );

    Ok(())
}

/// A fix-s fixture whose whole-utterance `@s` shortcuts qualify for
/// rewrite: every word in the utterance carries `@s`, so `chatter debug
/// fix-s` rewrites it to a `[- spa]` precode. Proven in
/// `talkbank-transform`'s `rewrites_whole_utterance_shortcuts_to_precode`.
fn fix_s_qualifying_body() -> String {
    "@UTF8\n@Begin\n@Languages:\teng, spa\n@Participants:\tCHI Target_Child\n\
@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thola@s amiga@s .\n@End\n"
        .to_string()
}

/// `chatter debug fix-s` over TWO paths, the first of which produces a
/// diagnostic partway through (mirroring the real IISRP `049-1.cha`
/// incident). The second path must still be processed and rewritten; the
/// first must be reported, not silently skipped, and neither file may
/// abort the run (the `die()` behaviour this closes).
#[test]
fn fix_s_processes_the_second_file_after_a_diagnostic_in_the_first() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    let first = write_fixture(
        harness.home_dir(),
        "diagnostic_first.cha",
        &healthy_then_diagnostic_body(),
    )?;
    let second = write_fixture(
        harness.home_dir(),
        "qualifies.cha",
        &fix_s_qualifying_body(),
    )?;

    let paths: Vec<PathBuf> = vec![first, second.clone()];
    let path_args: Vec<&str> = paths.iter().map(|p| p.to_str().unwrap()).collect();
    let mut args = vec!["debug", "fix-s"];
    args.extend(path_args);

    let run = harness.run_output(&args)?;
    assert!(
        run.status.success(),
        "a diagnostic in the first file must not abort the run; output: {}",
        combined_output(&run)
    );

    let report = combined_output(&run);
    assert!(
        report.contains("diagnostic") || report.contains("WARNING"),
        "the first file's diagnostic must be reported, got:\n{report}"
    );

    let rewritten = std::fs::read_to_string(&second)?;
    assert!(
        rewritten.contains("[- spa] hola amiga ."),
        "the second file must still be processed and rewritten, got:\n{rewritten}"
    );

    Ok(())
}
